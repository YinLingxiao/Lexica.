//! IPC 薄层：组合各模块服务，不写业务规则。跨模块编排只发生在这里。

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use tauri::{Emitter, State};

use crate::app::AppState;
use crate::dictionary::model::{LookupOutcome, Suggestion};
use crate::dictionary::provider::{DictionaryProvider, EcdictProvider};
use crate::domain::{ComprehensionLevel, EncounterId, ExampleId, ReviewId, SenseId, WordId};
use crate::error::AppError;
use crate::memory::model::MemoryStatus;
use crate::memory::service::MemoryService;
use crate::memory::service::StatsSummary;
use crate::review::model::{Hint, ReviewFeedback, ReviewItem};
use crate::review::service::ReviewService;

/// 所有 command 统一入口：把 DB 工作放到阻塞线程池执行，
/// 不占用主线程，也避免直接依赖 tokio（复用 Tauri 自带 runtime）。
async fn with_db<T, F>(state: &AppState, f: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce(&Connection) -> Result<T, AppError> + Send + 'static,
{
    let db = state.db();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db.lock().map_err(|_| AppError::LockPoisoned)?;
        f(&conn)
    })
    .await
    .map_err(|e| AppError::TaskJoin(e.to_string()))?
}

#[tauri::command]
pub async fn word_note(
    state: State<'_, AppState>,
    word_id: i64,
) -> Result<crate::library::WordNote, AppError> {
    with_db(&state, move |conn| {
        Ok(crate::library::word_note(conn, word_id)?)
    })
    .await
}

#[tauri::command]
pub async fn save_word_note(
    state: State<'_, AppState>,
    word_id: i64,
    bookmarked: bool,
    note: String,
) -> Result<(), AppError> {
    with_db(&state, move |conn| {
        Ok(crate::library::save_note(conn, word_id, bookmarked, &note)?)
    })
    .await
}

#[tauri::command]
pub async fn library_words(
    state: State<'_, AppState>,
    query: String,
    filter: String,
    offset: u32,
) -> Result<crate::library::LibraryPage, AppError> {
    with_db(&state, move |conn| {
        Ok(crate::library::list(conn, &query, &filter, offset)?)
    })
    .await
}

#[tauri::command]
pub async fn activity_days(
    state: State<'_, AppState>,
    offset_minutes: i32,
) -> Result<Vec<crate::library::ActivityDay>, AppError> {
    with_db(&state, move |conn| {
        Ok(crate::library::activity(conn, Utc::now(), offset_minutes)?)
    })
    .await
}

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub schema_version: i64,
    pub word_count: i64,
    pub fts_ok: bool,
    pub dictionary_ready: bool,
}

/// 冒烟命令：证明 IPC → Rust → SQLite（含 FTS5 虚拟表）全链路可用。
#[tauri::command]
pub async fn app_info(state: State<'_, AppState>) -> Result<AppInfo, AppError> {
    let db_path = state.db_path.clone();
    with_db(&state, move |conn| {
        let schema_version: i64 = conn
            .query_row("SELECT user_version FROM pragma_user_version", [], |r| {
                r.get(0)
            })
            .map_err(crate::database::DbError::from)?;
        let word_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM words", [], |r| r.get(0))
            .map_err(crate::database::DbError::from)?;
        let fts_ok = conn
            .query_row("SELECT rowid FROM words_fts LIMIT 1", [], |r| {
                r.get::<_, i64>(0)
            })
            .optional()
            .is_ok();
        let dir = db_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let dictionary_ready = dir.join("ecdict.ok").is_file()
            || conn
                .query_row(
                    "SELECT COUNT(*) FROM words WHERE source = 'ecdict'",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .map(|n| n >= 1_000_000)
                .unwrap_or(false);
        Ok(AppInfo {
            schema_version,
            word_count,
            fts_ok,
            dictionary_ready,
        })
    })
    .await
}

/// Download ECDICT when missing, then import into the local database.
#[tauri::command]
pub async fn ensure_dictionary(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::dictionary::bootstrap::EnsureDictionaryResult, AppError> {
    let db_path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::dictionary::bootstrap::ensure(&app, &db_path).inspect_err(|e| {
            let _ = app.emit(
                "dictionary-setup",
                crate::dictionary::bootstrap::SetupProgress {
                    phase: crate::dictionary::bootstrap::SetupPhase::Error,
                    progress: None,
                    message: e.to_string(),
                },
            );
        })
    })
    .await
    .map_err(|e| AppError::TaskJoin(e.to_string()))?
}

