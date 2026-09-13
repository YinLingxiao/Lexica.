//! 数据源抽象（导入侧）。查词永远查本地 SQLite；provider 负责把外部数据灌入库。
//! 未来接入 ECDICT / 在线 API = 新增一个实现，不动引擎。

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{params, Connection, OpenFlags};
use serde::Deserialize;

use super::engine::normalize_word;
use super::DictionaryError;
use crate::database::fmt_ts;

pub trait DictionaryProvider {
    /// 数据源标识（写入 words.source，追溯数据来源）。
    fn name(&self) -> &'static str;
    /// 幂等导入。返回本次新导入的词条数。
    fn import(&self, conn: &Connection) -> Result<usize, DictionaryError>;
}

/// 内嵌种子词库（原创内容，见 seed/seed.json）。
pub struct SeedProvider;

impl DictionaryProvider for SeedProvider {
    fn name(&self) -> &'static str {
        "seed"
    }

    fn import(&self, conn: &Connection) -> Result<usize, DictionaryError> {
        let seed: SeedFile = serde_json::from_str(SEED_JSON)?;
        let now = fmt_ts(Utc::now());
        let mut imported = 0;

        let tx = conn.unchecked_transaction()?;
        for w in &seed.words {
            let word = normalize_word(&w.word);
            if word.is_empty() || w.senses.is_empty() {
                continue; // 词库中不应有无义项词条
            }
            tx.execute(
                "INSERT OR IGNORE INTO words
				 (word, display, phonetic, frequency_rank, synonyms, antonyms, word_family, source, created_at)
				 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'seed', ?8)",
                params![
                    word,
                    w.word,
                    w.phonetic,
                    w.frequency_rank,
                    serde_json::to_string(&w.synonyms).unwrap_or_else(|_| "[]".into()),
                    serde_json::to_string(&w.antonyms).unwrap_or_else(|_| "[]".into()),
                    serde_json::to_string(&w.word_family).unwrap_or_else(|_| "[]".into()),
                    now
                ],
            )?;
            if tx.changes() == 0 {
                continue; // 已存在：幂等跳过（子行也不重复插入）
            }
            imported += 1;
            let word_id = tx.last_insert_rowid();

            for (i, s) in w.senses.iter().enumerate() {
                tx.execute(
					"INSERT INTO senses (word_id, pos, english_definition, chinese_definition, level, order_idx)
					 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
					params![word_id, s.pos, s.english, s.chinese, s.level, i as i64],
				)?;
                let sense_id = tx.last_insert_rowid();
                for (j, ex) in s.examples.iter().enumerate() {
                    tx.execute(
                        "INSERT INTO examples (sense_id, text, translation, order_idx)
						 VALUES (?1, ?2, ?3, ?4)",
                        params![sense_id, ex.text, ex.translation, j as i64],
                    )?;
                }
            }

            for (i, c) in w.collocations.iter().enumerate() {
                tx.execute(
                    "INSERT INTO collocations (word_id, text, gloss, order_idx)
					 VALUES (?1, ?2, ?3, ?4)",
                    params![word_id, c.text, c.gloss, i as i64],
                )?;
            }

            for form in w.forms.values() {
                let f = normalize_word(form);
                if !f.is_empty() && f != word {
                    tx.execute(
                        "INSERT OR IGNORE INTO word_forms (form, word_id) VALUES (?1, ?2)",
                        params![f, word_id],
                    )?;
                }
            }
        }
        tx.commit()?;
        Ok(imported)
    }
}

const SEED_JSON: &str = include_str!("seed/seed.json");

/// ECDICT 全量词库（MIT 许可，https://github.com/skywind3000/ECDICT）。
/// `source_path` 指向官方发布包内的 stardict.db（SQLite，340 万词条）。
///
/// 映射策略：
/// - `definition`（英）/`translation`（中）按行拆义项；两侧行数相等时逐行配对，
///   不相等（或只有单侧）时合并为一条义项——ECDICT 两侧行数经常不齐。
/// - `frq`（当代语料排名）优先、`bnc` 兜底 → `frequency_rank`。
/// - `exchange`（i:going/p:went/...）→ `word_forms` 屈折映射。
/// - phonetic 原始值不带斜杠，统一包成 `/.../`。
/// - 与种子词冲突时种子优先（INSERT OR IGNORE，种子先行导入）。
pub struct EcdictProvider {
    pub source_path: PathBuf,
}

impl DictionaryProvider for EcdictProvider {
    fn name(&self) -> &'static str {
        "ecdict"
    }

    fn import(&self, conn: &Connection) -> Result<usize, DictionaryError> {
        let source =
            Connection::open_with_flags(&self.source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Self::import_from_source(&source, conn)
    }
}

