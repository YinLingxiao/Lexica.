//! SQL 层：只做查询与装配，不含策略（策略在 engine）。

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};

use super::model::{Collocation, Example, Sense, Suggestion, WordBrief, WordEntry};
use super::DictionaryError;
use crate::domain::{ExampleId, SenseId, WordId};

/// FTS5 查询词转义：包成双引号字符串，内部双引号加倍。
/// 使 "don't"、"e-mail" 等含特殊字符的词头可安全用于 MATCH。
fn fts_quote_term(term: &str) -> String {
    format!("\"{}\"", term.replace('"', "\"\""))
}

pub fn word_by_normalized(
    conn: &Connection,
    word: &str,
) -> Result<Option<WordEntry>, DictionaryError> {
    let id: Option<i64> = conn
        .query_row("SELECT id FROM words WHERE word = ?1", [word], |r| r.get(0))
        .optional()?;
    match id {
        Some(id) => Ok(Some(load_entry(conn, WordId(id))?)),
        None => Ok(None),
    }
}

pub fn word_by_id(conn: &Connection, id: WordId) -> Result<Option<WordEntry>, DictionaryError> {
    let exists: Option<i64> = conn
        .query_row("SELECT id FROM words WHERE id = ?1", [id.0], |r| r.get(0))
        .optional()?;
    match exists {
        Some(_) => Ok(Some(load_entry(conn, id)?)),
        None => Ok(None),
    }
}

/// 屈折回退：went → go 的 word_id。
pub fn headword_of_form(conn: &Connection, form: &str) -> Result<Option<WordId>, DictionaryError> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT word_id FROM word_forms WHERE form = ?1",
            [form],
            |r| r.get(0),
        )
        .optional()?;
    Ok(id.map(WordId))
}

/// 变音符折叠后的精确命中（cafe → café）。
/// FTS5 unicode61 remove_diacritics 在索引与查询两侧都做折叠，因此精确 MATCH 即等价匹配。
pub fn word_by_folded(conn: &Connection, word: &str) -> Result<Option<WordEntry>, DictionaryError> {
    let pattern = format!("word:{}", fts_quote_term(word));
    let id: Option<i64> = conn
        .query_row(
            "SELECT s.word_id FROM words_fts f
			 JOIN senses s ON s.id = f.rowid
			 WHERE words_fts MATCH ?1 LIMIT 1",
            [&pattern],
            |r| r.get(0),
        )
        .optional()?;
    match id {
        Some(id) => Ok(Some(load_entry(conn, WordId(id))?)),
        None => Ok(None),
    }
}