/// 导入 ECDICT 全量词库（MIT，stardict.db）。
/// 刻意开独立数据库连接而不走 AppState：340 万词条导入以分钟计，
/// 不能占住全局互斥锁把查词/复习一起卡死。WAL 下批量写与应用侧
/// 读写并行，应用连接遇导入事务最多等待 busy_timeout。
#[tauri::command]
pub async fn import_ecdict(
    state: State<'_, AppState>,
    source_path: String,
) -> Result<usize, AppError> {
    let db_path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<usize, AppError> {
        let provider = EcdictProvider {
            source_path: PathBuf::from(&source_path),
        };
        let conn = crate::database::open(&db_path)?;
        let imported = provider.import(&conn)?;
        let ecdict: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM words WHERE source = 'ecdict'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if ecdict >= 1_000_000 {
            if let Some(dir) = db_path.parent() {
                let _ = std::fs::write(dir.join("ecdict.ok"), b"1");
            }
        }
        Ok(imported)
    })
    .await
    .map_err(|e| AppError::TaskJoin(e.to_string()))?
}

/// 输入过程中的搜索建议（防抖后调用）。
#[tauri::command]
pub async fn search_suggest(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u8>,
) -> Result<Vec<Suggestion>, AppError> {
    with_db(&state, move |conn| {
        let limit = limit.unwrap_or(8);
        crate::dictionary::engine::suggest(conn, &query, limit).map_err(AppError::from)
    })
    .await
}

/// 查词结果附带的记忆摘要（安静的一行提示，不打扰阅读）。
#[derive(Debug, Serialize)]
pub struct MemorySummary {
    /// None = 新词（尚无记忆状态）
    pub status: Option<MemoryStatus>,
    pub visit_count: u32,
    pub high_priority: bool,
}

#[derive(Debug, Serialize)]
pub struct LookupWordResult {
    pub outcome: LookupOutcome,
    /// 命中时自动开启的 visit（查词即 Encounter，无需用户加入生词本）。
    pub encounter_id: Option<EncounterId>,
    pub memory: Option<MemorySummary>,
    pub comprehension_level: Option<ComprehensionLevel>,
}

/// 查词：词典命中 + 自动记录 Encounter + 重折记忆。
#[tauri::command]
pub async fn lookup_word(
    state: State<'_, AppState>,
    word: String,
) -> Result<LookupWordResult, AppError> {
    let now = Utc::now();
    with_db(&state, move |conn| {
        let outcome = crate::dictionary::engine::lookup(conn, &word)?;
        let memory = MemoryService::default();
        let (encounter_id, summary) = match &outcome {
            LookupOutcome::Hit { entry } => {
                let id = memory.open_visit(conn, entry.id, now)?;
                let state = memory.load_state(conn, entry.id)?;
                let summary = state.map(|s| MemorySummary {
                    status: Some(s.status()),
                    visit_count: s.visit_count,
                    high_priority: s.high_priority,
                });
                (Some(id), summary)
            }
            LookupOutcome::Miss { .. } => (None, None),
        };
        let comprehension_level = encounter_id
            .map(|id| crate::memory::store::get(conn, id))
            .transpose()?
            .flatten()
            .map(|encounter| encounter.comprehension_level);
        Ok(LookupWordResult {
            outcome,
            encounter_id,
            memory: summary,
            comprehension_level,
        })
    })
    .await
}

/// 披露动作上报：用户实际查看到哪一级，理解级别就是哪一级。
#[tauri::command]
pub async fn set_comprehension(
    state: State<'_, AppState>,
    encounter_id: EncounterId,
    level: ComprehensionLevel,
) -> Result<(), AppError> {
    let now = Utc::now();
    with_db(&state, move |conn| {
        MemoryService::default()
            .set_comprehension(conn, encounter_id, level, now)
            .map_err(AppError::from)
    })
    .await
}

