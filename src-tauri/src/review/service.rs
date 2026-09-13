//! 复习会话编排：队列构建、逐级提示、判分落库、记忆更新。
//! SQL 直接写在这里（review 模块语句少，不再分 store 层）。

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use super::grader;
use super::model::{Hint, ReviewFeedback, ReviewItem};
use super::ReviewError;
use crate::database::fmt_ts;
use crate::dictionary::engine;
use crate::domain::{ExampleId, ReviewId, SenseId, WordId};
use crate::memory::service::MemoryService;

pub struct ReviewService;

/// 队列内一行（reviews 表的 pending 行快照）。
struct PendingReview {
    word_id: WordId,
    sense_id: Option<SenseId>,
    hints_used: u32,
}

impl ReviewService {
    /// 复习队列：memory 提供到期 WordId，dictionary 补 cloze 素材。
    /// 队列条目不含答案。
    pub fn queue(
        &self,
        conn: &Connection,
        now: DateTime<Utc>,
    ) -> Result<Vec<ReviewItem>, ReviewError> {
        let ids = MemoryService::default().due_words(conn, now)?;
        let mut items = Vec::with_capacity(crate::memory::service::REVIEW_SESSION_LIMIT);
        for due in ids {
            let id = due.word_id;
            if let Some(entry) = engine::entry_by_id(conn, id)? {
                if entry.senses.is_empty() {
                    continue;
                }
                let cloze = grader::build_cloze(&entry);
                if cloze.text.trim().is_empty() {
                    continue; // ECDICT 词条可能既无例句也无英文释义，空 prompt 无法作答
                }
                items.push(ReviewItem {
                    word_id: id,
                    prompt: cloze.text,
                    sense_id: cloze.sense_id,
                    example_id: cloze.example_id,
                });
                if items.len() >= crate::memory::service::REVIEW_SESSION_LIMIT {
                    break;
                }
            }
        }
        Ok(items)
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
        let sense = match review.sense_id {
            Some(id) => entry.senses.iter().find(|sense| sense.id == id),
            None => entry.senses.first(),
        }
        .ok_or(ReviewError::NoWordData)?;

        let hint = match hint_no {
            1 => Hint::Mask {
                text: grader::mask_hint(&entry.display),
            },
            2 => Hint::English {
                text: sense.english_definition.clone(),
            },
            _ => Hint::Chinese {
                text: sense.chinese_definition.clone().unwrap_or_default(),
            },
        };

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
        // 队列条目不含答案：meticulous 的 cloze 里词已被挖空
        let item = items.iter().find(|i| i.word_id == due).unwrap();
        assert!(item.prompt.contains("______"), "{}", item.prompt);
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
}
