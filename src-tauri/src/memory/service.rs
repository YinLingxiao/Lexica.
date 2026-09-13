//! Encounter 策略 + 记忆投影 + 复习队列 + 统计。
//!
//! 一切写路径（开 visit / 升级理解级别 / 完成复习）都会重折该词并 UPSERT memory_states。
//! memory_states 永远 = fold(该词事件流)，可随时全量重建。

use chrono::{DateTime, Duration, Utc};
use rusqlite::Connection;
use serde::Serialize;

use super::model::{MemoryState, MemoryStatus, RecentVisit};
use super::scheduler::{MemoryScheduler, V1Scheduler};
use super::{store, MemoryError};
use crate::domain::{ComprehensionLevel, EncounterId, WordId};

/// 同词 30 分钟内的重复查词合并为同一 visit（防短期重复污染记忆信号）。
pub const VISIT_MERGE_WINDOW_MS: i64 = 30 * 60_000;

/// 复习队列的呈现过滤：只显示最近有活动的词，防陈年词淹没计数。
pub const QUEUE_ACTIVE_WINDOW_MS: i64 = 60 * 86_400_000;

/// 单次复习会话上限（无每日任务——这是上限，不是目标）。
pub const REVIEW_SESSION_LIMIT: usize = 12;

/// 到期词（fading 列表 / 统计用；不含词文本——由 commands 从 dictionary 补齐）。
#[derive(Debug, Clone, Serialize)]
pub struct DueWord {
    pub word_id: WordId,
    pub high_priority: bool,
    pub next_review_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsSummary {
    pub encountered: usize,
    pub learning: usize,
    pub familiar: usize,
    pub stable: usize,
    pub ignored: usize,
    pub due_now: usize,
    pub high_priority: usize,
}

#[derive(Default)]
pub struct MemoryService {
    pub scheduler: V1Scheduler,
}

impl MemoryService {
    // ── visit ──────────────────────────────────────────────

    /// 查词即 Encounter：开一次 visit（或并入最近一次），随后重折该词。
    pub fn open_visit(
        &self,
        conn: &Connection,
        word_id: WordId,
        now: DateTime<Utc>,
    ) -> Result<EncounterId, MemoryError> {
        let id = match store::latest_visit(conn, word_id)? {
            Some(latest)
                if now >= latest.visited_at
                    && now - latest.visited_at < Duration::milliseconds(VISIT_MERGE_WINDOW_MS) =>
            {
                store::update_visit_time(conn, latest.id, now)?;
                latest.id
            }
            _ => store::insert_visit(conn, word_id, now)?,
        };
        self.refresh_word(conn, word_id, now)?;
        Ok(id)
    }

    /// 记录理解级别（策略：最新 visit、单调升级、同级幂等），随后重折。
    pub fn set_comprehension(
        &self,
        conn: &Connection,
        encounter_id: EncounterId,
        level: ComprehensionLevel,
        now: DateTime<Utc>,
    ) -> Result<(), MemoryError> {
        let encounter = store::get(conn, encounter_id)?
            .ok_or(MemoryError::EncounterNotFound(encounter_id.0))?;

        let latest = store::latest_visit(conn, encounter.word_id)?;
        if latest.map(|l| l.id) != Some(encounter_id) {
            return Err(MemoryError::NotLatestEncounter(encounter_id.0));
        }

        if level != encounter.comprehension_level {
            let escalates = encounter.comprehension_level == ComprehensionLevel::Unknown
                || level.help_rank() > encounter.comprehension_level.help_rank();
            if !escalates {
                return Err(MemoryError::NotAnEscalation {
                    from: encounter.comprehension_level.as_str().to_string(),
                    to: level.as_str().to_string(),
                });
            }
            store::update_level(conn, encounter_id, level)?;
        }
        self.refresh_word(conn, encounter.word_id, now)?;
        Ok(())
    }

    /// 每词最新一次 visit，按时间倒序。
    pub fn recent_visits(
        &self,
        conn: &Connection,
        limit: u8,
    ) -> Result<Vec<RecentVisit>, MemoryError> {
        store::recent_visits(conn, limit)
    }

    // ── 投影 ────────────────────────────────────────────────