/// Recent 列表 DTO：memory 的访问记录 + dictionary 的词头（组合层产物）。
#[derive(Debug, Serialize)]
pub struct RecentWord {
    pub word: String,
    pub display: String,
    pub phonetic: Option<String>,
    pub visited_at: DateTime<Utc>,
    pub comprehension_level: ComprehensionLevel,
}

/// Recent 列表：memory 提供访问记录，dictionary 补齐词头（模块解耦，commands 组合）。
#[tauri::command]
pub async fn recent_words(
    state: State<'_, AppState>,
    limit: Option<u8>,
) -> Result<Vec<RecentWord>, AppError> {
    with_db(&state, move |conn| {
        let limit = limit.unwrap_or(6);
        let visits = MemoryService::default().recent_visits(conn, limit)?;
        let ids: Vec<_> = visits.iter().map(|v| v.word_id).collect();
        let briefs = crate::dictionary::engine::word_briefs(conn, &ids)?;
        Ok(visits
            .iter()
            .filter_map(|v| {
                briefs.get(&v.word_id).map(|b| RecentWord {
                    word: b.word.clone(),
                    display: b.display.clone(),
                    phonetic: b.phonetic.clone(),
                    visited_at: v.visited_at,
                    comprehension_level: v.comprehension_level,
                })
            })
            .collect())
    })
    .await
}

/// 首页与统计页总览（Encountered / Familiar / Stable / due…）。
#[tauri::command]
pub async fn stats_summary(state: State<'_, AppState>) -> Result<StatsSummary, AppError> {
    let now = Utc::now();
    with_db(&state, move |conn| {
        MemoryService::default()
            .stats_summary(conn, now)
            .map_err(AppError::from)
    })
    .await
}

/// 忽略 / 取消忽略一个词（唯一的“别再提醒我”出口）。
#[tauri::command]
pub async fn set_word_ignored(
    state: State<'_, AppState>,
    word_id: WordId,
    ignored: bool,
) -> Result<(), AppError> {
    with_db(&state, move |conn| {
        MemoryService::default()
            .set_ignored(conn, word_id, ignored)
            .map_err(AppError::from)
    })
    .await
}

/// 开发用：全量重建记忆投影（换算法后调用）。
#[tauri::command]
pub async fn rebuild_memory_projection(state: State<'_, AppState>) -> Result<usize, AppError> {
    let now = Utc::now();
    with_db(&state, move |conn| {
        MemoryService::default()
            .rebuild_projection(conn, now)
            .map_err(AppError::from)
    })
    .await
}

// ── 复习 ──────────────────────────────────────────────────

/// 复习队列（旧接口，保留兼容）：题目条目，不含答案。
#[tauri::command]
pub async fn review_queue(state: State<'_, AppState>) -> Result<Vec<ReviewItem>, AppError> {
    let now = Utc::now();
    with_db(&state, move |conn| {
        ReviewService.queue(conn, now).map_err(AppError::from)
    })
    .await
}

/// 只读复习组：到期词（≤10）的完整词条 + 本地题目。
/// 浏览阶段零副作用：不写 encounters、不创建 pending review、不动记忆投影。
#[tauri::command]
pub async fn review_group(
    state: State<'_, AppState>,
) -> Result<Vec<crate::review::service::ReviewGroupEntry>, AppError> {
    let now = Utc::now();
    with_db(&state, move |conn| {
        ReviewService.group(conn, now).map_err(AppError::from)
    })
    .await
}

/// 组内重考：只读判分（复用 grader 规则），不重复计入正式复习。
#[tauri::command]
pub async fn practice_check(
    state: State<'_, AppState>,
    word_id: WordId,
    answer: Option<String>,
) -> Result<bool, AppError> {
    with_db(&state, move |conn| {
        ReviewService
            .practice_check(conn, word_id, answer)
            .map_err(AppError::from)
    })
    .await
}

