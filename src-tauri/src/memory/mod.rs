pub mod model;
pub mod scheduler;
pub mod service;
pub mod store;

/// 记忆模块错误。memory 不依赖 dictionary（分层规则：只按 WordId 工作）。
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("encounter {0} not found")]
    EncounterNotFound(i64),
    #[error("encounter {0} is not the latest for its word")]
    NotLatestEncounter(i64),
    #[error("comprehension cannot change from {from} to {to}")]
    NotAnEscalation { from: String, to: String },
    #[error("corrupt memory event: {0}")]
    CorruptEvent(String),
}
