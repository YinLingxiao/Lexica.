//! encounters / memory_states / ignored_words 的 SQL 层。
//! 合并窗口与单调升级等策略在 service；折算算法在 scheduler。

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use super::model::{
    Encounter, EncounterSource, MemoryEvent, MemoryState, RecallQuality, RecentVisit,
};
use super::MemoryError;
use crate::database::{fmt_ts, parse_ts};
use crate::domain::{ComprehensionLevel, EncounterId, WordId};

/// store 内部用的行结构（策略判断需要 visited_at，无需完整 Encounter）。
pub struct LatestVisit {
    pub id: EncounterId,
    pub visited_at: DateTime<Utc>,
}

/// 队列候选行（排序涉及的列；R 与优先级排序在 service 用 scheduler 现算）。
pub struct DueRow {
    pub word_id: WordId,
    pub strength_days: f64,
    pub difficulty: f64,
    pub last_reinforcement_at: Option<DateTime<Utc>>,
    pub high_priority: bool,
    pub next_review_at: Option<DateTime<Utc>>,
}

/// 统计用行。
pub struct StatsRow {
    pub strength_days: f64,
    pub high_priority: bool,
    pub next_review_at: Option<DateTime<Utc>>,
    pub last_visit_at: Option<DateTime<Utc>>,
    pub last_review_at: Option<DateTime<Utc>>,
    pub ignored: bool,
}

pub fn insert_visit(
    conn: &Connection,
    word_id: WordId,
    now: DateTime<Utc>,
) -> Result<EncounterId, MemoryError> {
    conn.execute(
        "INSERT INTO encounters (word_id, visited_at, source, comprehension_level)
		 VALUES (?1, ?2, 'lookup', 'unknown')",
        params![word_id.0, fmt_ts(now)],
    )?;
    Ok(EncounterId(conn.last_insert_rowid()))
}

pub fn latest_visit(
    conn: &Connection,
    word_id: WordId,
) -> Result<Option<LatestVisit>, MemoryError> {
    let row: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, visited_at FROM encounters WHERE word_id = ?1
			 ORDER BY id DESC LIMIT 1",
            [word_id.0],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(row.and_then(|(id, ts)| {
        parse_ts(&ts).map(|visited_at| LatestVisit {
            id: EncounterId(id),
            visited_at,
        })
    }))
}

pub fn update_visit_time(
    conn: &Connection,
    id: EncounterId,
    now: DateTime<Utc>,
) -> Result<(), MemoryError> {
    conn.execute(
        "UPDATE encounters SET visited_at = ?1 WHERE id = ?2",
        params![fmt_ts(now), id.0],
    )?;
    Ok(())
}