/// 组内重考：只读提示（与正式复习同一构造逻辑），不落任何记录。
#[tauri::command]
pub async fn practice_hint(
    state: State<'_, AppState>,
    word_id: WordId,
    sense_id: Option<SenseId>,
    hint_no: u32,
) -> Result<Hint, AppError> {
    with_db(&state, move |conn| {
        ReviewService
            .practice_hint(conn, word_id, sense_id, hint_no)
            .map_err(AppError::from)
    })
    .await
}

/// 开始一题，返回 review_id。
#[tauri::command]
pub async fn start_review_item(
    state: State<'_, AppState>,
    word_id: WordId,
    sense_id: Option<SenseId>,
    example_id: Option<ExampleId>,
) -> Result<ReviewId, AppError> {
    let now = Utc::now();
    with_db(&state, move |conn| {
        ReviewService
            .start(conn, word_id, sense_id, example_id, now)
            .map_err(AppError::from)
    })
    .await
}

/// 请求第 hint_no 级提示（1..=3）。
#[tauri::command]
pub async fn request_hint(
    state: State<'_, AppState>,
    review_id: ReviewId,
    hint_no: u32,
) -> Result<Hint, AppError> {
    let now = Utc::now();
    with_db(&state, move |conn| {
        ReviewService
            .hint(conn, review_id, hint_no, now)
            .map_err(AppError::from)
    })
    .await
}

/// 提交答案（None = 放弃/揭示）→ 判分 + 记忆更新。
#[tauri::command]
pub async fn submit_review(
    state: State<'_, AppState>,
    review_id: ReviewId,
    answer: Option<String>,
) -> Result<ReviewFeedback, AppError> {
    let now = Utc::now();
    with_db(&state, move |conn| {
        ReviewService
            .submit(conn, review_id, answer, now)
            .map_err(AppError::from)
    })
    .await
}

// ── AI 例句（可选，默认关闭）────────────────────────────────

/// 读取 AI 配置视图（has_key 布尔，绝不回传密钥本身）。
#[tauri::command]
pub async fn ai_config_get(state: State<'_, AppState>) -> Result<crate::ai::AiConfigView, AppError> {
    with_db(&state, move |conn| {
        let cfg = crate::ai::load_config(conn).map_err(AppError::from)?;
        let has_key = crate::ai::key_store().get()?.is_some();
        Ok(crate::ai::AiConfigView {
            enabled: cfg.enabled,
            base_url: cfg.base_url,
            model: cfg.model,
            has_key,
        })
    })
    .await
}

/// 保存 AI 配置。key 语义：None = 保留现有密钥；Some("") = 删除；Some(k) = 更换。
/// 密钥写入 Windows 凭据管理器失败时直接报错——绝不降级为明文存储。
#[tauri::command]
pub async fn ai_config_save(
    state: State<'_, AppState>,
    enabled: bool,
    base_url: String,
    model: String,
    key: Option<String>,
) -> Result<crate::ai::AiConfigView, AppError> {
    if let Some(key) = &key {
        let store = crate::ai::key_store();
        if key.is_empty() {
            store.delete()?;
        } else {
            store.set(key)?;
        }
    }
    let base_url = base_url.trim().to_string();
    let model = model.trim().to_string();
    with_db(&state, move |conn| {
        crate::ai::save_config(conn, enabled, &base_url, &model).map_err(AppError::from)
    })
    .await?;
    ai_config_get(state).await
}

/// 连接测试：一次最小 Chat Completions 请求验证地址/模型/密钥。
/// 配置先短锁取出，HTTP 在阻塞线程池执行——等待网络期间不持有数据库锁。
#[tauri::command]
pub async fn ai_test_connection(state: State<'_, AppState>) -> Result<(), AppError> {
    let cfg = with_db(&state, |conn| {
        crate::ai::load_config(conn).map_err(AppError::from)
    })
    .await?;
    let key = crate::ai::key_store()
        .get()?
        .ok_or_else(|| AppError::Network("No API key stored for AI".into()))?;
    tauri::async_runtime::spawn_blocking(move || crate::ai::test_connection(&cfg, &key))
        .await
        .map_err(|e| AppError::TaskJoin(e.to_string()))?
}

