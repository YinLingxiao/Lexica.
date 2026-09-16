//! AI 例句补充（可选、默认关闭）。
//!
//! 职责边界：查词仅在缺少词典例句时补 AI 例句；复习填空句统一由 AI 生成；判分永远在本地。
//! - 非敏感配置（开关/服务地址/模型）存 SQLite `app_settings`；
//!   密钥存 Windows 凭据管理器（keyring），绝不入库、不入 localStorage、不入日志。
//! - 有效例句缓存到 `ai_example_cache`，键 = 词 + 义项内容 + 服务地址 + 模型 + 提示词版本。
//! - 网络走 reqwest blocking（15s 超时、不自动重试）；调用方在 spawn_blocking 中执行，
//!   **等待网络期间不持有数据库锁**（配置与缓存在 HTTP 前后各自短锁读写）。
//! - 模型输出经 Rust 验证（ID 命中请求集、句子非空、目标词完整词边界出现）
//!   并挖空全部目标词位置后才交给前端；不合规条目直接丢弃 → 回退本地题目。

use std::sync::Mutex;
use std::time::Duration;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::database::{fmt_ts, DbError};
use crate::dictionary::engine::normalize_word;
use crate::error::AppError;
use crate::review::grader;

/// 提示词版本：进入缓存键，改提示词即自然失效旧缓存。
pub const PROMPT_VERSION: u32 = 2;
/// 每批请求超时（不做自动重试——失败即回退本地题目）。
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
pub const DEFAULT_AI_BASE_URL: &str = "https://api.deepseek.com/v1";
pub const DEFAULT_AI_MODEL: &str = "deepseek-chat";

const CHAT_PATH: &str = "chat/completions";
const KEY_SERVICE: &str = "com.lexica.app";
const KEY_ACCOUNT: &str = "ai_api_key";

/// 发给前端的配置视图：只报告 has_key，绝不回传密钥本身。
#[derive(Debug, Clone, Serialize)]
pub struct AiConfigView {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub has_key: bool,
}

/// 内部配置（无密钥）。
#[derive(Debug, Clone, Default)]
pub struct AiConfig {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
}

/// 一条待生成词（词 + 选定义项），由命令层从 DB 装配。
#[derive(Debug, Clone)]
pub struct GenWord {
    pub word_id: i64,
    pub sense_id: Option<i64>,
    pub word: String,
    pub english: String,
    pub chinese: Option<String>,
}

/// 生成结果：已验证、已挖空的题目文本（前端直接当 prompt 用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiExample {
    pub word_id: i64,
    pub sense_id: Option<i64>,
    pub sentence: String,
}

// ── 密钥存储 ──────────────────────────────────────────────────

/// 密钥存取抽象。生产实现 = Windows 凭据管理器；测试用内存实现。
pub trait KeyStore: Send + Sync {
    fn get(&self) -> Result<Option<String>, AppError>;
    fn set(&self, key: &str) -> Result<(), AppError>;
    fn delete(&self) -> Result<(), AppError>;
}

/// Windows 凭据管理器实现。
pub struct WindowsKeyStore;

impl KeyStore for WindowsKeyStore {
    fn get(&self) -> Result<Option<String>, AppError> {
        let entry = keyring::Entry::new(KEY_SERVICE, KEY_ACCOUNT)
            .map_err(|e| AppError::Io(format!("credential store: {e}")))?;
        match entry.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::Io(format!("credential store: {e}"))),
        }
    }

    fn set(&self, key: &str) -> Result<(), AppError> {
        let entry = keyring::Entry::new(KEY_SERVICE, KEY_ACCOUNT)
            .map_err(|e| AppError::Io(format!("credential store: {e}")))?;
        entry
            .set_password(key)
            .map_err(|e| AppError::Io(format!("credential store: {e}")))
    }

    fn delete(&self) -> Result<(), AppError> {
        let entry = keyring::Entry::new(KEY_SERVICE, KEY_ACCOUNT)
            .map_err(|e| AppError::Io(format!("credential store: {e}")))?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            // 幂等：本就没有密钥视为删除成功。
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Io(format!("credential store: {e}"))),
        }
    }
}