/// 前缀建议：精确命中优先，其次按词频升序，字母序补齐。
/// 340 万词条下不走 FTS（每义项一行 + DISTINCT 去重 + 全量排序，
/// 单字母前缀 600ms+），改走 words 表索引：
/// ① idx_words_freq_rank（部分索引）按词频扫出常用词；
/// ② 不足 limit 再从 idx_words_word 范围按字母序补齐无词频的词。
/// 代价：失去 FTS 的变音符折叠（查询已 normalize，仅剩 café 类词头前缀差异，可接受）。
pub fn suggest_prefix(
    conn: &Connection,
    query: &str,
    limit: u8,
) -> Result<Vec<Suggestion>, DictionaryError> {
    let bound = prefix_upper_bound(query);
    let mut rows: Vec<Suggestion> = {
        let mut stmt = conn.prepare(
            "SELECT word, display, phonetic, frequency_rank FROM words
             WHERE word >= ?1 AND word < ?2 AND frequency_rank IS NOT NULL
             ORDER BY frequency_rank LIMIT ?3",
        )?;
        let collected = stmt
            .query_map(params![query, bound, i64::from(limit)], |r| {
                Ok(Suggestion {
                    word: r.get(0)?,
                    display: r.get(1)?,
                    phonetic: r.get(2)?,
                    frequency_rank: r.get::<_, Option<i64>>(3)?.map(|v| v as u32),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        collected
    };

    if rows.len() < usize::from(limit) {
        let mut stmt = conn.prepare(
            "SELECT word, display, phonetic, frequency_rank FROM words
             WHERE word >= ?1 AND word < ?2 ORDER BY word LIMIT ?3",
        )?;
        let seen: std::collections::HashSet<String> = rows.iter().map(|s| s.word.clone()).collect();
        for suggestion in stmt.query_map(params![query, bound, i64::from(limit)], |r| {
            Ok(Suggestion {
                word: r.get(0)?,
                display: r.get(1)?,
                phonetic: r.get(2)?,
                frequency_rank: r.get::<_, Option<i64>>(3)?.map(|v| v as u32),
            })
        })? {
            let suggestion = suggestion?;
            if !seen.contains(&suggestion.word) {
                rows.push(suggestion);
                if rows.len() >= usize::from(limit) {
                    break;
                }
            }
        }
    }

    // 稳定排序保持词频/字母序不变，仅把精确命中提到最前。
    rows.sort_by_key(|s| s.word != query);
    rows.truncate(usize::from(limit));
    Ok(rows)
}

/// 前缀查询的字典序上界（"met" → "meu"），供索引范围扫描使用。
fn prefix_upper_bound(prefix: &str) -> String {
    let mut bound = prefix.to_string();
    while let Some(last) = bound.pop() {
        if let Some(next) = char::from_u32(u32::from(last) + 1) {
            bound.push(next);
            return bound;
        }
    }
    "\u{10FFFF}".to_string()
}

pub fn word_count(conn: &Connection) -> Result<i64, DictionaryError> {
    Ok(conn.query_row("SELECT COUNT(*) FROM words", [], |r| r.get(0))?)
}

/// 批量取词显示信息（供 commands 补齐 Recent 等列表）。
pub fn word_briefs(
    conn: &Connection,
    ids: &[WordId],
) -> Result<HashMap<WordId, WordBrief>, DictionaryError> {
    let mut map = HashMap::new();
    if ids.is_empty() {
        return Ok(map);
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!("SELECT id, word, display, phonetic FROM words WHERE id IN ({placeholders})");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(ids.iter().map(|i| i.0)), |r| {
            Ok((
                WordId(r.get::<_, i64>(0)?),
                WordBrief {
                    word: r.get(1)?,
                    display: r.get(2)?,
                    phonetic: r.get(3)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, brief) in rows {
        map.insert(id, brief);
    }
    Ok(map)
}

/// 装配完整词条：词 + 义项（按序）+ 例句（按序）+ 搭配（按序）。
fn load_entry(conn: &Connection, id: WordId) -> Result<WordEntry, DictionaryError> {
    let (word, display, phonetic, frequency_rank, synonyms, antonyms, word_family) = conn
        .query_row(
            "SELECT word, display, phonetic, frequency_rank, synonyms, antonyms, word_family
		 FROM words WHERE id = ?1",
            [id.0],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )?;

    let mut stmt = conn.prepare(
        "SELECT id, pos, english_definition, chinese_definition, level
		 FROM senses WHERE word_id = ?1 ORDER BY order_idx, id",
    )?;
    let mut senses: Vec<Sense> = stmt
        .query_map([id.0], |row| {
            Ok(Sense {
                id: SenseId(row.get(0)?),
                pos: row.get(1)?,
                english_definition: row.get(2)?,
                chinese_definition: row.get(3)?,
                level: row.get(4)?,
                examples: Vec::new(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // 一次查询取出全部例句，按 sense_id 分组
    let mut stmt = conn.prepare(
        "SELECT e.id, e.sense_id, e.text, e.translation
		 FROM examples e JOIN senses s ON s.id = e.sense_id
		 WHERE s.word_id = ?1 ORDER BY e.order_idx, e.id",
    )?;
    let mut examples_by_sense: HashMap<i64, Vec<Example>> = {
        let mut map: HashMap<i64, Vec<Example>> = HashMap::new();
        let rows = stmt.query_map([id.0], |row| {
            Ok((
                row.get::<_, i64>(1)?,
                Example {
                    id: ExampleId(row.get(0)?),
                    text: row.get(2)?,
                    translation: row.get(3)?,
                },
            ))
        })?;
        for pair in rows {
            let (sense_id, example) = pair?;
            map.entry(sense_id).or_default().push(example);
        }
        map
    };
    for sense in &mut senses {
        if let Some(examples) = examples_by_sense.remove(&sense.id.0) {
            sense.examples = examples;
        }
    }

    let mut stmt = conn.prepare(
        "SELECT text, gloss FROM collocations WHERE word_id = ?1 ORDER BY order_idx, id",
    )?;
    let collocations: Vec<Collocation> = stmt
        .query_map([id.0], |row| {
            Ok(Collocation {
                text: row.get(0)?,
                gloss: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(WordEntry {
        id,
        word,
        display,
        phonetic,
        senses,
        collocations,
        synonyms: serde_json::from_str(&synonyms)?,
        antonyms: serde_json::from_str(&antonyms)?,
        word_family: serde_json::from_str(&word_family)?,
        frequency_rank: frequency_rank.map(|v| v as u32),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_upper_bound_covers_range() {
        assert_eq!(prefix_upper_bound("met"), "meu");
        assert_eq!(prefix_upper_bound("a"), "b");
        // 尾字符递增落入代理区时继续回退
        assert_eq!(prefix_upper_bound(""), "\u{10FFFF}");
        // 非 ASCII 尾字符
        assert_eq!(prefix_upper_bound("é"), "ê");
    }
}