pub fn update_level(
    conn: &Connection,
    id: EncounterId,
    level: ComprehensionLevel,
) -> Result<(), MemoryError> {
    conn.execute(
        "UPDATE encounters SET comprehension_level = ?1 WHERE id = ?2",
        params![level.as_str(), id.0],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, id: EncounterId) -> Result<Option<Encounter>, MemoryError> {
    let row: Option<(i64, String, String, String)> = conn
        .query_row(
            "SELECT word_id, visited_at, source, comprehension_level
			 FROM encounters WHERE id = ?1",
            [id.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    Ok(row.and_then(|(word_id, ts, source, level)| {
        Some(Encounter {
            id,
            word_id: WordId(word_id),
            timestamp: parse_ts(&ts)?,
            source: EncounterSource::from_db(&source)?,
            comprehension_level: ComprehensionLevel::from_db(&level)?,
        })
    }))
}

/// 每词取最新一条 visit，按最近访问排序（Recent 列表）。
pub fn recent_visits(conn: &Connection, limit: u8) -> Result<Vec<RecentVisit>, MemoryError> {
    let mut stmt = conn.prepare(
        "SELECT e.word_id, e.visited_at, e.comprehension_level
		 FROM encounters e
		 JOIN (SELECT word_id, MAX(id) AS max_id FROM encounters GROUP BY word_id) t
		   ON t.max_id = e.id
		 ORDER BY e.visited_at DESC
		 LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([limit as i64], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(word_id, ts, level)| {
            Some(RecentVisit {
                word_id: WordId(word_id),
                visited_at: parse_ts(&ts)?,
                comprehension_level: ComprehensionLevel::from_db(&level)?,
            })
        })
        .collect())
}

// ── 折算事件流 ────────────────────────────────────────────

/// 加载一个词的全部记忆事件（时间序）：encounters 的 visit + 已完成 reviews。
/// visit 取**当前** comprehension_level——visit 内级别升级后重折即自洽。
pub fn load_events(conn: &Connection, word_id: WordId) -> Result<Vec<MemoryEvent>, MemoryError> {
    let mut stmt = conn.prepare(
        "SELECT kind, at, detail FROM (
			 SELECT 'visit' AS kind, visited_at AS at, comprehension_level AS detail, id AS seq
			 FROM encounters WHERE word_id = ?1
			 UNION ALL
			 SELECT 'review', completed_at, COALESCE(quality, ''), id
			 FROM reviews WHERE word_id = ?1 AND completed_at IS NOT NULL
		 ) ORDER BY at, seq",
    )?;
    let rows = stmt
        .query_map([word_id.0], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut events = Vec::with_capacity(rows.len());
    for (kind, at, detail) in rows {
        let at = parse_ts(&at)
            .ok_or_else(|| MemoryError::CorruptEvent(format!("bad timestamp: {at}")))?;
        match kind.as_str() {
            "visit" => {
                let level = ComprehensionLevel::from_db(&detail)
                    .ok_or_else(|| MemoryError::CorruptEvent(format!("bad level: {detail}")))?;
                events.push(MemoryEvent::Visit { at, level });
            }
            "review" => {
                let quality = RecallQuality::from_db(&detail)
                    .ok_or_else(|| MemoryError::CorruptEvent(format!("bad quality: {detail}")))?;
                events.push(MemoryEvent::ReviewDone { at, quality });
            }
            other => return Err(MemoryError::CorruptEvent(format!("bad kind: {other}"))),
        }
    }
    Ok(events)
}

// ── 投影 ──────────────────────────────────────────────────

pub fn upsert_state(
    conn: &Connection,
    state: &MemoryState,
    now: DateTime<Utc>,
) -> Result<(), MemoryError> {
    conn.execute(
        "INSERT INTO memory_states
		 (word_id, strength_days, difficulty, review_count, lapse_count, visit_count,
		  weak_streak, high_priority, first_visit_at, last_visit_at, last_review_at,
		  last_reinforcement_at, next_review_at, updated_at)
		 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
		 ON CONFLICT(word_id) DO UPDATE SET
		   strength_days=excluded.strength_days, difficulty=excluded.difficulty,
		   review_count=excluded.review_count, lapse_count=excluded.lapse_count,
		   visit_count=excluded.visit_count, weak_streak=excluded.weak_streak,
		   high_priority=excluded.high_priority, first_visit_at=excluded.first_visit_at,
		   last_visit_at=excluded.last_visit_at, last_review_at=excluded.last_review_at,
		   last_reinforcement_at=excluded.last_reinforcement_at,
		   next_review_at=excluded.next_review_at, updated_at=excluded.updated_at",
        params![
            state.word_id.0,
            state.strength_days,
            state.difficulty,
            state.review_count,
            state.lapse_count,
            state.visit_count,
            state.weak_streak,
            state.high_priority,
            fmt_ts(state.first_visit_at),
            state.last_visit_at.map(fmt_ts),
            state.last_review_at.map(fmt_ts),
            state.last_reinforcement_at.map(fmt_ts),
            state.next_review_at.map(fmt_ts),
            fmt_ts(now)
        ],
    )?;
    Ok(())
}

type StoredMemoryState = (
    f64,
    f64,
    i64,
    i64,
    i64,
    i64,
    i64,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

pub fn load_state(conn: &Connection, word_id: WordId) -> Result<Option<MemoryState>, MemoryError> {
    let row: Option<StoredMemoryState> = conn
        .query_row(
            "SELECT strength_days, difficulty, review_count, lapse_count, visit_count,
			        weak_streak, high_priority, first_visit_at, last_visit_at, last_review_at,
			        last_reinforcement_at, next_review_at
			 FROM memory_states WHERE word_id = ?1",
            [word_id.0],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get::<_, i64>(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                ))
            },
        )
        .optional()?;
    Ok(row.and_then(
        |(s, d, rc, lc, vc, ws, hp, first, last_v, last_r, last_re, next)| {
            Some(MemoryState {
                word_id,
                strength_days: s,
                difficulty: d,
                review_count: rc.max(0) as u32,
                lapse_count: lc.max(0) as u32,
                visit_count: vc.max(0) as u32,
                weak_streak: ws.max(0) as u32,
                high_priority: hp != 0,
                first_visit_at: parse_ts(&first)?,
                last_visit_at: last_v.and_then(|t| parse_ts(&t)),
                last_review_at: last_r.and_then(|t| parse_ts(&t)),
                last_reinforcement_at: last_re.and_then(|t| parse_ts(&t)),
                next_review_at: next.and_then(|t| parse_ts(&t)),
            })
        },
    ))
}

/// 有事件流的全部词（重建投影用）。
pub fn words_with_events(conn: &Connection) -> Result<Vec<WordId>, MemoryError> {
    let mut stmt = conn.prepare("SELECT DISTINCT word_id FROM encounters")?;
    let ids = stmt
        .query_map([], |r| r.get::<_, i64>(0).map(WordId))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

// ── 复习队列 ──────────────────────────────────────────────

/// 到期候选：due ≤ now、未 ignore、60 天内有活动（visit 或 review）。
/// 排序（high_priority DESC, R ASC）在 service 完成（R 需现算）。
pub fn due_rows(
    conn: &Connection,
    now: DateTime<Utc>,
    active_since: DateTime<Utc>,
) -> Result<Vec<DueRow>, MemoryError> {
    let mut stmt = conn.prepare(
		"SELECT m.word_id, m.strength_days, m.difficulty, m.last_reinforcement_at, m.high_priority, m.next_review_at
		 FROM memory_states m
		 WHERE m.next_review_at IS NOT NULL AND m.next_review_at <= ?1
		   AND m.word_id NOT IN (SELECT word_id FROM ignored_words)
		   AND ( (m.last_visit_at IS NOT NULL AND m.last_visit_at >= ?2)
		      OR (m.last_review_at IS NOT NULL AND m.last_review_at >= ?2) )",
	)?;
    let rows = stmt
        .query_map(params![fmt_ts(now), fmt_ts(active_since)], |r| {
            Ok(DueRow {
                word_id: WordId(r.get(0)?),
                strength_days: r.get(1)?,
                difficulty: r.get(2)?,
                last_reinforcement_at: r.get::<_, Option<String>>(3)?.and_then(|t| parse_ts(&t)),
                high_priority: r.get::<_, i64>(4)? != 0,
                next_review_at: r.get::<_, Option<String>>(5)?.and_then(|t| parse_ts(&t)),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ── 统计 ──────────────────────────────────────────────────

pub fn stats_rows(conn: &Connection) -> Result<Vec<StatsRow>, MemoryError> {
    let mut stmt = conn.prepare(
		"SELECT m.strength_days, m.high_priority, m.next_review_at, m.last_visit_at, m.last_review_at,
		        (i.word_id IS NOT NULL) AS ignored
		 FROM memory_states m LEFT JOIN ignored_words i ON i.word_id = m.word_id",
	)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(StatsRow {
                strength_days: r.get(0)?,
                high_priority: r.get::<_, i64>(1)? != 0,
                next_review_at: r.get::<_, Option<String>>(2)?.and_then(|t| parse_ts(&t)),
                last_visit_at: r.get::<_, Option<String>>(3)?.and_then(|t| parse_ts(&t)),
                last_review_at: r.get::<_, Option<String>>(4)?.and_then(|t| parse_ts(&t)),
                ignored: r.get::<_, i64>(5)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ── ignore ────────────────────────────────────────────────

pub fn set_ignored(conn: &Connection, word_id: WordId, ignored: bool) -> Result<(), MemoryError> {
    if ignored {
        conn.execute(
            "INSERT OR IGNORE INTO ignored_words (word_id) VALUES (?1)",
            [word_id.0],
        )?;
    } else {
        conn.execute("DELETE FROM ignored_words WHERE word_id = ?1", [word_id.0])?;
    }
    Ok(())
}

pub fn is_ignored(conn: &Connection, word_id: WordId) -> Result<bool, MemoryError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ignored_words WHERE word_id = ?1",
        [word_id.0],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}