/// 生产密钥库。
pub fn key_store() -> &'static dyn KeyStore {
    &WindowsKeyStore
}

/// 测试用内存实现（不触碰真实凭据库）。
pub struct MemoryKeyStore(Mutex<Option<String>>);

impl MemoryKeyStore {
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }
}

impl Default for MemoryKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyStore for MemoryKeyStore {
    fn get(&self) -> Result<Option<String>, AppError> {
        Ok(self.0.lock().map(|g| g.clone()).unwrap_or(None))
    }

    fn set(&self, key: &str) -> Result<(), AppError> {
        if let Ok(mut g) = self.0.lock() {
            *g = Some(key.to_string());
        }
        Ok(())
    }

    fn delete(&self) -> Result<(), AppError> {
        if let Ok(mut g) = self.0.lock() {
            *g = None;
        }
        Ok(())
    }
}

// ── 配置（app_settings）──────────────────────────────────────

/// 读取配置。AI 默认关闭，但服务地址和模型预填 DeepSeek；已有自定义值优先。
pub fn load_config(conn: &Connection) -> Result<AiConfig, DbError> {
    let get = |k: &str| -> Result<Option<String>, DbError> {
        Ok(conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                [k],
                |r| r.get(0),
            )
            .optional()?)
    };
    let base_url = get("ai_base_url")?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_AI_BASE_URL.to_string());
    let model = get("ai_model")?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_AI_MODEL.to_string());
    Ok(AiConfig {
        enabled: get("ai_enabled")?.as_deref() == Some("1"),
        base_url,
        model,
    })
}

pub fn save_config(
    conn: &Connection,
    enabled: bool,
    base_url: &str,
    model: &str,
) -> Result<(), DbError> {
    let rows = [
        ("ai_enabled", if enabled { "1" } else { "0" }),
        ("ai_base_url", base_url.trim()),
        ("ai_model", model.trim()),
    ];
    for (k, v) in rows {
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![k, v],
        )?;
    }
    Ok(())
}

// ── 词条装配与缓存 ───────────────────────────────────────────