impl EcdictProvider {
    /// 校验源库结构并执行导入。source 与目标分离，测试可直接喂内存库。
    pub fn import_from_source(
        source: &Connection,
        conn: &Connection,
    ) -> Result<usize, DictionaryError> {
        let has_table: i64 = source.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'stardict'",
            [],
            |r| r.get::<_, i64>(0),
        )?;
        if has_table == 0 {
            return Err(DictionaryError::InvalidSource(
                "not an ECDICT stardict.db (table `stardict` missing)".into(),
            ));
        }

        let mut ins_word = conn.prepare(
            "INSERT OR IGNORE INTO words
			 (word, display, phonetic, frequency_rank, synonyms, antonyms, word_family, source, created_at)
			 VALUES (?1, ?2, ?3, ?4, '[]', '[]', '[]', 'ecdict', ?5)",
        )?;
        let mut ins_sense = conn.prepare(
            "INSERT INTO senses (word_id, pos, english_definition, chinese_definition, level, order_idx)
			 VALUES (?1, NULL, ?2, ?3, NULL, 0)",
        )?;
        let mut ins_form =
            conn.prepare("INSERT OR IGNORE INTO word_forms (form, word_id) VALUES (?1, ?2)")?;

        let mut select = source.prepare(
            // 有词频的实词先行：normalize 会剥掉注音行的引号（stardict 首行 "'a" → a），
            // 若垃圾行先到会占住 normalized 词位，让真正的 a/and/as 等高频词拿不到词频。
            "SELECT word, phonetic, definition, translation, frq, bnc, exchange FROM stardict
             ORDER BY (COALESCE(frq, 0) > 0 OR COALESCE(bnc, 0) > 0) DESC, id",
        )?;
        let mut rows = select.query([])?;

        let now = fmt_ts(Utc::now());
        let mut imported = 0usize;
        let mut in_chunk = 0usize;
        let mut tx = conn.unchecked_transaction()?;

        while let Some(row) = rows.next()? {
            let raw_word: Option<String> = row.get(0)?;
            let raw_word = raw_word.as_deref().unwrap_or("").trim();
            if raw_word.is_empty() {
                continue;
            }
            let word = normalize_word(raw_word);
            if word.is_empty() {
                continue;
            }
            let definition: Option<String> = row.get(2)?;
            let translation: Option<String> = row.get(3)?;
            let zh_lines = split_lines(translation.as_deref());
            let en_lines = split_lines(definition.as_deref());
            if en_lines.is_empty() && zh_lines.is_empty() {
                continue; // 无任何释义内容，不值得占一个词条
            }

            let phonetic: Option<String> = row.get(1)?;
            let phonetic = phonetic
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .map(|p| {
                    if p.starts_with('/') {
                        p
                    } else {
                        format!("/{p}/")
                    }
                });
            let frq: Option<i64> = row.get(4)?;
            let bnc: Option<i64> = row.get(5)?;
            let rank = frq.filter(|v| *v > 0).or(bnc.filter(|v| *v > 0));

            ins_word.execute(params![word, raw_word, phonetic, rank, now])?;
            if conn.changes() == 0 {
                continue; // 已存在（种子或上次的导入）：幂等跳过
            }
            imported += 1;
            in_chunk += 1;
            let word_id = conn.last_insert_rowid();

            // 义项：行数相等逐行配对，否则合并单条（english_definition 非空约束允许 ""）。
            let senses: Vec<(String, Option<String>)> =
                if en_lines.len() == zh_lines.len() && en_lines.len() > 1 {
                    en_lines
                        .iter()
                        .zip(zh_lines.iter())
                        .map(|(en, zh)| ((*en).to_string(), Some((*zh).to_string())))
                        .collect()
                } else {
                    let zh = (!zh_lines.is_empty()).then(|| zh_lines.join("\n"));
                    vec![(en_lines.join("\n"), zh)]
                };
            for (en, zh) in senses {
                ins_sense.execute(params![word_id, en, zh])?;
            }

            // 屈折：exchange 形如 "i:going/p:went/d:gone/3:goes/s:goes"。
            let exchange: Option<String> = row.get(6)?;
            if let Some(exchange) = exchange {
                for token in exchange.split('/') {
                    let Some((_kind, form)) = token.split_once(':') else {
                        continue;
                    };
                    let form = normalize_word(form);
                    if !form.is_empty() && form != word {
                        ins_form.execute(params![form, word_id])?;
                    }
                }
            }

            // 分块提交：340 万词条一次事务太大，每 2 万词刷一次盘；
            // 期间应用连接的其他写入只需短暂等待 busy_timeout。
            if in_chunk >= 20_000 {
                tx.commit()?;
                in_chunk = 0;
                tx = conn.unchecked_transaction()?;
            }
        }
        tx.commit()?;
        Ok(imported)
    }
}

