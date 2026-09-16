use rusqlite::Connection;

use super::DbError;

/// 版本化迁移：按顺序应用 SQL 文件，`PRAGMA user_version` 记录当前版本。
/// 新迁移 = 在 `migrations/` 新增 `NNNN_名称.sql` 并追加到下面的列表。
const MIGRATIONS: &[&str] = &[
    include_str!("migrations/0001_init.sql"),
    include_str!("migrations/0002_freq_rank_index.sql"),
    include_str!("migrations/0003_word_notes.sql"),
    include_str!("migrations/0004_ai_examples.sql"),
];

pub fn migrate(conn: &Connection) -> Result<(), DbError> {
    let mut version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let target = (i + 1) as i64;
        if version >= target {
            continue;
        }
        // 单一 Mutex 保证无并发事务，unchecked_transaction 是安全的。
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", target)?;
        tx.commit()?;
        version = target;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{open_in_memory, test_conn};

    fn object_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type IN ('table','view') ORDER BY name")
            .unwrap();
        stmt.query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn upgrading_v2_keeps_existing_learning_data() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.execute_batch(MIGRATIONS[1]).unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        conn.execute("INSERT INTO words(id,word,display,created_at) VALUES(1,'test','test','2026-01-01T00:00:00.000Z')", []).unwrap();
        conn.execute("INSERT INTO encounters(word_id,visited_at,comprehension_level) VALUES(1,'2026-01-01T00:00:00.000Z','english')", []).unwrap();
        migrate(&conn).unwrap();
        let level: String = conn
            .query_row(
                "SELECT comprehension_level FROM encounters WHERE word_id=1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(level, "english");
        crate::library::save_note(&conn, 1, true, "existing word").unwrap();
        assert!(crate::library::word_note(&conn, 1).unwrap().bookmarked);
    }

    #[test]
    fn migrations_apply_and_version_bump() {
        let conn = open_in_memory().unwrap();
        migrate(&conn).unwrap();

        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 4);

        let names = object_names(&conn);
        for expected in [
            "words",
            "senses",
            "examples",
            "collocations",
            "word_forms",
            "words_fts",
            "encounters",
            "memory_states",
            "ignored_words",
            "reviews",
            "review_attempts",
            "app_settings",
            "ai_example_cache",
        ] {
            assert!(
                names.iter().any(|n| n == expected),
                "missing table: {expected}"
            );
        }

        // 幂等：重跑不报错、版本不变。
        migrate(&conn).unwrap();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 4);
    }

    #[test]
    fn fts_triggers_sync_on_insert_update_delete() {
        let conn = test_conn();

        conn.execute(
            "INSERT INTO words (id, word, display, created_at)
			 VALUES (1, 'meticulous', 'meticulous', '2026-01-01T00:00:00.000Z')",
            [],
        )
        .unwrap();
        conn.execute(
			"INSERT INTO senses (id, word_id, pos, english_definition, chinese_definition, order_idx)
			 VALUES (10, 1, 'adj.', 'very careful about small details', '一丝不苟的', 0)",
			[],
		)
		.unwrap();

        let hit = |q: &str| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM words_fts WHERE words_fts MATCH ?1",
                [q],
                |r| r.get(0),
            )
            .unwrap()
        };

        // 插入 → FTS 行出现（词头与英文释义均可检索）
        assert_eq!(hit("word:meticulous"), 1);
        assert_eq!(hit("english:careful"), 1);

        // 更新释义 → FTS 同步
        conn.execute(
            "UPDATE senses SET english_definition = 'extremely thorough' WHERE id = 10",
            [],
        )
        .unwrap();
        assert_eq!(hit("english:thorough"), 1);
        assert_eq!(hit("english:careful"), 0);

        // 删除义项 → FTS 行消失
        conn.execute("DELETE FROM senses WHERE id = 10", [])
            .unwrap();
        assert_eq!(hit("word:meticulous"), 0);
    }
}