/// 从 DB 装配一条待生成词。sense_id 必须属于该词；
/// 未指定义项时取第一条非空英文释义的义项，兜底第一条义项。
pub fn load_gen_word(
    conn: &Connection,
    word_id: i64,
    sense_id: Option<i64>,
) -> Result<Option<GenWord>, DbError> {
    let word: Option<String> = conn
        .query_row("SELECT word FROM words WHERE id = ?1", [word_id], |r| r.get(0))
        .optional()?;
    let Some(word) = word else {
        return Ok(None);
    };
    let sense: Option<(i64, String, Option<String>)> = match sense_id {
        Some(sid) => conn
            .query_row(
                "SELECT id, english_definition, chinese_definition FROM senses
                 WHERE id = ?1 AND word_id = ?2",
                params![sid, word_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?,
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, english_definition, chinese_definition FROM senses
                 WHERE word_id = ?1 ORDER BY order_idx, id",
            )?;
            let rows = stmt
                .query_map([word_id], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            // 无非空英文释义的义项时兜底第一条义项。
            let chosen = rows
                .iter()
                .find(|(_, en, _)| !en.trim().is_empty())
                .cloned()
                .or_else(|| rows.first().cloned());
            chosen
        }
    };
    let Some((sid, english, chinese)) = sense else {
        return Ok(None);
    };
    Ok(Some(GenWord {
        word_id,
        sense_id: Some(sid),
        word,
        english,
        chinese: chinese.filter(|c| !c.trim().is_empty()),
    }))
}

/// 缓存键：词 + 义项内容 + 服务地址（规范化尾部斜杠）+ 模型 + 提示词版本。
fn cache_key(w: &GenWord, cfg: &AiConfig) -> String {
    format!(
        "v{PROMPT_VERSION}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        normalize_word(&w.word),
        w.english.trim(),
        w.chinese.as_deref().unwrap_or("").trim(),
        cfg.base_url.trim_end_matches('/'),
        cfg.model.trim()
    )
}

/// 读缓存（命中即免网络）。
pub fn cache_get(
    conn: &Connection,
    words: &[GenWord],
    cfg: &AiConfig,
) -> Result<Vec<AiExample>, DbError> {
    let mut out = Vec::new();
    for w in words {
        let key = cache_key(w, cfg);
        let row: Option<(Option<i64>, String)> = conn
            .query_row(
                "SELECT sense_id, prompt_text FROM ai_example_cache WHERE cache_key = ?1",
                [&key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((sense_id, prompt_text)) = row {
            out.push(AiExample {
                word_id: w.word_id,
                sense_id,
                sentence: prompt_text,
            });
        }
    }
    Ok(out)
}

/// 写缓存（prompt_text 已挖空；同键覆盖）。
pub fn cache_put(
    conn: &Connection,
    words: &[GenWord],
    cfg: &AiConfig,
    examples: &[AiExample],
) -> Result<(), DbError> {
    let now = fmt_ts(Utc::now());
    for ex in examples {
        let Some(w) = words.iter().find(|w| w.word_id == ex.word_id) else {
            continue;
        };
        conn.execute(
            "INSERT OR REPLACE INTO ai_example_cache
             (cache_key, word_id, sense_id, prompt_text, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![cache_key(w, cfg), ex.word_id, ex.sense_id, ex.sentence, now],
        )?;
    }
    Ok(())
}

// ── 验证与挖空 ───────────────────────────────────────────────

/// 验证模型输出并挖空目标词，返回可直接用作题目的条目。
/// 规则：word_id 必须命中请求集；sense_id 与请求一致；句子非空；
/// 目标词须以完整词边界出现（大小写不敏感）。不合规条目被丢弃。
pub fn validate_and_blank(content: &str, words: &[GenWord]) -> Vec<AiExample> {
    let mut out = Vec::new();
    for item in parse_json_items(content) {
        let Some(word_id) = item.get("word_id").and_then(|v| v.as_i64()) else {
            continue;
        };
        let Some(w) = words.iter().find(|w| w.word_id == word_id) else {
            continue;
        };
        let Some(sentence) = item.get("sentence").and_then(|v| v.as_str()) else {
            continue;
        };
        let sentence = sentence.trim();
        if sentence.is_empty() {
            continue;
        }
        // Short enough to read at a glance on the dictionary card.
        if sentence.chars().count() > 160 || !(4..=20).contains(&sentence.split_whitespace().count()) {
            continue;
        }
        if let Some(sense_id) = item.get("sense_id").and_then(|v| v.as_i64()) {
            if w.sense_id != Some(sense_id) {
                continue;
            }
        }
        // blank_word 同时完成"完整词边界匹配"验证与"挖空全部出现位置"。
        let Some(blanked) = grader::blank_word(sentence, &w.word) else {
            continue;
        };
        out.push(AiExample {
            word_id: w.word_id,
            sense_id: w.sense_id,
            sentence: blanked,
        });
    }
    out
}

/// 从模型 content 提取 JSON 数组；剥代码围栏，容忍数组外的前后说明文字。
fn parse_json_items(content: &str) -> Vec<serde_json::Value> {
    let mut text = content.trim();
    if let Some(rest) = text.strip_prefix("```json") {
        text = rest;
    } else if let Some(rest) = text.strip_prefix("```") {
        text = rest;
    }
    if let Some(rest) = text.strip_suffix("```") {
        text = rest;
    }
    let text = text.trim();
    let slice = match (text.find('['), text.rfind(']')) {
        (Some(s), Some(e)) if e > s => &text[s..=e],
        _ => text,
    };
    serde_json::from_str(slice).unwrap_or_default()
}

// ── 网络请求 ─────────────────────────────────────────────────

fn chat_url(cfg: &AiConfig) -> Result<String, AppError> {
    let base = cfg.base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err(AppError::Network("AI service address is not configured".into()));
    }
    Ok(format!("{base}/{CHAT_PATH}"))
}

fn system_prompt() -> String {
    "You write one short English example sentence per vocabulary word for a learner. \
     Use a concrete, everyday situation and clear B1-level language. \
     Each sentence must be natural and self-contained, 6-14 words, and use the target word \
     exactly in its given base form as a whole word. Match the provided sense, not another meaning. \
     Avoid quotations, definitions disguised as sentences, rare names, and sensitive topics. \
     Reply with a single JSON array, one object per requested word in the same order, \
     each object exactly {\"word_id\": <number>, \"sense_id\": <number or null>, \
     \"sentence\": \"<sentence>\"}. No notes, no translations, no extra text."
        .to_string()
}

/// 请求正文只包含目标词与选定义项——不含笔记、学习历史或任何个人数据。
fn user_prompt(words: &[GenWord]) -> String {
    let mut lines = vec![String::from("Words:")];
    for w in words {
        lines.push(format!(
            "- word_id: {} | sense_id: {} | word: {} | english: {} | chinese: {}",
            w.word_id,
            w.sense_id
                .map(|s| s.to_string())
                .unwrap_or_else(|| "null".into()),
            w.word,
            w.english.trim(),
            w.chinese.as_deref().unwrap_or("").trim()
        ));
    }
    lines.join("\n")
}

/// 批量生成（阻塞调用，须在 spawn_blocking 中执行）。
/// 超时/HTTP 错误/非法响应一律 Err，由上层回退本地题目；不自动重试。
pub fn generate_batch(
    words: &[GenWord],
    cfg: &AiConfig,
    api_key: &str,
    timeout: Duration,
) -> Result<Vec<AiExample>, AppError> {
    if words.is_empty() {
        return Ok(vec![]);
    }
    let url = chat_url(cfg)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;
    let body = serde_json::json!({
        "model": cfg.model.trim(),
        "messages": [
            {"role": "system", "content": system_prompt()},
            {"role": "user", "content": user_prompt(words)}
        ]
    });
    // reqwest 未启用 json feature（依赖保持最小），手动序列化并声明 Content-Type。
    let payload = serde_json::to_vec(&body).map_err(|e| AppError::Network(e.to_string()))?;
    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .header("content-type", "application/json")
        .body(payload)
        .send()
        .map_err(|e| AppError::Network(e.to_string()))?;
    let status = resp.status();
    let text = resp
        .text()
        .map_err(|e| AppError::Network(e.to_string()))?;
    if !status.is_success() {
        return Err(AppError::Network(format!("AI service returned {status}")));
    }
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| AppError::Network(format!("AI response is not JSON: {e}")))?;
    let content = value
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            AppError::Network("AI response missing choices[0].message.content".into())
        })?;
    Ok(validate_and_blank(content, words))
}