    /// 重折一个词并 UPSERT 投影。无事件 → 无状态（不写行）。
    pub fn refresh_word(
        &self,
        conn: &Connection,
        word_id: WordId,
        now: DateTime<Utc>,
    ) -> Result<Option<MemoryState>, MemoryError> {
        let events = store::load_events(conn, word_id)?;
        let state = self.scheduler.fold(word_id, &events);
        if let Some(s) = &state {
            store::upsert_state(conn, s, now)?;
        }
        Ok(state)
    }

    /// 全量重建投影（开发/算法升级用）。
    pub fn rebuild_projection(
        &self,
        conn: &Connection,
        now: DateTime<Utc>,
    ) -> Result<usize, MemoryError> {
        let mut count = 0;
        for word_id in store::words_with_events(conn)? {
            self.refresh_word(conn, word_id, now)?;
            count += 1;
        }
        Ok(count)
    }

    pub fn load_state(
        &self,
        conn: &Connection,
        word_id: WordId,
    ) -> Result<Option<MemoryState>, MemoryError> {
        store::load_state(conn, word_id)
    }

    // ── 复习队列 ────────────────────────────────────────────

    /// 到期词（已做 60 天呈现过滤与排序），不设上限——fading 列表 / 统计用。
    pub fn due_words(
        &self,
        conn: &Connection,
        now: DateTime<Utc>,
    ) -> Result<Vec<DueWord>, MemoryError> {
        let rows = store::due_rows(
            conn,
            now,
            now - Duration::milliseconds(QUEUE_ACTIVE_WINDOW_MS),
        )?;
        let mut scored: Vec<(DueWord, f64)> = rows
            .into_iter()
            .map(|row| {
                // 只为计算 R 构造最小状态（retrievability 只看 anchor + S）
                let r = row.last_reinforcement_at.map_or(0.0, |anchor| {
                    let partial = MemoryState {
                        word_id: row.word_id,
                        strength_days: row.strength_days,
                        difficulty: row.difficulty,
                        review_count: 0,
                        lapse_count: 0,
                        visit_count: 0,
                        weak_streak: 0,
                        high_priority: row.high_priority,
                        first_visit_at: now,
                        last_visit_at: None,
                        last_review_at: None,
                        last_reinforcement_at: Some(anchor),
                        next_review_at: None,
                    };
                    self.scheduler.retrievability(&partial, now)
                });
                (
                    DueWord {
                        word_id: row.word_id,
                        high_priority: row.high_priority,
                        next_review_at: row.next_review_at,
                    },
                    r,
                )
            })
            .collect();
        scored.sort_by(|a, b| {
            b.0.high_priority
                .cmp(&a.0.high_priority)
                .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        });
        Ok(scored.into_iter().map(|(d, _)| d).collect())
    }

    /// 复习队列：到期词截断到会话上限。
    /// 只返回 WordId——cloze 素材由 review 模块补齐，保持解耦。
    pub fn review_queue(
        &self,
        conn: &Connection,
        now: DateTime<Utc>,
    ) -> Result<Vec<WordId>, MemoryError> {
        Ok(self
            .due_words(conn, now)?
            .into_iter()
            .take(REVIEW_SESSION_LIMIT)
            .map(|d| d.word_id)
            .collect())
    }

    // ── 统计 ────────────────────────────────────────────────

    pub fn stats_summary(
        &self,
        conn: &Connection,
        now: DateTime<Utc>,
    ) -> Result<StatsSummary, MemoryError> {
        let active_since = now - Duration::milliseconds(QUEUE_ACTIVE_WINDOW_MS);
        let mut summary = StatsSummary {
            encountered: 0,
            learning: 0,
            familiar: 0,
            stable: 0,
            ignored: 0,
            due_now: 0,
            high_priority: 0,
        };
        for row in store::stats_rows(conn)? {
            summary.encountered += 1;
            if row.ignored {
                summary.ignored += 1;
                continue;
            }
            match MemoryStatus::from_strength(row.strength_days) {
                MemoryStatus::Learning => summary.learning += 1,
                MemoryStatus::Familiar => summary.familiar += 1,
                MemoryStatus::Stable => summary.stable += 1,
            }
            if row.high_priority {
                summary.high_priority += 1;
            }
            let is_due = row.next_review_at.is_some_and(|due| due <= now);
            let active = row.last_visit_at.is_some_and(|t| t >= active_since)
                || row.last_review_at.is_some_and(|t| t >= active_since);
            if is_due && active {
                summary.due_now += 1;
            }
        }
        Ok(summary)
    }

