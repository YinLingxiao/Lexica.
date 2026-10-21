//! 复习会话编排：队列构建、逐级提示、判分落库、记忆更新。
//! SQL 直接写在这里（review 模块语句少，不再分 store 层）。

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::grader;
use super::model::{
    Hint, PromptSource, ReviewFeedback, ReviewItem, StudyCardEntry, StudyGrade, StudyPriority,
    StudySessionGroup, WordbookInfo,
};
use super::ReviewError;
use crate::database::fmt_ts;
use crate::dictionary::engine;
use crate::dictionary::model::WordEntry;
use crate::domain::{ExampleId, ReviewId, SenseId, WordId};
use crate::memory::model::RecallQuality;
use crate::memory::service::MemoryService;

/// Feature-owned tables: older installations have no study-plan migration yet.
pub fn ensure_study_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS custom_wordbook_words (
             word_id INTEGER PRIMARY KEY REFERENCES words(id) ON DELETE CASCADE,
             added_at TEXT NOT NULL DEFAULT ''
         );
         CREATE TABLE IF NOT EXISTS word_enrichments (
             word_id INTEGER PRIMARY KEY REFERENCES words(id) ON DELETE CASCADE,
             mnemonic TEXT,
             roots_json TEXT NOT NULL DEFAULT '[]',
             exam_example_text TEXT,
             exam_example_translation TEXT,
             standard_example_text TEXT,
             standard_example_translation TEXT,
             updated_at TEXT NOT NULL
         );",
    )
}

/// All identifiers here are fixed SQL, never interpolated user input. Missing
/// metadata in pre-plan databases is treated like an empty tagged wordbook.
fn wordbook_filter(conn: &Connection, wordbook: &str) -> Result<String, ReviewError> {
    match wordbook {
        "custom" => Ok("w.id IN (SELECT word_id FROM custom_wordbook_words UNION SELECT word_id FROM word_notes WHERE bookmarked = 1)".into()),
        "encountered" => Ok("EXISTS (SELECT 1 FROM encounters e WHERE e.word_id = w.id)".into()),
        "cet4" | "cet6" | "ky" | "toefl" | "ielts" | "gre" | "gk" | "oxford" => {
            let mut stmt = conn.prepare("PRAGMA table_info(words)")?;
            let columns = stmt.query_map([], |r| r.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?;
            let tags = if columns.iter().any(|c| c == "tags") {
                "instr(lower(w.tags), ?1) > 0"
            } else {
                "0"
            };
            let filter = if wordbook == "oxford" && columns.iter().any(|c| c == "oxford") {
                format!("(w.oxford = 1 OR {tags})")
            } else {
                tags.to_string()
            };
            // Bind a named numeric parameter even when metadata columns are absent.
            let filter = format!("({filter}) AND ?1 IS NOT NULL");
            let exists: bool = conn.query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM words w WHERE {filter})"),
                [wordbook], |r| r.get(0),
            )?;
            if exists {
                Ok(filter)
            } else {
                Ok("(w.source = 'seed' OR w.frequency_rank > 0) AND ?1 IS NOT NULL".into())
            }
        }
        _ => Err(ReviewError::NoWordData),
    }
}

pub struct ReviewService;

/// 只读复习组条目：完整词条（浏览用）+ 本地题目（出题用）。
/// 浏览阶段零副作用：不写 encounters、不创建 pending review。
#[derive(Debug, Clone, Serialize)]
pub struct ReviewGroupEntry {
    pub item: ReviewItem,
    pub entry: WordEntry,
}

/// 队列内一行（reviews 表的 pending 行快照）。
struct PendingReview {
    word_id: WordId,
    sense_id: Option<SenseId>,
    hints_used: u32,
}

