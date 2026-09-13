//! 策略层：规范化、查词回退链（精确 → 变音符折叠 → 屈折 → 建议）。

use rusqlite::Connection;
use unicode_normalization::UnicodeNormalization;

use super::model::{LookupOutcome, Suggestion, WordBrief, WordEntry};
use super::{store, DictionaryError};
use crate::domain::WordId;
use std::collections::HashMap;

/// 词头规范化：NFKC（含全角→半角）→ 小写 → 去首尾空白与引号 → 弯引号归直。
/// 查词、导入、词形表统一使用。
pub fn normalize_word(input: &str) -> String {
    let s: String = input.nfkc().collect();
    let s = s.trim().to_lowercase();
    let s = s.replace(['‘', '’', '`', '´'], "'");
    let s = s.replace(['“', '”'], "\"");
    s.trim_matches(|c| c == '"' || c == '\'').to_string()
}

/// 查词回退链：
/// 1. 规范化精确匹配
/// 2. 变音符折叠匹配（cafe → café）
/// 3. 屈折词形回退（went → go）
/// 4. 全部落空 → Miss + 前缀建议
pub fn lookup(conn: &Connection, input: &str) -> Result<LookupOutcome, DictionaryError> {
    let q = normalize_word(input);
    if q.is_empty() {
        return Ok(LookupOutcome::Miss {
            suggestions: Vec::new(),
        });
    }
    if let Some(entry) = store::word_by_normalized(conn, &q)? {
        return Ok(LookupOutcome::Hit { entry });
    }
    if let Some(entry) = store::word_by_folded(conn, &q)? {
        return Ok(LookupOutcome::Hit { entry });
    }
    if let Some(word_id) = store::headword_of_form(conn, &q)? {
        if let Some(entry) = store::word_by_id(conn, word_id)? {
            return Ok(LookupOutcome::Hit { entry });
        }
    }
    let suggestions = suggest(conn, &q, 8)?;
    Ok(LookupOutcome::Miss { suggestions })
}

/// 输入过程中的搜索建议（空查询返回空）。
pub fn suggest(
    conn: &Connection,
    query: &str,
    limit: u8,
) -> Result<Vec<Suggestion>, DictionaryError> {
    let q = normalize_word(query);
    if q.is_empty() {
        return Ok(Vec::new());
    }
    store::suggest_prefix(conn, &q, limit)
}

/// 按 ID 取完整词条（review 构造提示等内部用途）。
pub fn entry_by_id(conn: &Connection, id: WordId) -> Result<Option<WordEntry>, DictionaryError> {
    store::word_by_id(conn, id)
}