/// 连接测试：一次最小请求验证地址、模型与密钥。
pub fn test_connection(cfg: &AiConfig, api_key: &str) -> Result<(), AppError> {
    let url = chat_url(cfg)?;
    if cfg.model.trim().is_empty() {
        return Err(AppError::Network("AI model is not configured".into()));
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;
    let body = serde_json::json!({
        "model": cfg.model.trim(),
        "messages": [{"role": "user", "content": "Reply with the single word: OK"}],
        "max_tokens": 8
    });
    let payload = serde_json::to_vec(&body).map_err(|e| AppError::Network(e.to_string()))?;
    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .header("content-type", "application/json")
        .body(payload)
        .send()
        .map_err(|e| AppError::Network(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
        let detail = resp.text().unwrap_or_default();
        let detail = detail.lines().next().unwrap_or("").to_string();
        let mut detail = detail.chars().take(200).collect::<String>();
        if detail.is_empty() {
            return Err(AppError::Network(format!("AI service returned {status}")));
        }
        detail.insert_str(0, ": ");
        return Err(AppError::Network(format!("AI service returned {status}{detail}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_conn;
    use crate::dictionary::provider::{DictionaryProvider, SeedProvider};
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread::JoinHandle;

    // ── 测试用假 Chat Completions 服务（std-only，无新依赖）────

    fn spawn_server(status_line: &'static str, body: String, delay: Duration) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 8192];
                let _ = read_request(&mut stream, &mut buf);
                std::thread::sleep(delay);
                let head = format!("{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body.as_bytes());
                let _ = stream.flush();
            }
        });
        (format!("http://{addr}/v1"), handle)
    }

    /// 读完请求头（\r\n\r\n 为界）即可——假服务不解析请求体。
    fn read_request(stream: &mut TcpStream, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut filled = 0;
        loop {
            let n = stream.read(&mut buf[filled..])?;
            if n == 0 {
                return Ok(filled);
            }
            filled += n;
            if buf[..filled].windows(4).any(|w| w == b"\r\n\r\n") {
                return Ok(filled);
            }
        }
    }

    fn chat_body(content: &str) -> String {
        serde_json::json!({
            "choices": [{"message": {"role": "assistant", "content": content}}]
        })
        .to_string()
    }

    fn gen_words(conn: &Connection) -> Vec<GenWord> {
        SeedProvider.import(conn).unwrap();
        let mut out = Vec::new();
        // 种子导入后：word 1 = meticulous（sense 1），word 3 = subtle（sense 4）。
        for (wid, sid) in [(1i64, 1i64), (3, 4)] {
            out.push(load_gen_word(conn, wid, Some(sid)).unwrap().unwrap());
        }
        out
    }

    fn test_cfg(base_url: &str) -> AiConfig {
        AiConfig {
            enabled: true,
            base_url: base_url.to_string(),
            model: "mock-model".into(),
        }
    }

    // ── 配置 ─────────────────────────────────────────────────

    #[test]
    fn config_defaults_to_disabled_and_roundtrips() {
        let conn = test_conn();
        let cfg = load_config(&conn).unwrap();
        assert!(!cfg.enabled);
        assert_eq!(cfg.base_url, DEFAULT_AI_BASE_URL);
        assert_eq!(cfg.model, DEFAULT_AI_MODEL);

        save_config(&conn, true, "https://api.example.com/v1/", "test-model").unwrap();
        let cfg = load_config(&conn).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.base_url, "https://api.example.com/v1/");
        assert_eq!(cfg.model, "test-model");

        save_config(&conn, false, "https://api.example.com/v1/", "test-model").unwrap();
        assert!(!load_config(&conn).unwrap().enabled);
    }

    #[test]
    fn memory_key_store_roundtrip() {
        let store = MemoryKeyStore::new();
        assert_eq!(store.get().unwrap(), None);
        store.set("sk-test").unwrap();
        assert_eq!(store.get().unwrap().as_deref(), Some("sk-test"));
        store.delete().unwrap();
        assert_eq!(store.get().unwrap(), None);
        // 幂等删除
        store.delete().unwrap();
    }

    // ── 词条装配与缓存 ───────────────────────────────────────

    #[test]
    fn load_gen_word_respects_sense_and_falls_back() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        // 指定义项
        let w = load_gen_word(&conn, 1, Some(1)).unwrap().unwrap();
        assert_eq!(w.word, "meticulous");
        assert_eq!(w.sense_id, Some(1));
        assert!(w.english.contains("careful"));
        // 未指定 → 第一条非空英文释义的义项
        let w = load_gen_word(&conn, 1, None).unwrap().unwrap();
        assert_eq!(w.sense_id, Some(1));
        // 不存在的词 / 不属于该词的义项 → None
        assert!(load_gen_word(&conn, 99999, None).unwrap().is_none());
        assert!(load_gen_word(&conn, 1, Some(99999)).unwrap().is_none());
    }

    #[test]
    fn cache_roundtrip_and_key_sensitivity() {
        let conn = test_conn();
        SeedProvider.import(&conn).unwrap();
        let words = gen_words(&conn);
        let cfg = test_cfg("https://api.example.com/v1");
        assert!(cache_get(&conn, &words, &cfg).unwrap().is_empty());

        let examples = vec![AiExample {
            word_id: words[0].word_id,
            sense_id: words[0].sense_id,
            sentence: "The result was clearly ______ in the final data.".into(),
        }];
        cache_put(&conn, &words, &cfg, &examples).unwrap();

        let hits = cache_get(&conn, &words, &cfg).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].sentence, examples[0].sentence);

        // 缓存键对模型敏感：换模型不命中
        let other = AiConfig { model: "other-model".into(), ..cfg.clone() };
        assert!(cache_get(&conn, &words, &other).unwrap().is_empty());
        // 对服务地址敏感（尾部斜杠不影响）
        let slash = AiConfig { base_url: "https://api.example.com/v1/".into(), ..cfg.clone() };
        assert_eq!(cache_get(&conn, &words, &slash).unwrap().len(), 1);
        let other_url = AiConfig { base_url: "https://other.example.com/v1".into(), ..cfg };
        assert!(cache_get(&conn, &words, &other_url).unwrap().is_empty());
    }

    // ── 验证与挖空 ───────────────────────────────────────────

    #[test]
    fn validate_and_blank_accepts_only_valid_entries() {
        let conn = test_conn();
        let words = gen_words(&conn); // [meticulous(sense 1), subtle(sense 4)]
        let content = serde_json::json!([
            {"word_id": 1, "sense_id": 1, "sentence": "She was meticulous about checking every detail."},
            {"word_id": 1, "sense_id": 1, "sentence": "The report was meticulous and thorough."},   // 同词重复出现也有效
            {"word_id": 1, "sense_id": 1, "sentence": "It meticulousness quickly."},                 // 目标词原形未出现 → 拒绝
            {"word_id": 1, "sense_id": 1, "sentence": "   "},                                       // 空 → 拒绝
            {"word_id": 1, "sense_id": 1, "sentence": "Nothing relevant here."},                     // 不含目标词 → 拒绝
            {"word_id": 999, "sense_id": 1, "sentence": "Unknown id."},                              // ID 未请求 → 拒绝
            {"word_id": 3, "sense_id": 999, "sentence": "Wrong sense."},                             // 义项不一致 → 拒绝
            {"word_id": 3, "sense_id": 4, "sentence": "There was a subtle shift in the wind."}       // 保留
        ])
        .to_string();
        let out = validate_and_blank(&content, &words);
        assert_eq!(out.len(), 3, "{out:?}");
        assert_eq!(
            out[0].sentence,
            "She was ______ about checking every detail."
        );
        assert_eq!(out[2].sentence, "There was a ______ shift in the wind.");
        // 重复出现的目标词全部挖空
        let multi = serde_json::json!([
            {"word_id": 1, "sense_id": 1, "sentence": "She was meticulous, always meticulous, about details."}
        ])
        .to_string();
        let out = validate_and_blank(&multi, &words);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].sentence,
            "She was ______, always ______, about details."
        );
    }

    #[test]
    fn parse_json_items_strips_fences_and_prose() {
        let fenced = "```json\n[{\"word_id\":1}]\n```";
        assert_eq!(parse_json_items(fenced).len(), 1);
        let prose = "Here you go:\n[{\"word_id\":1},{\"word_id\":2}] hope it helps";
        assert_eq!(parse_json_items(prose).len(), 2);
        assert!(parse_json_items("no json at all").is_empty());
    }

    // ── 网络（假服务）─────────────────────────────────────────

    #[test]
    fn generate_batch_success_validates_and_returns() {
        let conn = test_conn();
        let words = gen_words(&conn);
        let content = serde_json::json!([
            {"word_id": 1, "sense_id": 1, "sentence": "She was meticulous about checking every detail."},
            {"word_id": 3, "sense_id": 4, "sentence": "There was a subtle shift in the wind."}
        ])
        .to_string();
        let (base, handle) = spawn_server("HTTP/1.1 200 OK", chat_body(&content), Duration::from_millis(0));
        let out = generate_batch(&words, &test_cfg(&base), "sk-test", REQUEST_TIMEOUT).unwrap();
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|e| e.sentence.contains("______")));
        handle.join().unwrap();
    }

    #[test]
    fn generate_batch_rejects_bad_status_and_bad_payload() {
        let conn = test_conn();
        let words = gen_words(&conn);

        // 401 → Err（错误密钥）
        let (base, h) = spawn_server("HTTP/1.1 401 Unauthorized", "{\"error\":\"bad key\"}".into(), Duration::from_millis(0));
        let err = generate_batch(&words, &test_cfg(&base), "sk-wrong", REQUEST_TIMEOUT).unwrap_err();
        assert!(err.to_string().contains("401"), "{err}");
        h.join().unwrap();

        // 200 但 choices 缺失 → Err
        let (base, h) = spawn_server("HTTP/1.1 200 OK", "{\"choices\":[]}".into(), Duration::from_millis(0));
        assert!(generate_batch(&words, &test_cfg(&base), "sk", REQUEST_TIMEOUT).is_err());
        h.join().unwrap();

        // 200 但 body 非 JSON → Err
        let (base, h) = spawn_server("HTTP/1.1 200 OK", "not json".into(), Duration::from_millis(0));
        assert!(generate_batch(&words, &test_cfg(&base), "sk", REQUEST_TIMEOUT).is_err());
        h.join().unwrap();

        // content 里没有可用的 JSON 数组 → Ok(空)＝全部回退本地
        let (base, h) = spawn_server("HTTP/1.1 200 OK", chat_body("I cannot help with that."), Duration::from_millis(0));
        let out = generate_batch(&words, &test_cfg(&base), "sk", REQUEST_TIMEOUT).unwrap();
        assert!(out.is_empty());
        h.join().unwrap();
    }

    #[test]
    fn generate_batch_times_out_without_retry() {
        let conn = test_conn();
        let words = gen_words(&conn);
        // 服务端延迟 1s > 客户端 200ms 超时 → Err
        let (base, h) = spawn_server(
            "HTTP/1.1 200 OK",
            chat_body("[]"),
            Duration::from_secs(1),
        );
        let started = std::time::Instant::now();
        assert!(generate_batch(&words, &test_cfg(&base), "sk", Duration::from_millis(200)).is_err());
        assert!(started.elapsed() < Duration::from_millis(900), "应在超时后立即返回，不重试");
        h.join().unwrap();
    }

    #[test]
    fn generate_batch_empty_words_skips_network() {
        let out = generate_batch(&[], &test_cfg("http://127.0.0.1:1"), "sk", REQUEST_TIMEOUT);
        assert!(out.unwrap().is_empty());
    }

    #[test]
    fn test_connection_reports_bad_status() {
        let cfg = AiConfig { enabled: true, base_url: "http://127.0.0.1:1/v1".into(), model: "m".into() };
        // 连接被拒（未监听端口）→ Err
        assert!(test_connection(&cfg, "sk").is_err());
        // 未配置模型 → Err
        let no_model = AiConfig { model: String::new(), ..cfg };
        assert!(test_connection(&no_model, "sk").is_err());
        // 未配置地址 → Err
        let no_url = AiConfig { base_url: String::new(), ..no_model };
        assert!(test_connection(&no_url, "sk").is_err());
    }
}