impl ReviewService {
    /// Read-only card browsing: only grading records a first encounter.
    pub fn study_group(
        &self,
        conn: &Connection,
        wordbook: &str,
        daily_new_limit: u32,
        priority: StudyPriority,
        now: DateTime<Utc>,
    ) -> Result<StudySessionGroup, ReviewError> {
        ensure_study_schema(conn)?;
        let filter = wordbook_filter(conn, wordbook)?;
        let midnight = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let today_learned_new: u32 = conn.query_row(
            "SELECT COUNT(*) FROM memory_states WHERE first_visit_at >= ?1 AND first_visit_at < ?2",
            params![
                fmt_ts(midnight),
                fmt_ts(midnight + chrono::Duration::days(1))
            ],
            |r| r.get(0),
        )?;
        let mut due_cards = Vec::new();
        let due = MemoryService::default().due_words(conn, now)?;
        let due_count = due.len();
        for word in due {
            if let Some(card) = study_card(conn, word.word_id, false)? {
                due_cards.push(card);
                if due_cards.len() == 20 {
                    break;
                }
            }
        }
        let mut new_cards = Vec::new();
        let quota = daily_new_limit.saturating_sub(today_learned_new).min(20);
        if wordbook != "encountered" && quota > 0 {
            let sql = format!(
                "SELECT w.id FROM words w WHERE ({filter}) AND ?1 IS NOT NULL
                 AND NOT EXISTS (SELECT 1 FROM memory_states m WHERE m.word_id = w.id)
                 AND NOT EXISTS (SELECT 1 FROM ignored_words i WHERE i.word_id = w.id)
                 AND EXISTS (SELECT 1 FROM senses s WHERE s.word_id = w.id
                     AND (trim(s.english_definition) <> '' OR trim(coalesce(s.chinese_definition, '')) <> ''))
                 ORDER BY CASE WHEN w.frequency_rank > 0 THEN 0 ELSE 1 END,
                          w.frequency_rank, w.id LIMIT ?2"
            );
            let mut stmt = conn.prepare(&sql)?;
            let ids = stmt
                .query_map(params![wordbook, quota], |r| r.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            for id in ids {
                if let Some(card) = study_card(conn, WordId(id), true)? {
                    new_cards.push(card);
                }
            }
        }
        let new_count = new_cards.len();
        let mut cards = Vec::new();
        match priority {
            StudyPriority::NewFirst => {
                cards.extend(new_cards.into_iter().chain(due_cards).take(20))
            }
            StudyPriority::ReviewFirst => {
                cards.extend(due_cards.into_iter().chain(new_cards).take(20))
            }
            StudyPriority::Mixed => {
                let mut due = due_cards.into_iter();
                let mut new = new_cards.into_iter();
                while cards.len() < 20 {
                    match (due.next(), new.next()) {
                        (None, None) => break,
                        (a, b) => {
                            cards.extend(a);
                            if cards.len() < 20 {
                                cards.extend(b);
                            }
                        }
                    }
                }
            }
        }
        Ok(StudySessionGroup {
            cards,
            due_count,
            new_count,
            today_learned_new: today_learned_new as usize,
        })
    }

    pub fn study_grade(
        &self,
        conn: &Connection,
        word_id: WordId,
        grade: StudyGrade,
        now: DateTime<Utc>,
    ) -> Result<ReviewFeedback, ReviewError> {
        let tx = conn.unchecked_transaction()?;
        let entry = engine::entry_by_id(&tx, word_id)?.ok_or(ReviewError::NoWordData)?;
        let memory_service = MemoryService::default();
        let encountered: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM encounters WHERE word_id = ?1)",
            [word_id.0],
            |r| r.get(0),
        )?;
        if !encountered {
            memory_service.open_visit(&tx, word_id, now)?;
        }
        let (quality, correct) = match grade {
            StudyGrade::Know => (RecallQuality::Good, true),
            StudyGrade::Hazy => (RecallQuality::Hard, true),
            StudyGrade::Forgot => (RecallQuality::Forgotten, false),
        };
        tx.execute(
            "INSERT INTO reviews (word_id, started_at, completed_at, quality) VALUES (?1, ?2, ?2, ?3)",
            params![word_id.0, fmt_ts(now), quality.as_str()],
        )?;
        let review_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO review_attempts (review_id, at, action, correct) VALUES (?1, ?2, 'answer', ?3)",
            params![review_id, fmt_ts(now), correct],
        )?;
        let memory = memory_service.refresh_word(&tx, word_id, now)?;
        tx.commit()?;
        Ok(ReviewFeedback {
            correct,
            quality,
            answer: entry.display,
            hints_used: 0,
            memory,
        })
    }

    pub fn wordbooks_list(&self, conn: &Connection) -> Result<Vec<WordbookInfo>, ReviewError> {
        ensure_study_schema(conn)?;
        let mut books = Vec::new();
        for (id, name) in [
            ("cet4", "CET-4"),
            ("cet6", "CET-6"),
            ("ky", "考研"),
            ("toefl", "TOEFL"),
            ("ielts", "IELTS"),
            ("gre", "GRE"),
            ("gk", "高考"),
            ("oxford", "Oxford 3000"),
            ("custom", "自定义单词本"),
        ] {
            let filter = wordbook_filter(conn, id)?;
            let (total_words, learned_words) = conn.query_row(
                &format!("SELECT COUNT(*), COUNT(m.word_id) FROM words w LEFT JOIN memory_states m ON m.word_id = w.id WHERE ({filter}) AND ?1 IS NOT NULL"),
                [id], |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            books.push(WordbookInfo {
                id: id.into(),
                name: name.into(),
                total_words,
                learned_words,
            });
        }
        Ok(books)
    }

    pub fn custom_wordbook_toggle(
        &self,
        conn: &Connection,
        word_id: WordId,
        added: bool,
    ) -> Result<(), ReviewError> {
        ensure_study_schema(conn)?;
        if added {
            conn.execute(
                "INSERT OR IGNORE INTO custom_wordbook_words (word_id) VALUES (?1)",
                [word_id.0],
            )?;
        } else {
            conn.execute(
                "DELETE FROM custom_wordbook_words WHERE word_id = ?1",
                [word_id.0],
            )?;
        }
        Ok(())
    }

    /// 只读复习组：沿用现有到期筛选与优先级，截断到组上限。
    /// 本地题目仅用释义（中文优先）；AI 例句由应用层补成填空题。
    pub fn group(
        &self,
        conn: &Connection,
        now: DateTime<Utc>,
    ) -> Result<Vec<ReviewGroupEntry>, ReviewError> {
        let ids = MemoryService::default().due_words(conn, now)?;
        let mut items = Vec::with_capacity(crate::memory::service::REVIEW_SESSION_LIMIT);
        for due in ids {
            let id = due.word_id;
            if let Some(entry) = engine::entry_by_id(conn, id)? {
                if entry.senses.is_empty() {
                    continue;
                }
                let prompt = grader::build_definition_prompt(&entry);
                if prompt.text.trim().is_empty() {
                    continue; // ECDICT 词条可能既无例句也无英文释义，空 prompt 无法作答
                }
                items.push(ReviewGroupEntry {
                    item: ReviewItem {
                        word_id: id,
                        prompt: prompt.text,
                        kind: prompt.kind,
                        source: PromptSource::Dictionary,
                        sense_id: prompt.sense_id,
                        example_id: prompt.example_id,
                    },
                    entry,
                });
                if items.len() >= crate::memory::service::REVIEW_SESSION_LIMIT {
                    break;
                }
            }
        }
        Ok(items)
    }

    /// 复习队列（旧接口，组合层保留兼容）：组条目映射为题目条目。
    pub fn queue(
        &self,
        conn: &Connection,
        now: DateTime<Utc>,
    ) -> Result<Vec<ReviewItem>, ReviewError> {
        Ok(self.group(conn, now)?.into_iter().map(|g| g.item).collect())
    }

    /// 开始一题：创建 pending 的 reviews 行。
    pub fn start(
        &self,
        conn: &Connection,
        word_id: WordId,
        sense_id: Option<SenseId>,
        example_id: Option<ExampleId>,
        now: DateTime<Utc>,
    ) -> Result<ReviewId, ReviewError> {
        let entry = engine::entry_by_id(conn, word_id)?.ok_or(ReviewError::NoWordData)?;
        let selected_sense = match sense_id {
            Some(id) => Some(
                entry
                    .senses
                    .iter()
                    .find(|sense| sense.id == id)
                    .ok_or(ReviewError::NoWordData)?,
            ),
            None => None,
        };
        if let Some(example_id) = example_id {
            let valid = match selected_sense {
                Some(sense) => sense
                    .examples
                    .iter()
                    .any(|example| example.id == example_id),
                None => entry
                    .senses
                    .iter()
                    .flat_map(|sense| sense.examples.iter())
                    .any(|example| example.id == example_id),
            };
            if !valid {
                return Err(ReviewError::NoWordData);
            }
        }
        conn.execute(
            "INSERT INTO reviews (word_id, started_at, sense_id, example_id) VALUES (?1, ?2, ?3, ?4)",
            params![
                word_id.0,
                fmt_ts(now),
                sense_id.map(|id| id.0),
                example_id.map(|id| id.0)
            ],
        )?;
        Ok(ReviewId(conn.last_insert_rowid()))
    }

    /// 逐级提示（1..=3）。记 review_attempts，更新 hints_used。
    pub fn hint(
        &self,
        conn: &Connection,
        review_id: ReviewId,
        hint_no: u32,
        now: DateTime<Utc>,
    ) -> Result<Hint, ReviewError> {
        if !(1..=3).contains(&hint_no) {
            return Err(ReviewError::InvalidHint(hint_no));
        }
        let tx = conn.unchecked_transaction()?;
        let review = load_pending(&tx, review_id)?;
        let expected = review.hints_used + 1;
        if hint_no != expected {
            return Err(ReviewError::UnexpectedHint {
                requested: hint_no,
                expected,
            });
        }
        let entry = engine::entry_by_id(&tx, review.word_id)?.ok_or(ReviewError::NoWordData)?;
        let hint = hint_for(&entry, review.sense_id, hint_no)?;

        let action = match hint_no {
            1 => "hint1",
            2 => "hint2",
            _ => "hint3",
        };
        tx.execute(
            "INSERT INTO review_attempts (review_id, at, action) VALUES (?1, ?2, ?3)",
            params![review_id.0, fmt_ts(now), action],
        )?;
        tx.execute(
            "UPDATE reviews SET hints_used = MAX(hints_used, ?1) WHERE id = ?2",
            params![hint_no, review_id.0],
        )?;
        tx.commit()?;
        Ok(hint)
    }

    /// 提交：判分（answer=None 表示放弃/揭示）→ 完成复习 → 重折记忆。
    pub fn submit(
        &self,
        conn: &Connection,
        review_id: ReviewId,
        answer: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<ReviewFeedback, ReviewError> {
        let tx = conn.unchecked_transaction()?;
        let review = load_pending(&tx, review_id)?;
        let entry = engine::entry_by_id(&tx, review.word_id)?.ok_or(ReviewError::NoWordData)?;

        let correct = answer
            .as_deref()
            .is_some_and(|a| grader::is_correct(a, &entry.word));
        let quality = grader::quality_for(correct, review.hints_used);

        tx.execute(
            "UPDATE reviews SET completed_at = ?1, quality = ?2 WHERE id = ?3",
            params![fmt_ts(now), quality.as_str(), review_id.0],
        )?;
        tx.execute(
            "INSERT INTO review_attempts (review_id, at, action, correct) VALUES (?1, ?2, ?3, ?4)",
            params![
                review_id.0,
                fmt_ts(now),
                if correct || answer.is_some() {
                    "answer"
                } else {
                    "reveal"
                },
                correct
            ],
        )?;

        // 完成 review、写原始尝试、重折记忆必须原子提交。
        let memory = MemoryService::default().refresh_word(&tx, review.word_id, now)?;
        tx.commit()?;

        Ok(ReviewFeedback {
            correct,
            quality,
            answer: entry.display,
            hints_used: review.hints_used,
            memory,
        })
    }

    // ── 组内只读重考（不写 reviews / review_attempts，不动记忆投影）──

    /// 只读判分：复用 grader 的规范化与拼写容错规则。
    /// answer=None（揭示）恒为不正确，由调用方决定是否重新排队。
    pub fn practice_check(
        &self,
        conn: &Connection,
        word_id: WordId,
        answer: Option<String>,
    ) -> Result<bool, ReviewError> {
        let entry = engine::entry_by_id(conn, word_id)?.ok_or(ReviewError::NoWordData)?;
        Ok(answer
            .as_deref()
            .is_some_and(|a| grader::is_correct(a, &entry.word)))
    }

    /// 只读提示：与正式复习共用同一构造逻辑（hint_for），不落任何记录。
    pub fn practice_hint(
        &self,
        conn: &Connection,
        word_id: WordId,
        sense_id: Option<SenseId>,
        hint_no: u32,
    ) -> Result<Hint, ReviewError> {
        if !(1..=3).contains(&hint_no) {
            return Err(ReviewError::InvalidHint(hint_no));
        }
        let entry = engine::entry_by_id(conn, word_id)?.ok_or(ReviewError::NoWordData)?;
        hint_for(&entry, sense_id, hint_no)
    }
}

fn study_card(
    conn: &Connection,
    id: WordId,
    is_new: bool,
) -> Result<Option<StudyCardEntry>, ReviewError> {
    let Some(entry) = engine::entry_by_id(conn, id)? else {
        return Ok(None);
    };
    let prompt = grader::build_definition_prompt(&entry);
    if entry.senses.is_empty() || prompt.text.trim().is_empty() {
        return Ok(None);
    }
    let (bookmarked, in_custom_wordbook) = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM word_notes WHERE word_id = ?1 AND bookmarked = 1),
                EXISTS(SELECT 1 FROM custom_wordbook_words WHERE word_id = ?1)",
        [id.0],
        |r| Ok((r.get::<_, bool>(0)?, r.get::<_, bool>(1)?)),
    )?;
    Ok(Some(StudyCardEntry {
        item: ReviewItem {
            word_id: id,
            prompt: prompt.text,
            kind: prompt.kind,
            source: PromptSource::Dictionary,
            sense_id: prompt.sense_id,
            example_id: prompt.example_id,
        },
        entry,
        is_new,
        bookmarked,
        in_custom_wordbook,
    }))
}