    // ── ignore ──────────────────────────────────────────────

    pub fn set_ignored(
        &self,
        conn: &Connection,
        word_id: WordId,
        ignored: bool,
    ) -> Result<(), MemoryError> {
        store::set_ignored(conn, word_id, ignored)
    }

    pub fn is_ignored(&self, conn: &Connection, word_id: WordId) -> Result<bool, MemoryError> {
        store::is_ignored(conn, word_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_conn;
    use crate::dictionary::engine::lookup;
    use crate::dictionary::model::LookupOutcome;
    use crate::dictionary::provider::{DictionaryProvider, SeedProvider};

    fn seeded_word(conn: &Connection, word: &str) -> WordId {
        match lookup(conn, word).unwrap() {
            LookupOutcome::Hit { entry } => entry.id,
            LookupOutcome::Miss { .. } => panic!("no seed word {word}"),
        }
    }

    #[test]
    fn open_visit_creates_projection_at_current_level() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let word_id = seeded_word(&conn, "subtle");
        let now = Utc::now();
        let svc = MemoryService::default();

        // 开 visit（unknown）→ 投影以当前级别 unknown 折算
        svc.open_visit(&conn, word_id, now).unwrap();
        let s = svc.load_state(&conn, word_id).unwrap().unwrap();
        assert_eq!(s.visit_count, 1);
        assert_eq!(s.weak_streak, 1);
        assert!((s.strength_days - 0.25).abs() < 1e-9); // s0[unknown]

        // 披露到 english → 重折自洽（同一 visit 按最终级别计）
        let enc_id = store::latest_visit(&conn, word_id).unwrap().unwrap().id;
        svc.set_comprehension(&conn, enc_id, ComprehensionLevel::EnglishDefinition, now)
            .unwrap();
        let s2 = svc.load_state(&conn, word_id).unwrap().unwrap();
        assert!((s2.strength_days - 1.0).abs() < 1e-9); // s0[english]
        assert_eq!(s2.weak_streak, 0);
    }

    #[test]
    fn stored_projection_matches_full_refold() {
        // 增量维护的投影 == 从事件流全量重折（fold_rebuild 一致性）
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let word_id = seeded_word(&conn, "acquire");
        let svc = MemoryService::default();
        let t0 = Utc::now();

        svc.open_visit(&conn, word_id, t0).unwrap();
        svc.open_visit(&conn, word_id, t0 + Duration::hours(3))
            .unwrap();
        let enc = store::latest_visit(&conn, word_id).unwrap().unwrap().id;
        svc.set_comprehension(
            &conn,
            enc,
            ComprehensionLevel::ChineseDefinition,
            t0 + Duration::hours(3),
        )
        .unwrap();

        let stored = svc.load_state(&conn, word_id).unwrap().unwrap();
        let events = store::load_events(&conn, word_id).unwrap();
        let refolded = svc.scheduler.fold(word_id, &events).unwrap();
        assert_eq!(stored, refolded);
    }

    #[test]
    fn queue_only_contains_due_unignored_words() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let svc = MemoryService::default();
        let now = Utc::now();

        let due_word = seeded_word(&conn, "subtle");
        let future_word = seeded_word(&conn, "acquire");
        let ignored_word = seeded_word(&conn, "tentative");

        // 3 天前的弱查词 → 已到期
        svc.open_visit(&conn, due_word, now - Duration::days(3))
            .unwrap();
        // 刚查的词 → 未到期
        svc.open_visit(&conn, future_word, now).unwrap();
        // 3 天前查的词但被 ignore → 排除
        svc.open_visit(&conn, ignored_word, now - Duration::days(3))
            .unwrap();
        svc.set_ignored(&conn, ignored_word, true).unwrap();

        let queue = svc.review_queue(&conn, now).unwrap();
        assert!(queue.contains(&due_word), "{queue:?}");
        assert!(!queue.contains(&future_word), "{queue:?}");
        assert!(!queue.contains(&ignored_word), "{queue:?}");

        // 解除 ignore → 回到队列
        svc.set_ignored(&conn, ignored_word, false).unwrap();
        let queue = svc.review_queue(&conn, now).unwrap();
        assert!(queue.contains(&ignored_word));
    }

