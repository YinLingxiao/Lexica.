//! SQL 层：只做查询与装配，不含策略（策略在 engine）。

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};

use super::engine::normalize_word;
use super::model::{
    Collocation, Example, RelatedWordItem, RootPart, Sense, Suggestion, WordBrief, WordEntry,
    WordFormEntry,
};
use super::roots;
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

type EnrichmentRow = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// 装配完整词条：词 + 义项（按序）+ 例句（按序）+ 搭配（按序）+ 8 大拓展栏。
fn load_entry(conn: &Connection, id: WordId) -> Result<WordEntry, DictionaryError> {
    let (
        word,
        display,
        phonetic,
        frequency_rank,
        synonyms,
        antonyms,
        word_family,
        tags_raw,
        collins_raw,
        oxford_raw,
    ) = conn.query_row(
        "SELECT word, display, phonetic, frequency_rank, synonyms, antonyms, word_family,
                tags, collins, oxford
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
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<i64>>(9)?,
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

    let collins = collins_raw.filter(|&c| c > 0).map(|c| c as u8);
    let tags = parse_tags(tags_raw.as_deref().unwrap_or(""));
    let oxford = oxford_raw.unwrap_or(0) != 0 || tags.iter().any(|t| t == "oxford");

    // 兼容旧版已导入的 ECDICT 行：若 pos 或 level 为空，动态提取词性并推断 CEFR 等级
    for (idx, sense) in senses.iter_mut().enumerate() {
        if sense.pos.as_deref().unwrap_or("").trim().is_empty() {
            let (inferred_pos, cleaned_en) =
                infer_pos_and_strip(&sense.english_definition, sense.chinese_definition.as_deref());
            if let Some(p) = inferred_pos {
                sense.pos = Some(p);
            }
            if !cleaned_en.is_empty() {
                sense.english_definition = cleaned_en;
            }
        }
        if sense.level.as_deref().unwrap_or("").trim().is_empty() {
            sense.level = Some(infer_cefr_level(frequency_rank, collins, oxford, idx));
        }
    }

    // 词形变化（word_forms）
    let mut stmt = conn.prepare(
        "SELECT kind, form FROM word_forms WHERE word_id = ?1 ORDER BY rowid",
    )?;
    let forms: Vec<WordFormEntry> = stmt
        .query_map([id.0], |row| {
            let raw_kind: String = row.get(0)?;
            let form_val: String = row.get(1)?;
            let label = if raw_kind.is_empty() || raw_kind == "form" {
                infer_form_label(&word, &form_val)
            } else {
                raw_kind
            };
            Ok(WordFormEntry {
                label,
                form: form_val,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // 助记、词根词缀、真题/标准例句（word_enrichments）
    let enrichment: Option<EnrichmentRow> = conn
        .query_row(
            "SELECT mnemonic, roots_json, exam_example_text, exam_example_translation,
                    standard_example_text, standard_example_translation
             FROM word_enrichments WHERE word_id = ?1",
            [id.0],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()?;

    let mut mnemonic: Option<String> = None;
    let mut roots_vec: Vec<RootPart> = Vec::new();
    let mut exam_example: Option<Example> = None;

    if let Some((mn, r_json, ex_text, ex_trans, std_text, std_trans)) = enrichment {
        mnemonic = mn.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        if let Some(raw_roots) = r_json {
            let trimmed = raw_roots.trim();
            if !trimmed.is_empty() && trimmed != "[]" && trimmed != "null" {
                if let Ok(parsed) = serde_json::from_str::<Vec<RootPart>>(trimmed) {
                    roots_vec = parsed;
                }
            }
        }
        if let Some(text) = ex_text.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
            exam_example = Some(Example {
                id: ExampleId(0),
                text,
                translation: ex_trans
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
            });
        } else if let Some(text) = std_text.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
            exam_example = Some(Example {
                id: ExampleId(0),
                text,
                translation: std_trans
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
            });
        }
    }

    if roots_vec.is_empty() {
        roots_vec = roots::decompose_roots(&word);
    }
    if exam_example.is_none() {
        exam_example = senses
            .iter()
            .flat_map(|s| s.examples.iter())
            .next()
            .cloned();
    }

    let synonyms_vec: Vec<String> = serde_json::from_str(&synonyms)?;
    let antonyms_vec: Vec<String> = serde_json::from_str(&antonyms)?;
    let word_family_vec: Vec<String> = serde_json::from_str(&word_family)?;

    let mut derivatives = resolve_derivatives(conn, &word_family_vec)?;
    if derivatives.is_empty() {
        derivatives = find_prefix_derivatives(conn, &word, 4)?;
    }
    let synonyms_detailed = resolve_related_words(conn, &synonyms_vec)?;
    let antonyms_detailed = resolve_related_words(conn, &antonyms_vec)?;
    let lookalikes = find_lookalikes(conn, &word, 5)?;

    if mnemonic.is_none() && !roots_vec.is_empty() {
        let breakdown = roots_vec
            .iter()
            .map(|r| format!("{}（{}）", r.part, r.meaning))
            .collect::<Vec<_>>()
            .join(" + ");
        let primary_zh = senses
            .iter()
            .find_map(|s| s.chinese_definition.as_deref())
            .and_then(|z| z.lines().next())
            .unwrap_or("")
            .trim();
        mnemonic = Some(if primary_zh.is_empty() {
            format!("{display} 构词拆解：{breakdown}。")
        } else {
            format!("{display} 构词记忆：{breakdown} → {primary_zh}。")
        });
    }

    Ok(WordEntry {
        id,
        word,
        display,
        phonetic,
        senses,
        collocations,
        synonyms: synonyms_vec,
        antonyms: antonyms_vec,
        word_family: word_family_vec,
        frequency_rank: frequency_rank.map(|v| v as u32),
        tags,
        collins,
        oxford,
        mnemonic,
        roots: roots_vec,
        derivatives,
        synonyms_detailed,
        antonyms_detailed,
        lookalikes,
        forms,
        exam_example,
    })
}

fn infer_pos_and_strip(en: &str, zh: Option<&str>) -> (Option<String>, String) {
    let check = |text: &str| -> Option<(&'static str, String)> {
        let trimmed = text.trim();
        for (prefix, norm) in [
            ("noun ", "noun"),
            ("verb ", "verb"),
            ("n.", "n."),
            ("vt.", "vt."),
            ("vi.", "vi."),
            ("v.", "v."),
            ("adj.", "adj."),
            ("adv.", "adv."),
            ("prep.", "prep."),
            ("conj.", "conj."),
            ("pron.", "pron."),
            ("a.", "adj."),
            ("s.", "adj."),
            ("r.", "adv."),
        ] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                    return Some((norm, rest.trim().to_string()));
                }
            }
        }
        None
    };
    if !en.contains('\n') {
        if let Some((pos, rest)) = check(en) {
            return (Some(pos.to_string()), rest);
        }
    }
    if let Some((pos, _)) = zh.and_then(check) {
        return (Some(pos.to_string()), String::new());
    }
    (None, String::new())
}

fn infer_cefr_level(
    frequency_rank: Option<i64>,
    collins: Option<u8>,
    oxford: bool,
    sense_idx: usize,
) -> String {
    if sense_idx >= 2 {
        return "R".to_string();
    }
    if let Some(c) = collins {
        return match c {
            5 => "A2",
            4 => "B1",
            3 => "B2",
            2 => "C1",
            _ => "C2",
        }
        .to_string();
    }
    if let Some(rank) = frequency_rank.filter(|&r| r > 0) {
        return if rank <= 1500 {
            "A2"
        } else if rank <= 3500 {
            "B1"
        } else if rank <= 7000 {
            "B2"
        } else if rank <= 14000 {
            "C1"
        } else {
            "R"
        }
        .to_string();
    }
    if oxford {
        "B1".to_string()
    } else {
        "B2".to_string()
    }
}

fn infer_form_label(headword: &str, form: &str) -> String {
    if form.ends_with("ing") && !headword.ends_with("ing") {
        "Present participle".to_string()
    } else if form.ends_with("iest") || (form.ends_with("est") && !headword.ends_with("est")) {
        "Superlative".to_string()
    } else if form.ends_with("ier") || (form.ends_with("er") && !headword.ends_with("er")) {
        "Comparative".to_string()
    } else if form.ends_with("ed") && !headword.ends_with("ed") {
        "Past / Past participle".to_string()
    } else if form.ends_with("ies") || form.ends_with('s') {
        "Plural / 3rd singular".to_string()
    } else {
        "Inflection".to_string()
    }
}

fn find_prefix_derivatives(
    conn: &Connection,
    word: &str,
    limit: usize,
) -> Result<Vec<RelatedWordItem>, DictionaryError> {
    if word.chars().count() < 4 {
        return Ok(Vec::new());
    }
    let upper = prefix_upper_bound(word);
    let min_len = (word.len() + 2) as i64;
    let max_len = (word.len() + 7) as i64;
    let mut stmt = conn.prepare(
        "SELECT w.word, s.pos, s.chinese_definition, s.english_definition
         FROM words w
         JOIN senses s ON s.word_id = w.id AND s.order_idx = 0
         WHERE w.word > ?1 AND w.word < ?2
           AND length(w.word) BETWEEN ?3 AND ?4
           AND instr(w.word, ' ') = 0 AND instr(w.word, '-') = 0
         ORDER BY CASE WHEN w.frequency_rank IS NOT NULL AND w.frequency_rank > 0 THEN 0 ELSE 1 END,
                  w.frequency_rank, w.word
         LIMIT ?5",
    )?;
    let rows = stmt
        .query_map(params![word, upper, min_len, max_len, limit as i64], |r| {
            let w: String = r.get(0)?;
            let pos: Option<String> = r.get(1)?;
            let zh: Option<String> = r.get(2)?;
            let en: String = r.get(3)?;
            let (clean_pos, clean_gloss) =
                roots::extract_pos_and_clean_gloss(pos, zh.as_deref(), Some(en.as_str()));
            Ok(RelatedWordItem {
                word: w,
                pos: clean_pos,
                gloss: clean_gloss,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn parse_tags(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if trimmed.starts_with('[') {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(trimmed) {
            return parsed
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    trimmed
        .split(|c: char| c.is_whitespace() || c == ',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn lookup_word_pos_and_gloss(
    conn: &Connection,
    target_word: &str,
) -> Result<Option<(Option<String>, String)>, DictionaryError> {
    let norm = normalize_word(target_word);
    if norm.is_empty() {
        return Ok(None);
    }
    let row: Option<(Option<String>, Option<String>, String)> = conn
        .query_row(
            "SELECT s.pos, s.chinese_definition, s.english_definition
             FROM words w
             JOIN senses s ON s.word_id = w.id
             WHERE w.word = ?1
             ORDER BY s.order_idx, s.id
             LIMIT 1",
            [&norm],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;

    if let Some((pos, zh, en)) = row {
        let (final_pos, clean_gloss) =
            roots::extract_pos_and_clean_gloss(pos, zh.as_deref(), Some(en.as_str()));
        return Ok(Some((final_pos, clean_gloss)));
    }
    Ok(None)
}

fn resolve_derivatives(
    conn: &Connection,
    word_family: &[String],
) -> Result<Vec<RelatedWordItem>, DictionaryError> {
    let mut out = Vec::with_capacity(word_family.len());
    for raw in word_family {
        let (w, parsed_pos, parsed_gloss) = roots::parse_word_family_entry(raw);
        if w.is_empty() {
            continue;
        }
        if let Some(gloss) = parsed_gloss.filter(|g| !g.trim().is_empty()) {
            out.push(RelatedWordItem {
                word: w,
                pos: parsed_pos,
                gloss,
            });
            continue;
        }
        if let Some((db_pos, db_gloss)) = lookup_word_pos_and_gloss(conn, &w)? {
            out.push(RelatedWordItem {
                word: w,
                pos: parsed_pos.or(db_pos),
                gloss: db_gloss,
            });
            continue;
        }
        let norm = normalize_word(&w);
        if let Some((fb_pos, fb_gloss)) = roots::fallback_related_gloss(&norm) {
            out.push(RelatedWordItem {
                word: w,
                pos: parsed_pos.or_else(|| Some(fb_pos.to_string())),
                gloss: fb_gloss.to_string(),
            });
        } else {
            out.push(RelatedWordItem {
                word: w,
                pos: parsed_pos,
                gloss: String::new(),
            });
        }
    }
    Ok(out)
}

fn resolve_related_words(
    conn: &Connection,
    words: &[String],
) -> Result<Vec<RelatedWordItem>, DictionaryError> {
    let mut out = Vec::with_capacity(words.len());
    for raw in words {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((db_pos, db_gloss)) = lookup_word_pos_and_gloss(conn, trimmed)? {
            out.push(RelatedWordItem {
                word: trimmed.to_string(),
                pos: db_pos,
                gloss: db_gloss,
            });
            continue;
        }
        let (parsed_w, parsed_pos, parsed_gloss) = roots::parse_word_family_entry(trimmed);
        if parsed_pos.is_some() {
            if let Some(gloss) = parsed_gloss.filter(|g| !g.trim().is_empty()) {
                out.push(RelatedWordItem {
                    word: parsed_w,
                    pos: parsed_pos,
                    gloss,
                });
                continue;
            }
        }
        let norm = normalize_word(trimmed);
        if let Some((fb_pos, fb_gloss)) = roots::fallback_related_gloss(&norm) {
            out.push(RelatedWordItem {
                word: trimmed.to_string(),
                pos: Some(fb_pos.to_string()),
                gloss: fb_gloss.to_string(),
            });
        } else {
            out.push(RelatedWordItem {
                word: trimmed.to_string(),
                pos: None,
                gloss: String::new(),
            });
        }
    }
    Ok(out)
}

/// 用首字母 + 词长 ±2 范围查询 words 表，结合编辑距离（Levenshtein 1..=2）选出最多 `limit` 个形近词。
fn find_lookalikes(
    conn: &Connection,
    word: &str,
    limit: usize,
) -> Result<Vec<RelatedWordItem>, DictionaryError> {
    let Some(first_char) = word.chars().next() else {
        return Ok(Vec::new());
    };
    let lower_bound = first_char.to_string();
    let upper_bound = prefix_upper_bound(&lower_bound);
    let char_len = word.chars().count() as i64;
    let min_len = (char_len - 2).max(1);
    let max_len = char_len + 2;

    let mut stmt = conn.prepare(
        "SELECT w.id, w.word, w.frequency_rank
         FROM words w
         WHERE w.word >= ?1 AND w.word < ?2
           AND length(w.word) BETWEEN ?3 AND ?4
           AND w.word <> ?5
         ORDER BY CASE WHEN w.frequency_rank IS NOT NULL AND w.frequency_rank > 0 THEN 0 ELSE 1 END,
                  w.frequency_rank, w.word
         LIMIT 500",
    )?;

    let candidates = stmt
        .query_map(
            params![lower_bound, upper_bound, min_len, max_len, word],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;

    let mut matched: Vec<(usize, usize, i64, i64, String)> = Vec::new();
    for (cand_id, cand_word, freq) in candidates {
        let dist = roots::levenshtein(word, &cand_word);
        if (1..=2).contains(&dist) {
            let len_diff = cand_word.chars().count().abs_diff(word.chars().count());
            let rank = freq.filter(|&v| v > 0).unwrap_or(i64::MAX);
            matched.push((dist, len_diff, rank, cand_id, cand_word));
        }
    }

    matched.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.4.cmp(&b.4))
    });
    matched.truncate(limit);

    let mut out = Vec::with_capacity(matched.len());
    for (_, _, _, cand_id, cand_word) in matched {
        let sense_row: Option<(Option<String>, Option<String>, String)> = conn
            .query_row(
                "SELECT pos, chinese_definition, english_definition
                 FROM senses WHERE word_id = ?1 ORDER BY order_idx, id LIMIT 1",
                [cand_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let (pos, gloss) = match sense_row {
            Some((p, zh, en)) => {
                roots::extract_pos_and_clean_gloss(p, zh.as_deref(), Some(en.as_str()))
            }
            None => (None, String::new()),
        };
        out.push(RelatedWordItem {
            word: cand_word,
            pos,
            gloss,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_conn;
    use crate::dictionary::provider::{DictionaryProvider, SeedProvider};

    #[test]
    fn prefix_upper_bound_covers_range() {
        assert_eq!(prefix_upper_bound("met"), "meu");
        assert_eq!(prefix_upper_bound("a"), "b");
        // 尾字符递增落入代理区时继续回退
        assert_eq!(prefix_upper_bound(""), "\u{10FFFF}");
        // 非 ASCII 尾字符
        assert_eq!(prefix_upper_bound("é"), "ê");
    }

    #[test]
    fn surgery_entry_loads_all_8_enrichment_sections() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();

        let entry = word_by_normalized(&conn, "surgery")
            .unwrap()
            .expect("surgery must exist in seed");
        assert_eq!(entry.phonetic.as_deref(), Some("/ˈsɜːdʒəri/"));
        assert_eq!(
            entry.tags,
            vec!["cet4", "cet6", "ky", "ielts", "toefl", "oxford"]
        );
        assert!(entry.oxford);
        assert_eq!(entry.senses.len(), 4);
        assert_eq!(entry.senses[0].pos.as_deref(), Some("noun"));
        assert_eq!(entry.senses[0].level.as_deref(), Some("B2"));
        assert_eq!(entry.senses[1].level.as_deref(), Some("B2"));
        assert_eq!(entry.senses[2].level.as_deref(), Some("R"));
        assert_eq!(entry.senses[3].level.as_deref(), Some("R"));
        assert_eq!(
            entry.mnemonic.as_deref(),
            Some("surgery 来自希腊语 cheirourgia，cheir 表示手，ergon 意为工作，指用手操作的外科手术，在医疗领域至关重要。")
        );
        assert_eq!(
            entry.roots,
            vec![
                RootPart {
                    part: "surg".into(),
                    meaning: "上升，升起".into(),
                },
                RootPart {
                    part: "-ory²".into(),
                    meaning: "构成名词，表示‘...的地方’或‘与...相关的事物’".into(),
                },
            ]
        );
        assert_eq!(
            entry.derivatives,
            vec![RelatedWordItem {
                word: "microsurgery".into(),
                pos: Some("n.".into()),
                gloss: "显微外科".into(),
            }]
        );
        assert_eq!(
            entry.forms,
            vec![WordFormEntry {
                label: "Plural".into(),
                form: "surgeries".into(),
            }]
        );
        let lookalike_words: Vec<&str> = entry.lookalikes.iter().map(|l| l.word.as_str()).collect();
        assert!(lookalike_words.contains(&"sugary"), "{lookalike_words:?}");
        assert!(lookalike_words.contains(&"surely"), "{lookalike_words:?}");
        assert!(lookalike_words.contains(&"surety"), "{lookalike_words:?}");
        assert!(entry.exam_example.is_some());
    }
}