/// 逐级提示构造（正式复习与只读重考共用，保证行为一致）。
fn hint_for(
    entry: &WordEntry,
    sense_id: Option<SenseId>,
    hint_no: u32,
) -> Result<Hint, ReviewError> {
    let sense = match sense_id {
        Some(id) => entry.senses.iter().find(|sense| sense.id == id),
        None => entry.senses.first(),
    }
    .ok_or(ReviewError::NoWordData)?;
    Ok(match hint_no {
        1 => Hint::Mask {
            text: grader::mask_hint(&entry.display),
        },
        2 => Hint::English {
            text: sense.english_definition.clone(),
        },
        _ => Hint::Chinese {
            text: sense.chinese_definition.clone().unwrap_or_default(),
        },
    })
}

type PendingReviewRow = (i64, i64, Option<i64>, i64, Option<String>);

/// 读取未完成的 review 行；已完成/不存在 → 明确错误。
fn load_pending(conn: &Connection, review_id: ReviewId) -> Result<PendingReview, ReviewError> {
    let row: Option<PendingReviewRow> = conn
        .query_row(
            "SELECT id, word_id, sense_id, hints_used, completed_at FROM reviews WHERE id = ?1",
            [review_id.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    match row {
        None => Err(ReviewError::NotFound(review_id.0)),
        Some((id, _, _, _, Some(_))) => Err(ReviewError::AlreadyCompleted(id)),
        Some((_, word_id, sense_id, hints_used, None)) => Ok(PendingReview {
            word_id: WordId(word_id),
            sense_id: sense_id.map(SenseId),
            hints_used: hints_used.max(0) as u32,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_conn;
    use crate::dictionary::model::LookupOutcome;
    use crate::dictionary::provider::{DictionaryProvider, SeedProvider};
    use crate::memory::model::RecallQuality;
    use crate::memory::service::MemoryService as Mem;

    fn seeded_word(conn: &Connection, word: &str) -> WordId {
        match engine::lookup(conn, word).unwrap() {
            LookupOutcome::Hit { entry } => entry.id,
            LookupOutcome::Miss { .. } => panic!("no seed word {word}"),
        }
    }

    /// 造一个到期词：3 天前的弱查词。
    fn make_due(conn: &Connection, word: &str) -> WordId {
        let id = seeded_word(conn, word);
        let now = Utc::now();
        Mem::default()
            .open_visit(conn, id, now - chrono::Duration::days(3))
            .unwrap();
        id
    }

    #[test]
    fn study_fallback_quota_priorities_and_read_only_browsing() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let now = Utc::now();
        let due = make_due(&conn, "meticulous");
        let today = seeded_word(&conn, "subtle");
        Mem::default().open_visit(&conn, today, now).unwrap();
        let ignored = seeded_word(&conn, "acquire");
        Mem::default().set_ignored(&conn, ignored, true).unwrap();
        for priority in [
            StudyPriority::Mixed,
            StudyPriority::NewFirst,
            StudyPriority::ReviewFirst,
        ] {
            let group = ReviewService
                .study_group(&conn, "cet4", 3, priority, now)
                .unwrap();
            assert_eq!(group.today_learned_new, 1);
            assert_eq!(group.new_count, 2);
            assert_eq!(group.due_count, 1);
            assert_eq!(group.cards.len(), 3);
            assert!(!group
                .cards
                .iter()
                .any(|c| c.entry.id == today || c.entry.id == ignored));
            assert_eq!(group.cards.iter().filter(|c| c.is_new).count(), 2);
            assert_eq!(group.cards[0].is_new, priority == StudyPriority::NewFirst);
            assert!(group.cards.iter().any(|c| c.entry.id == due && !c.is_new));
        }
        let exhausted = ReviewService
            .study_group(&conn, "cet4", 0, StudyPriority::Mixed, now)
            .unwrap();
        assert_eq!(exhausted.new_count, 0);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM encounters", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM reviews", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert!(ReviewService
            .study_group(&conn, "unknown", 20, StudyPriority::Mixed, now)
            .is_err());
    }

    #[test]
    fn study_tagged_oxford_custom_and_encountered_books() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        conn.execute_batch("UPDATE words SET tags = '', oxford = 0;")
            .unwrap();
        let tagged = seeded_word(&conn, "meticulous");
        let oxford = seeded_word(&conn, "subtle");
        conn.execute(
            "UPDATE words SET tags = 'CET4 cet6' WHERE id = ?1",
            [tagged.0],
        )
        .unwrap();
        conn.execute("UPDATE words SET oxford = 1 WHERE id = ?1", [oxford.0])
            .unwrap();
        let now = Utc::now();
        let group = ReviewService
            .study_group(&conn, "cet4", 20, StudyPriority::Mixed, now)
            .unwrap();
        assert_eq!(group.cards.len(), 1);
        assert_eq!(group.cards[0].entry.id, tagged);
        let group = ReviewService
            .study_group(&conn, "oxford", 20, StudyPriority::Mixed, now)
            .unwrap();
        assert_eq!(group.cards.len(), 1);
        assert_eq!(group.cards[0].entry.id, oxford);
        ReviewService
            .custom_wordbook_toggle(&conn, tagged, true)
            .unwrap();
        ReviewService
            .custom_wordbook_toggle(&conn, tagged, true)
            .unwrap();
        crate::library::save_note(&conn, oxford.0, true, "note").unwrap();
        let group = ReviewService
            .study_group(&conn, "custom", 20, StudyPriority::Mixed, now)
            .unwrap();
        assert_eq!(group.cards.len(), 2);
        assert!(group
            .cards
            .iter()
            .any(|c| c.entry.id == tagged && c.in_custom_wordbook));
        assert!(group
            .cards
            .iter()
            .any(|c| c.entry.id == oxford && c.bookmarked));
        let books = ReviewService.wordbooks_list(&conn).unwrap();
        assert_eq!(books.len(), 9);
        assert_eq!(
            books.iter().find(|b| b.id == "custom").unwrap().total_words,
            2
        );
        ReviewService
            .custom_wordbook_toggle(&conn, tagged, false)
            .unwrap();
        assert!(ReviewService
            .study_group(&conn, "encountered", 20, StudyPriority::Mixed, now)
            .unwrap()
            .cards
            .is_empty());
        Mem::default()
            .open_visit(&conn, tagged, now - chrono::Duration::days(3))
            .unwrap();
        let group = ReviewService
            .study_group(&conn, "encountered", 20, StudyPriority::Mixed, now)
            .unwrap();
        assert_eq!(group.cards.len(), 1);
        assert!(!group.cards[0].is_new);
        let books = ReviewService.wordbooks_list(&conn).unwrap();
        assert_eq!(
            books.iter().find(|b| b.id == "cet4").unwrap().learned_words,
            1
        );
    }

    #[test]
    fn study_grades_create_atomic_events_and_count_first_visit_once() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let id = seeded_word(&conn, "meticulous");
        let now = Utc::now();
        for (grade, quality, correct) in [
            (StudyGrade::Know, RecallQuality::Good, true),
            (StudyGrade::Hazy, RecallQuality::Hard, true),
            (StudyGrade::Forgot, RecallQuality::Forgotten, false),
        ] {
            let feedback = ReviewService.study_grade(&conn, id, grade, now).unwrap();
            assert_eq!(feedback.quality, quality);
            assert_eq!(feedback.correct, correct);
            assert!(feedback.memory.is_some());
        }
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM encounters", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM reviews WHERE completed_at IS NOT NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            3
        );
        assert_eq!(
            conn.query_row("SELECT SUM(correct) FROM review_attempts", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        let group = ReviewService
            .study_group(&conn, "cet4", 20, StudyPriority::Mixed, now)
            .unwrap();
        assert_eq!(group.today_learned_new, 1);
        assert!(group.cards.len() <= 20);
        assert!(ReviewService
            .study_grade(&conn, WordId(-1), StudyGrade::Know, now)
            .is_err());
    }

    #[test]
    fn study_session_limit_and_utc_day_boundary() {
        let conn = test_conn();
        let now = DateTime::parse_from_rfc3339("2026-10-20T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        for i in 0..50 {
            conn.execute(
                "INSERT INTO words (word, display, source, created_at) VALUES (?1, ?1, 'seed', ?2)",
                params![format!("studyword{i}"), fmt_ts(now)],
            )
            .unwrap();
            let id = WordId(conn.last_insert_rowid());
            conn.execute(
                "INSERT INTO senses (word_id, english_definition) VALUES (?1, 'A vocabulary test word')",
                [id.0],
            ).unwrap();
            if i < 25 {
                Mem::default()
                    .open_visit(&conn, id, now - chrono::Duration::days(3))
                    .unwrap();
            } else if i == 25 {
                Mem::default().open_visit(&conn, id, now).unwrap();
            } else if i == 26 {
                Mem::default()
                    .open_visit(&conn, id, now - chrono::Duration::milliseconds(1))
                    .unwrap();
            }
        }
        for priority in [
            StudyPriority::Mixed,
            StudyPriority::NewFirst,
            StudyPriority::ReviewFirst,
        ] {
            let group = ReviewService
                .study_group(&conn, "cet4", 100, priority, now)
                .unwrap();
            assert_eq!(group.cards.len(), 20);
            assert_eq!(group.today_learned_new, 1);
            let new_count = group.cards.iter().filter(|c| c.is_new).count();
            assert_eq!(
                new_count,
                match priority {
                    StudyPriority::Mixed => 10,
                    StudyPriority::NewFirst => 20,
                    StudyPriority::ReviewFirst => 0,
                }
            );
            let ids: std::collections::HashSet<_> =
                group.cards.iter().map(|c| c.entry.id).collect();
            assert_eq!(ids.len(), 20);
        }
    }

    #[test]
    fn study_grade_rolls_back_initial_visit_if_review_write_fails() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let id = seeded_word(&conn, "meticulous");
        conn.execute_batch("CREATE TRIGGER reject_study_review BEFORE INSERT ON reviews BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
        assert!(ReviewService
            .study_grade(&conn, id, StudyGrade::Know, Utc::now())
            .is_err());
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM encounters", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert!(Mem::default().load_state(&conn, id).unwrap().is_none());
    }

    #[test]
    fn chinese_only_words_still_have_review_prompts() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let id = make_due(&conn, "meticulous");
        conn.execute(
            "DELETE FROM examples WHERE sense_id IN (SELECT id FROM senses WHERE word_id=?1)",
            [id.0],
        )
        .unwrap();
        conn.execute(
            "UPDATE senses SET english_definition='' WHERE word_id=?1",
            [id.0],
        )
        .unwrap();
        let queue = ReviewService.queue(&conn, Utc::now()).unwrap();
        let item = queue.iter().find(|item| item.word_id == id).unwrap();
        assert!(!item.prompt.is_empty());
        assert!(!item.prompt.contains("meticulous"));
    }

    #[test]
    fn queue_contains_only_eligible_words_with_prompt() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let now = Utc::now();

        let due = make_due(&conn, "meticulous");
        // 未到期词不应出现
        let future = seeded_word(&conn, "subtle");
        Mem::default().open_visit(&conn, future, now).unwrap();

        let items = ReviewService.queue(&conn, now).unwrap();
        assert!(items.iter().any(|i| i.word_id == due), "{items:?}");
        assert!(!items.iter().any(|i| i.word_id == future), "{items:?}");
        // 队列条目不含答案；复习本地兜底为释义题。
        let item = items.iter().find(|i| i.word_id == due).unwrap();
        assert_eq!(item.kind, crate::review::model::PromptKind::DefinitionZh);
        assert!(!item.prompt.contains("meticulous"), "{}", item.prompt);
    }

    #[test]
    fn submit_correct_with_hints_updates_memory() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let now = Utc::now();
        let word_id = make_due(&conn, "meticulous");
        let svc = ReviewService;

        let review_id = svc.start(&conn, word_id, None, None, now).unwrap();
        svc.hint(&conn, review_id, 1, now).unwrap();
        svc.hint(&conn, review_id, 2, now).unwrap();

        let before = Mem::default().load_state(&conn, word_id).unwrap().unwrap();
        let fb = svc
            .submit(&conn, review_id, Some("meticulous".into()), now)
            .unwrap();
        assert!(fb.correct);
        assert_eq!(fb.quality, RecallQuality::Hard); // 2 条提示
        assert_eq!(fb.answer, "meticulous");
        let after = fb.memory.clone().unwrap();
        assert!(after.strength_days > before.strength_days, "答对应增长 S");

        // 已完成的 review 不能再提交
        let err = svc
            .submit(&conn, review_id, Some("x".into()), now)
            .unwrap_err();
        assert!(matches!(err, ReviewError::AlreadyCompleted(_)));

        // 事件流包含 ReviewDone；投影一致
        let events = crate::memory::store::load_events(&conn, word_id).unwrap();
        assert_eq!(events.len(), 2); // 1 visit + 1 review
    }

    #[test]
    fn submit_wrong_or_reveal_is_forgotten() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let now = Utc::now();
        let word_id = make_due(&conn, "tentative");
        let svc = ReviewService;

        // 答错
        let rid = svc.start(&conn, word_id, None, None, now).unwrap();
        let fb = svc
            .submit(&conn, rid, Some("careless".into()), now)
            .unwrap();
        assert!(!fb.correct);
        assert_eq!(fb.quality, RecallQuality::Forgotten);
        let after = fb.memory.unwrap();
        assert_eq!(after.lapse_count, 1);
        // Forgotten 后 S 缩小、很快再到期
        assert!(after.next_review_at.unwrap() <= now + chrono::Duration::hours(12));

        // 放弃（reveal）
        let rid2 = svc.start(&conn, word_id, None, None, now).unwrap();
        let fb2 = svc.submit(&conn, rid2, None, now).unwrap();
        assert!(!fb2.correct);
        assert_eq!(fb2.quality, RecallQuality::Forgotten);
        assert_eq!(fb2.hints_used, 0);
    }

    #[test]
    fn typo_distance_one_accepted() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let now = Utc::now();
        let word_id = make_due(&conn, "subtle");
        let svc = ReviewService;

        let rid = svc.start(&conn, word_id, None, None, now).unwrap();
        let fb = svc.submit(&conn, rid, Some("Subtl".into()), now).unwrap();
        assert!(fb.correct, "编辑距离 1 应判对");
    }

    #[test]
    fn review_keeps_prompt_sense_for_hints() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let now = Utc::now();
        let entry = match engine::lookup(&conn, "elaborate").unwrap() {
            LookupOutcome::Hit { entry } => entry,
            LookupOutcome::Miss { .. } => panic!("expected seeded word"),
        };
        let sense = &entry.senses[1];
        let example_id = sense.examples.first().map(|example| example.id);
        let rid = ReviewService
            .start(&conn, entry.id, Some(sense.id), example_id, now)
            .unwrap();

        ReviewService.hint(&conn, rid, 1, now).unwrap();
        match ReviewService.hint(&conn, rid, 2, now).unwrap() {
            Hint::English { text } => assert_eq!(text, sense.english_definition),
            other => panic!("expected english hint, got {other:?}"),
        }
        let stored_example: Option<i64> = conn
            .query_row(
                "SELECT example_id FROM reviews WHERE id = ?1",
                [rid.0],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_example, example_id.map(|id| id.0));
    }

    #[test]
    fn hint_flow_and_validation() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let now = Utc::now();
        let word_id = make_due(&conn, "rigorous");
        let svc = ReviewService;

        let rid = svc.start(&conn, word_id, None, None, now).unwrap();
        assert!(matches!(
            svc.hint(&conn, rid, 2, now).unwrap_err(),
            ReviewError::UnexpectedHint {
                requested: 2,
                expected: 1
            }
        ));
        match svc.hint(&conn, rid, 1, now).unwrap() {
            Hint::Mask { text } => assert!(text.starts_with('r') && text.contains('_')),
            other => panic!("expected mask, got {other:?}"),
        }
        match svc.hint(&conn, rid, 2, now).unwrap() {
            Hint::English { text } => assert!(text.contains("careful")),
            other => panic!("expected english, got {other:?}"),
        }
        match svc.hint(&conn, rid, 3, now).unwrap() {
            Hint::Chinese { text } => assert!(!text.is_empty()),
            other => panic!("expected chinese, got {other:?}"),
        }
        // 无效提示号
        assert!(matches!(
            svc.hint(&conn, rid, 0, now).unwrap_err(),
            ReviewError::InvalidHint(0)
        ));
        assert!(matches!(
            svc.hint(&conn, rid, 4, now).unwrap_err(),
            ReviewError::InvalidHint(4)
        ));
        // 不存在的 review
        assert!(matches!(
            svc.hint(&conn, ReviewId(9999), 1, now).unwrap_err(),
            ReviewError::NotFound(9999)
        ));
        assert!(matches!(
            svc.submit(&conn, ReviewId(9999), None, now).unwrap_err(),
            ReviewError::NotFound(9999)
        ));
    }

    // ── 只读复习组 ────────────────────────────────────────────

    #[test]
    fn group_is_read_only_and_capped_at_session_limit() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let now = Utc::now();
        // 11 个到期词：3 天前的弱查词
        let mut due_ids = Vec::new();
        for word in [
            "meticulous",
            "subtle",
            "acquire",
            "elaborate",
            "inevitable",
            "reluctant",
            "compelling",
            "profound",
            "ambiguous",
            "resilient",
            "pragmatic",
        ] {
            let id = seeded_word(&conn, word);
            Mem::default()
                .open_visit(&conn, id, now - chrono::Duration::days(3))
                .unwrap();
            due_ids.push(id);
        }
        let encounters_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM encounters", [], |r| r.get(0))
            .unwrap();
        let reviews_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM reviews", [], |r| r.get(0))
            .unwrap();

        let group = ReviewService.group(&conn, now).unwrap();
        assert_eq!(group.len(), crate::memory::service::REVIEW_SESSION_LIMIT);
        assert_eq!(group.len(), 10, "组上限 = 10");
        // 组内词条完整且与题目同源
        for g in &group {
            assert_eq!(g.entry.id, g.item.word_id);
            assert!(!g.entry.senses.is_empty());
            assert!(!g.item.prompt.trim().is_empty());
        }

        // 只读：encounters 与 reviews 均无新增，记忆投影不变
        let encounters_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM encounters", [], |r| r.get(0))
            .unwrap();
        let reviews_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM reviews", [], |r| r.get(0))
            .unwrap();
        assert_eq!(encounters_before, encounters_after);
        assert_eq!(reviews_before, reviews_after);

        // 单词组：只 1 个到期词 → 组大小 1
        let single = test_conn();
        SeedProvider.import(&single).unwrap();
        let id = seeded_word(&single, "meticulous");
        Mem::default()
            .open_visit(&single, id, now - chrono::Duration::days(3))
            .unwrap();
        let group = ReviewService.group(&single, now).unwrap();
        assert_eq!(group.len(), 1);

        // 空组：无到期词 → 0
        let none = test_conn();
        SeedProvider.import(&none).unwrap();
        assert!(ReviewService.group(&none, now).unwrap().is_empty());
    }

    #[test]
    fn group_fallback_uses_definitions_even_when_dictionary_examples_exist() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let now = Utc::now();
        for word in ["meticulous", "subtle"] {
            let id = seeded_word(&conn, word);
            Mem::default()
                .open_visit(&conn, id, now - chrono::Duration::days(3))
                .unwrap();
        }
        let group = ReviewService.group(&conn, now).unwrap();
        for g in &group {
            assert_eq!(g.item.kind, crate::review::model::PromptKind::DefinitionZh);
            assert_eq!(
                g.item.source,
                crate::review::model::PromptSource::Dictionary
            );
            assert!(g.item.example_id.is_none());
            assert!(!g.item.prompt.contains(&g.entry.word), "答案不得留在题目里");
        }
    }

    #[test]
    fn practice_check_and_hint_are_read_only() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let word_id = make_due(&conn, "meticulous");
        let svc = ReviewService;

        let state_before = Mem::default().load_state(&conn, word_id).unwrap().unwrap();
        let reviews_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM reviews", [], |r| r.get(0))
            .unwrap();
        let attempts_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM review_attempts", [], |r| r.get(0))
            .unwrap();

        // 判分：对/错/拼写容错/揭示（None）
        assert!(svc
            .practice_check(&conn, word_id, Some("meticulous".into()))
            .unwrap());
        assert!(!svc
            .practice_check(&conn, word_id, Some("subtle".into()))
            .unwrap());
        assert!(
            svc.practice_check(&conn, word_id, Some("meticulus".into()))
                .unwrap(),
            "编辑距离 1 应判对"
        );
        assert!(!svc.practice_check(&conn, word_id, None).unwrap());

        // 提示：1..=3 有效，越界拒绝
        assert!(matches!(
            svc.practice_hint(&conn, word_id, None, 1).unwrap(),
            Hint::Mask { .. }
        ));
        assert!(matches!(
            svc.practice_hint(&conn, word_id, None, 2).unwrap(),
            Hint::English { .. }
        ));
        assert!(matches!(
            svc.practice_hint(&conn, word_id, None, 3).unwrap(),
            Hint::Chinese { .. }
        ));
        assert!(matches!(
            svc.practice_hint(&conn, word_id, None, 0).unwrap_err(),
            ReviewError::InvalidHint(0)
        ));

        // 全程零写入
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM reviews", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            reviews_before
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM review_attempts", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            attempts_before
        );
        let state_after = Mem::default().load_state(&conn, word_id).unwrap().unwrap();
        assert_eq!(state_before, state_after);
    }
}