/// 拆多行字段：trim 每行、丢空行。
fn split_lines(value: Option<&str>) -> Vec<&str> {
    value
        .unwrap_or("")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect()
}

// ── 种子 JSON 的反序列化模型 ───────────────────────────────

#[derive(Debug, Deserialize)]
struct SeedFile {
    #[allow(dead_code)]
    version: u32,
    #[serde(default)]
    words: Vec<SeedWord>,
}

#[derive(Debug, Deserialize)]
struct SeedWord {
    word: String,
    phonetic: Option<String>,
    frequency_rank: Option<i64>,
    #[serde(default)]
    senses: Vec<SeedSense>,
    #[serde(default)]
    collocations: Vec<SeedCollocation>,
    #[serde(default)]
    synonyms: Vec<String>,
    #[serde(default)]
    antonyms: Vec<String>,
    #[serde(default)]
    word_family: Vec<String>,
    #[serde(default)]
    forms: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct SeedSense {
    pos: Option<String>,
    english: String,
    chinese: Option<String>,
    level: Option<String>,
    #[serde(default)]
    examples: Vec<SeedExample>,
}

#[derive(Debug, Deserialize)]
struct SeedExample {
    text: String,
    translation: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SeedCollocation {
    text: String,
    gloss: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_conn;

    #[test]
    fn seed_import_is_idempotent() {
        let conn = test_conn();
        let first = SeedProvider.import(&conn).unwrap();
        assert!(first >= 20, "expected a real seed batch, got {first}");

        let words_after_first: i64 = conn
            .query_row("SELECT COUNT(*) FROM words", [], |r| r.get::<_, i64>(0))
            .unwrap();
        let senses_after_first: i64 = conn
            .query_row("SELECT COUNT(*) FROM senses", [], |r| r.get::<_, i64>(0))
            .unwrap();
        let examples_after_first: i64 = conn
            .query_row("SELECT COUNT(*) FROM examples", [], |r| r.get::<_, i64>(0))
            .unwrap();
        let forms_after_first: i64 = conn
            .query_row("SELECT COUNT(*) FROM word_forms", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();

        // 第二次导入：0 新增，所有计数不变
        let second = SeedProvider.import(&conn).unwrap();
        assert_eq!(second, 0);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM words", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            words_after_first
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM senses", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            senses_after_first
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM examples", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            examples_after_first
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM word_forms", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            forms_after_first
        );
    }

    #[test]
    fn seed_import_populates_fts() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let hits: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM words_fts WHERE words_fts MATCH 'word:meticulous'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(hits, 1);
    }

    // ── EcdictProvider ────────────────────────────────────