pub fn word_briefs(
    conn: &Connection,
    ids: &[WordId],
) -> Result<HashMap<WordId, WordBrief>, DictionaryError> {
    store::word_briefs(conn, ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_conn;
    use crate::dictionary::provider::{DictionaryProvider, SeedProvider};

    fn seeded_conn() -> Connection {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        conn
    }

    fn hit_word(conn: &Connection, input: &str) -> String {
        match lookup(conn, input).unwrap() {
            LookupOutcome::Hit { entry } => entry.word,
            LookupOutcome::Miss { .. } => panic!("expected hit for {input}"),
        }
    }

    #[test]
    fn normalize_word_folds_case_width_quotes() {
        assert_eq!(normalize_word("  Meticulous  "), "meticulous");
        assert_eq!(normalize_word("“meticulous”"), "meticulous");
        assert_eq!(normalize_word("don’t"), "don't");
        assert_eq!(normalize_word("ｍｅｔｉｃｕｌｏｕｓ"), "meticulous"); // 全角
        assert_eq!(normalize_word(""), "");
    }

    #[test]
    fn exact_lookup_returns_full_entry() {
        let conn = seeded_conn();
        let entry = match lookup(&conn, "meticulous").unwrap() {
            LookupOutcome::Hit { entry } => entry,
            LookupOutcome::Miss { .. } => panic!("expected hit"),
        };
        assert_eq!(entry.word, "meticulous");
        assert_eq!(entry.senses.len(), 1);
        assert_eq!(entry.senses[0].pos.as_deref(), Some("adj."));
        assert!(entry.senses[0]
            .english_definition
            .starts_with("very careful"));
        assert_eq!(entry.senses[0].examples.len(), 3);
        assert!(entry.senses[0].examples[0].translation.is_some());
        assert_eq!(entry.collocations.len(), 2);
        assert_eq!(entry.synonyms.len(), 4);
        assert_eq!(entry.antonyms.len(), 3);
        assert_eq!(entry.word_family.len(), 2);
    }

    #[test]
    fn lookup_normalizes_case_and_quotes() {
        let conn = seeded_conn();
        assert_eq!(hit_word(&conn, "Meticulous"), "meticulous");
        assert_eq!(hit_word(&conn, " METICULOUS "), "meticulous");
        assert_eq!(hit_word(&conn, "“Meticulous”"), "meticulous");
    }

    #[test]
    fn lookup_folds_diacritics() {
        let conn = seeded_conn();
        // 临时插入带变音符的词
        conn.execute(
			"INSERT INTO words (word, display, created_at) VALUES ('café', 'café', '2026-01-01T00:00:00.000Z')",
			[],
		)
		.unwrap();
        conn.execute(
			"INSERT INTO senses (word_id, english_definition) VALUES ((SELECT id FROM words WHERE word='café'), 'a place where coffee is sold')",
			[],
		)
		.unwrap();
        assert_eq!(hit_word(&conn, "cafe"), "café");
        assert_eq!(hit_word(&conn, "CAFÉ"), "café");
    }

    #[test]
    fn multi_sense_preserves_order() {
        let conn = seeded_conn();
        let entry = match lookup(&conn, "elaborate").unwrap() {
            LookupOutcome::Hit { entry } => entry,
            LookupOutcome::Miss { .. } => panic!("expected hit"),
        };
        assert_eq!(entry.senses.len(), 2);
        assert_eq!(entry.senses[0].pos.as_deref(), Some("adj."));
        assert_eq!(entry.senses[1].pos.as_deref(), Some("v."));
        assert_eq!(entry.senses[1].examples.len(), 2);
    }

    #[test]
    fn inflection_finds_headword() {
        let conn = seeded_conn();
        // deteriorated 是 deteriorate 的屈折形式
        assert_eq!(hit_word(&conn, "deteriorated"), "deteriorate");
        assert_eq!(hit_word(&conn, "scrutinizing"), "scrutinize");
    }

    #[test]
    fn lookup_missing_returns_miss_with_suggestions() {
        let conn = seeded_conn();
        match lookup(&conn, "meti").unwrap() {
            LookupOutcome::Hit { .. } => panic!("expected miss"),
            LookupOutcome::Miss { suggestions } => {
                assert!(
                    suggestions.iter().any(|s| s.word == "meticulous"),
                    "{suggestions:?}"
                );
            }
        }
        // 完全无关的查询 → Miss + 空建议（不 panic）
        match lookup(&conn, "zzzz").unwrap() {
            LookupOutcome::Miss { suggestions } => assert!(suggestions.is_empty()),
            LookupOutcome::Hit { .. } => panic!("expected miss"),
        }
    }

    #[test]
    fn suggest_prefix_ranks_by_frequency() {
        let conn = seeded_conn();
        let s = suggest(&conn, "co", 8).unwrap();
        let words: Vec<&str> = s.iter().map(|x| x.word.as_str()).collect();
        assert!(words.contains(&"compelling"));
        assert!(words.contains(&"coherent"));
        assert!(words.contains(&"conspicuous"));
        // 词频升序：compelling(4700) < coherent(6500) < conspicuous(12000)
        let pos = |w: &str| words.iter().position(|x| *x == w).unwrap();
        assert!(pos("compelling") < pos("coherent"));
        assert!(pos("coherent") < pos("conspicuous"));
    }

    #[test]
    fn suggest_empty_query_returns_empty() {
        let conn = seeded_conn();
        assert!(suggest(&conn, "", 8).unwrap().is_empty());
        assert!(suggest(&conn, "   ", 8).unwrap().is_empty());
    }

    #[test]
    fn suggest_tops_up_and_prefers_exact() {
        let conn = seeded_conn();
        // 精确命中置顶
        let s = suggest(&conn, "subtle", 8).unwrap();
        assert_eq!(s.first().unwrap().word, "subtle");

        // 无词频的词也能通过字母序补齐出现在建议里
        conn.execute(
            "INSERT INTO words (word, display, created_at)
             VALUES ('zephyrque', 'zephyrque', '2026-01-01T00:00:00.000Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO senses (word_id, english_definition, order_idx)
             VALUES ((SELECT id FROM words WHERE word = 'zephyrque'), 'test sense', 0)",
            [],
        )
        .unwrap();
        let s = suggest(&conn, "zephyr", 8).unwrap();
        assert!(s.iter().any(|x| x.word == "zephyrque"), "{s:?}");
    }

    #[test]
    fn word_briefs_maps_ids() {
        let conn = seeded_conn();
        let entry = match lookup(&conn, "subtle").unwrap() {
            LookupOutcome::Hit { entry } => entry,
            LookupOutcome::Miss { .. } => panic!("expected hit"),
        };
        let briefs = word_briefs(&conn, &[entry.id]).unwrap();
        assert_eq!(briefs.get(&entry.id).unwrap().word, "subtle");
    }
}