#[derive(Debug, serde::Deserialize)]
pub struct AiGenItem {
    pub word_id: i64,
    pub sense_id: Option<i64>,
}

/// 例句生成（浏览期间后台调用，每组 ≤10 词，一批一个请求、15s 超时、不重试）。
/// 流程：短锁（配置 + 装配词条 + 查缓存）→ 无锁网络 → 短锁写缓存。
/// 仅处理 `enabled` 且有效（sense 属于该词）的条目；已缓存的直接返回。
#[tauri::command]
pub async fn ai_generate_examples(
    state: State<'_, AppState>,
    items: Vec<AiGenItem>,
) -> Result<Vec<crate::ai::AiExample>, AppError> {
    if items.is_empty() {
        return Ok(vec![]);
    }
    let (cfg, words, cached) = with_db(&state, move |conn| {
        let cfg = crate::ai::load_config(conn).map_err(AppError::from)?;
        if !cfg.enabled {
            return Ok((cfg, Vec::new(), Vec::new()));
        }
        let mut words = Vec::new();
        for item in &items {
            if let Some(w) =
                crate::ai::load_gen_word(conn, item.word_id, item.sense_id).map_err(AppError::from)?
            {
                words.push(w);
            }
        }
        let cached = crate::ai::cache_get(conn, &words, &cfg).map_err(AppError::from)?;
        Ok((cfg, words, cached))
    })
    .await?;
    if !cfg.enabled || words.is_empty() {
        return Ok(cached);
    }
    let missing: Vec<crate::ai::GenWord> = words
        .iter()
        .filter(|w| !cached.iter().any(|c| c.word_id == w.word_id))
        .cloned()
        .collect();
    if missing.is_empty() {
        return Ok(cached);
    }
    // 网络等待不持有数据库锁：密钥与配置已在锁外就绪。
    let key = match crate::ai::key_store().get() {
        Ok(Some(key)) => key,
        Ok(None) if !cached.is_empty() => return Ok(cached),
        Ok(None) => return Err(AppError::Network("No API key stored for AI".into())),
        Err(_) if !cached.is_empty() => return Ok(cached),
        Err(error) => return Err(error),
    };
    let http_cfg = cfg.clone();
    let http_words = missing.clone();
    let generated = tauri::async_runtime::spawn_blocking(move || {
        crate::ai::generate_batch(&http_words, &http_cfg, &key, crate::ai::REQUEST_TIMEOUT)
    })
    .await
    .map_err(|e| AppError::TaskJoin(e.to_string()))?;
    let generated = match generated {
        Ok(generated) => generated,
        Err(_) if !cached.is_empty() => return Ok(cached),
        Err(error) => return Err(error),
    };
    let cache_rows = generated.clone();
    with_db(&state, move |conn| {
        crate::ai::cache_put(conn, &missing, &cfg, &cache_rows).map_err(AppError::from)
    })
    .await?;
    Ok(cached.into_iter().chain(generated).collect())
}

/// 统计页的 fading 列表：memory 到期信息 + dictionary 词头。
#[derive(Debug, Serialize)]
pub struct FadingWord {
    pub word: String,
    pub display: String,
    pub high_priority: bool,
    pub next_review_at: Option<DateTime<Utc>>,
}

#[tauri::command]
pub async fn fading_words(
    state: State<'_, AppState>,
    limit: Option<u8>,
) -> Result<Vec<FadingWord>, AppError> {
    let now = Utc::now();
    with_db(&state, move |conn| {
        let limit = limit.unwrap_or(20) as usize;
        let due = MemoryService::default().due_words(conn, now)?;
        let ids: Vec<_> = due.iter().map(|d| d.word_id).collect();
        let briefs = crate::dictionary::engine::word_briefs(conn, &ids)?;
        Ok(due
            .iter()
            .filter_map(|d| {
                briefs.get(&d.word_id).map(|b| FadingWord {
                    word: b.word.clone(),
                    display: b.display.clone(),
                    high_priority: d.high_priority,
                    next_review_at: d.next_review_at,
                })
            })
            .take(limit)
            .collect())
    })
    .await
}
