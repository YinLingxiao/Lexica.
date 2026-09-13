//! Personal vocabulary, independent of the memory event stream.
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::database::{fmt_ts, DbError};
use crate::memory::model::{FAMILIAR_THRESHOLD, STABLE_THRESHOLD};

#[derive(Debug, Default, Serialize)]
pub struct WordNote {
    pub bookmarked: bool,
    pub note: String,
    pub ignored: bool,
}

pub fn word_note(conn: &Connection, word_id: i64) -> Result<WordNote, DbError> {
    Ok(conn.query_row(
        "SELECT COALESCE(n.bookmarked, 0), COALESCE(n.note, ''), i.word_id IS NOT NULL
         FROM words w LEFT JOIN word_notes n ON n.word_id=w.id
         LEFT JOIN ignored_words i ON i.word_id=w.id WHERE w.id=?1",
        [word_id],
        |r| {
            Ok(WordNote {
                bookmarked: r.get(0)?,
                note: r.get(1)?,
                ignored: r.get(2)?,
            })
        },
    )?)
}

pub fn save_note(
    conn: &Connection,
    word_id: i64,
    bookmarked: bool,
    note: &str,
) -> Result<(), DbError> {
    if note.chars().count() > 10000 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "A note may not exceed 10,000 characters",
        )
        .into());
    }
    conn.execute(
        "INSERT INTO word_notes(word_id, bookmarked, note, updated_at) VALUES (?1,?2,?3,?4)
         ON CONFLICT(word_id) DO UPDATE SET bookmarked=excluded.bookmarked, note=excluded.note, updated_at=excluded.updated_at",
        params![word_id, bookmarked, note, fmt_ts(Utc::now())],
    )?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct LibraryWord {
    pub id: i64,
    pub word: String,
    pub display: String,
    pub phonetic: Option<String>,
    pub definition: String,
    pub status: String,
    pub bookmarked: bool,
    pub note: String,
    pub visit_count: i64,
    pub next_review_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LibraryPage {
    pub words: Vec<LibraryWord>,
    pub total: i64,
}

pub fn list(
    conn: &Connection,
    query: &str,
    filter: &str,
    offset: u32,
) -> Result<LibraryPage, DbError> {
    // Start with the user's small vocabulary, never the multi-million-row dictionary.
    let cte = "WITH ids AS (
      SELECT word_id FROM memory_states UNION SELECT word_id FROM word_notes WHERE bookmarked=1 OR note<>''
    ), vocabulary AS (
      SELECT w.id,w.word,w.display,w.phonetic,COALESCE(n.bookmarked,0) bookmarked,
        COALESCE(n.note,'') note,COALESCE(m.visit_count,0) visit_count,m.next_review_at,
        COALESCE(m.last_visit_at,n.updated_at,'') touched,
        CASE WHEN i.word_id IS NOT NULL THEN 'ignored'
          WHEN m.word_id IS NULL THEN 'new'
          WHEN m.strength_days < ?3 THEN 'learning'
          WHEN m.strength_days < ?4 THEN 'familiar' ELSE 'stable' END status
      FROM ids JOIN words w ON w.id=ids.word_id
      LEFT JOIN memory_states m ON m.word_id=w.id
      LEFT JOIN word_notes n ON n.word_id=w.id
      LEFT JOIN ignored_words i ON i.word_id=w.id
    ), filtered AS (
      SELECT * FROM vocabulary WHERE (instr(lower(word),lower(?1))>0 OR instr(lower(note),lower(?1))>0)
      AND (?2='all' OR (?2='bookmarked' AND bookmarked=1) OR status=?2)
    )";
    let total = conn.query_row(
        &format!("{cte} SELECT COUNT(*) FROM filtered"),
        params![query.trim(), filter, FAMILIAR_THRESHOLD, STABLE_THRESHOLD],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&format!("{cte}
      SELECT f.id,f.word,f.display,f.phonetic,f.status,f.bookmarked,f.note,f.visit_count,f.next_review_at,
        COALESCE((SELECT COALESCE(NULLIF(chinese_definition,''),english_definition) FROM senses
          WHERE word_id=f.id ORDER BY order_idx,id LIMIT 1),'No definition')
      FROM filtered f ORDER BY touched DESC,id DESC LIMIT 24 OFFSET ?5"))?;
    let words = stmt
        .query_map(
            params![
                query.trim(),
                filter,
                FAMILIAR_THRESHOLD,
                STABLE_THRESHOLD,
                offset
            ],
            |r| {
                Ok(LibraryWord {
                    id: r.get(0)?,
                    word: r.get(1)?,
                    display: r.get(2)?,
                    phonetic: r.get(3)?,
                    status: r.get(4)?,
                    bookmarked: r.get(5)?,
                    note: r.get(6)?,
                    visit_count: r.get(7)?,
                    next_review_at: r.get(8)?,
                    definition: r.get(9)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LibraryPage { words, total })
}

#[derive(Debug, Serialize)]
pub struct ActivityDay {
    pub date: String,
    pub lookups: i64,
    pub reviews: i64,
}

pub fn activity(
    conn: &Connection,
    now: DateTime<Utc>,
    offset_minutes: i32,
) -> Result<Vec<ActivityDay>, DbError> {
    let offset = offset_minutes.clamp(-840, 840);
    let local_now = now + Duration::minutes(i64::from(offset));
    let first = local_now.date_naive() - Duration::days(13);
    let shift = format!("{offset} minutes");
    let mut stmt = conn.prepare(
        "SELECT day,SUM(lookups),SUM(reviews) FROM (
          SELECT date(visited_at,?1) day,COUNT(*) lookups,0 reviews FROM encounters WHERE visited_at>=?2 GROUP BY day
          UNION ALL
          SELECT date(completed_at,?1) day,0 lookups,COUNT(*) reviews FROM reviews WHERE completed_at>=?2 GROUP BY day
        ) GROUP BY day")?;
    let since = fmt_ts(now - Duration::days(15));
    let counts = stmt
        .query_map(params![shift, since], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, i64>(1)?, r.get::<_, i64>(2)?),
            ))
        })?
        .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;
    (0..14)
        .map(|i| {
            let date = (first + Duration::days(i)).to_string();
            let (lookups, reviews) = counts.get(&date).copied().unwrap_or((0, 0));
            Ok(ActivityDay {
                date,
                lookups,
                reviews,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_conn;
    use crate::dictionary::provider::{DictionaryProvider, SeedProvider};

    #[test]
    fn notes_persist_filter_and_reject_unknown_words() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let id = conn
            .query_row("SELECT id FROM words LIMIT 1", [], |r| r.get::<_, i64>(0))
            .unwrap();
        save_note(&conn, id, true, "阅读笔记 100% _").unwrap();
        let page = list(&conn, "100% _", "bookmarked", 0).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.words[0].note, "阅读笔记 100% _");
        assert_eq!(list(&conn, "", "all", 24).unwrap().words.len(), 0);
        save_note(&conn, id, false, "").unwrap();
        assert!(!word_note(&conn, id).unwrap().bookmarked);
        assert_eq!(list(&conn, "", "all", 0).unwrap().total, 0);
        assert!(save_note(&conn, -1, true, "").is_err());
        assert!(save_note(&conn, id, true, &"a".repeat(10001)).is_err());
    }

    #[test]
    fn activity_uses_local_dates_and_counts_only_completed_reviews() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let id = conn
            .query_row("SELECT id FROM words LIMIT 1", [], |r| r.get::<_, i64>(0))
            .unwrap();
        conn.execute(
            "INSERT INTO encounters(word_id,visited_at) VALUES (?1,'2026-09-12T17:00:00.000Z')",
            [id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO reviews(word_id,started_at) VALUES (?1,'2026-09-12T17:00:00.000Z')",
            [id],
        )
        .unwrap();
        let now = "2026-09-13T10:00:00Z".parse().unwrap();
        let days = activity(&conn, now, 480).unwrap();
        assert_eq!(days.len(), 14);
        assert_eq!(days[13].date, "2026-09-13");
        assert_eq!(days[13].lookups, 1);
        assert_eq!(days[13].reviews, 0);
        assert_eq!(activity(&conn, now, 0).unwrap()[12].lookups, 1);
    }
}