    /// 仿 stardict.db 结构的内存源库。
    fn fixture_source() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE stardict (
                id INTEGER PRIMARY KEY,
                word TEXT, sw TEXT, phonetic TEXT, definition TEXT, translation TEXT,
                pos TEXT, collins INTEGER, oxford INTEGER, tag TEXT,
                bnc INTEGER, frq INTEGER, exchange TEXT, detail TEXT, audio TEXT
            );
            INSERT INTO stardict
                (word, phonetic, definition, translation, pos, collins, oxford, tag, bnc, frq, exchange)
            VALUES
                ('''a', NULL, NULL, NULL, NULL, 0, 0, NULL, NULL, NULL, NULL),
                ('meticulous', 'mә''tikjulәs',
                 's. marked by precise accordance with details\ns. marked by extreme care',
                 'a. 一丝不苟的, 过细的', 'j:100', 1, 0, 'toefl', 11166, 11107, ''),
                ('go', 'gou', 'v. move away\nv. proceed',
                 'vi. 去\nvt. 进行', 'v:100', 0, 1, 'cet4', 21, 18,
                 'i:going/p:went/d:gone/3:goes/s:goes'),
                ('emptyword', NULL, NULL, NULL, NULL, 0, 0, NULL, 0, 0, NULL),
                ('a', NULL, 'art. one of a set', 'art. 一个', NULL, 0, 0, NULL, 5, 5, NULL);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn ecdict_import_maps_fields_and_forms() {
        let source = fixture_source();
        let conn = test_conn();

        // 先种下种子词：同词冲突时种子优先
        SeedProvider.import(&conn).unwrap();
        let seed_meticulous: i64 = conn
            .query_row("SELECT id FROM words WHERE word = 'meticulous'", [], |r| {
                r.get(0)
            })
            .unwrap();

        let imported = EcdictProvider::import_from_source(&source, &conn).unwrap();
        assert_eq!(imported, 2, "meticulous 撞种子被跳过，go 和 a 是新词");

        // 注音垃圾行 "'a"（无词频）排在实词 'a'（frq=5）之前，
        // 词频优先的导入顺序必须让实词赢下 normalized 词位。
        let a: (String, Option<i64>) = conn
            .query_row(
                "SELECT display, frequency_rank FROM words WHERE word = 'a'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(a.0, "a", "实词行应赢过注音行");
        assert_eq!(a.1, Some(5));

        let go: (i64, String, Option<String>, Option<i64>) = conn
            .query_row(
                "SELECT id, display, phonetic, frequency_rank FROM words WHERE word = 'go'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        let (go_id, display, phonetic, rank) = go;
        assert_eq!(display, "go");
        assert_eq!(phonetic.as_deref(), Some("/gou/"));
        assert_eq!(rank, Some(18));

        // go 的 definition/translation 各 2 行 → 逐行配对成 2 个义项
        let senses: Vec<(String, Option<String>)> = {
            let mut stmt = conn
                .prepare(
                    "SELECT english_definition, chinese_definition FROM senses
                     WHERE word_id = ?1 ORDER BY id",
                )
                .unwrap();
            let rows = stmt
                .query_map([go_id], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap();
            rows.map(|r| r.unwrap()).collect()
        };
        assert_eq!(
            senses,
            vec![
                ("v. move away".into(), Some("vi. 去".into())),
                ("v. proceed".into(), Some("vt. 进行".into()))
            ]
        );

        // exchange → word_forms
        for form in ["going", "went", "gone", "goes"] {
            let mapped: i64 = conn
                .query_row(
                    "SELECT word_id FROM word_forms WHERE form = ?1",
                    [form],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(mapped, go_id, "form {form} → go");
        }

        // 空内容词条不导入
        let empty: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM words WHERE word = 'emptyword'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(empty, 0);

        // 种子词未被覆盖
        let still_seed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM words WHERE word = 'meticulous' AND id = ?1 AND source = 'seed'",
                [seed_meticulous],
                |r| r.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(still_seed, 1);

        // 幂等：再跑一遍 0 新增
        let again = EcdictProvider::import_from_source(&source, &conn).unwrap();
        assert_eq!(again, 0);
    }

    #[test]
    fn ecdict_import_rejects_foreign_source() {
        let source = Connection::open_in_memory().unwrap();
        let conn = test_conn();
        let err = EcdictProvider::import_from_source(&source, &conn).unwrap_err();
        assert!(matches!(err, super::DictionaryError::InvalidSource(_)));
    }

    #[test]
    fn ecdict_paired_lines_become_separate_senses() {
        let source = Connection::open_in_memory().unwrap();
        source
            .execute_batch(
                "CREATE TABLE stardict (
                    id INTEGER PRIMARY KEY, word TEXT, sw TEXT, phonetic TEXT,
                    definition TEXT, translation TEXT, pos TEXT, collins INTEGER,
                    oxford INTEGER, tag TEXT, bnc INTEGER, frq INTEGER,
                    exchange TEXT, detail TEXT, audio TEXT
                );
                INSERT INTO stardict (word, definition, translation)
                VALUES ('bank', 'n. a financial institution\nn. sloping land beside water',
                        'n. 银行\nn. 河岸'),
                       ('fast', 'adj. moving quickly\nadv. firmly fixed',
                        'adj. 快的');",
            )
            .unwrap();
        let conn = test_conn();
        EcdictProvider::import_from_source(&source, &conn).unwrap();
        let senses: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM senses s JOIN words w ON w.id = s.word_id
                 WHERE w.word = 'bank'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(senses, 2, "行数相等时逐行配对成两个义项");

        // 行数不齐（2 英 vs 1 中）→ 合并单义项
        let fast: (i64, String, Option<String>) = conn
            .query_row(
                "SELECT COUNT(s.id), s.english_definition, s.chinese_definition
                 FROM senses s JOIN words w ON w.id = s.word_id WHERE w.word = 'fast'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(fast.0, 1);
        assert_eq!(fast.1, "adj. moving quickly\nadv. firmly fixed");
        assert_eq!(fast.2.as_deref(), Some("adj. 快的"));
    }
}