    #[test]
    fn queue_excludes_stale_words() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let svc = MemoryService::default();
        let now = Utc::now();

        // 90 天前查过一次 → 到期但无近期活动，不呈现
        let stale = seeded_word(&conn, "versatile");
        svc.open_visit(&conn, stale, now - Duration::days(90))
            .unwrap();

        let queue = svc.review_queue(&conn, now).unwrap();
        assert!(!queue.contains(&stale), "{queue:?}");
    }

    #[test]
    fn queue_orders_priority_then_retrievability() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let svc = MemoryService::default();
        let now = Utc::now();

        // 高优先级词：4 次弱查词（9 天内）
        let priority = seeded_word(&conn, "undermine");
        for d in [9u32, 5, 3, 1] {
            svc.open_visit(&conn, priority, now - Duration::days(i64::from(d)))
                .unwrap();
        }
        // 普通到期词：10 天前查过一次（R 更低）
        let old_normal = seeded_word(&conn, "feasible");
        svc.open_visit(&conn, old_normal, now - Duration::days(10))
            .unwrap();
        // 普通到期词：2 天前查过一次（R 更高）
        let recent_normal = seeded_word(&conn, "coherent");
        svc.open_visit(&conn, recent_normal, now - Duration::days(2))
            .unwrap();

        let queue = svc.review_queue(&conn, now).unwrap();
        let pos = |id: WordId| queue.iter().position(|q| *q == id).unwrap();
        assert_eq!(queue[0], priority, "高优先级排最前");
        assert!(
            pos(old_normal) < pos(recent_normal),
            "R 更低（更久未强化）的排更前: {queue:?}"
        );
    }

    #[test]
    fn stats_buckets_and_due_count() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let svc = MemoryService::default();
        let now = Utc::now();

        let a = seeded_word(&conn, "subtle");
        let b = seeded_word(&conn, "acquire");
        let c = seeded_word(&conn, "tentative");

        svc.open_visit(&conn, a, now - Duration::days(3)).unwrap();
        svc.open_visit(&conn, b, now).unwrap();
        svc.open_visit(&conn, c, now - Duration::days(3)).unwrap();
        svc.set_ignored(&conn, c, true).unwrap();

        let stats = svc.stats_summary(&conn, now).unwrap();
        assert_eq!(stats.encountered, 3);
        assert_eq!(stats.learning, 2); // a、b（ignore 的不计桶）
        assert_eq!(stats.ignored, 1);
        assert_eq!(stats.due_now, 1); // 只有 a（b 未到期，c 被 ignore）
    }

    #[test]
    fn rebuild_projection_is_idempotent() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let svc = MemoryService::default();
        let now = Utc::now();

        let a = seeded_word(&conn, "subtle");
        svc.open_visit(&conn, a, now - Duration::days(2)).unwrap();

        let before = svc.load_state(&conn, a).unwrap().unwrap();
        let n = svc.rebuild_projection(&conn, now).unwrap();
        assert_eq!(n, 1);
        let after = svc.load_state(&conn, a).unwrap().unwrap();
        assert_eq!(before, after);
    }

    // ── visit 合并与升级规则（阶段三用例在 service 上复验一遍端到端） ──

    #[test]
    fn visits_merge_and_escalate_end_to_end() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let word_id = seeded_word(&conn, "rigorous");
        let svc = MemoryService::default();
        let t0 = Utc::now();

        let id1 = svc.open_visit(&conn, word_id, t0).unwrap();
        let id2 = svc
            .open_visit(&conn, word_id, t0 + Duration::minutes(10))
            .unwrap();
        assert_eq!(id1, id2);

        svc.set_comprehension(&conn, id1, ComprehensionLevel::EnglishDefinition, t0)
            .unwrap();
        svc.set_comprehension(&conn, id1, ComprehensionLevel::ChineseDefinition, t0)
            .unwrap();

        let s = svc.load_state(&conn, word_id).unwrap().unwrap();
        assert_eq!(s.visit_count, 1, "合并窗口内只算一次 visit");
        assert_eq!(s.weak_streak, 1, "最终按 chinese 级折算");
    }
}
