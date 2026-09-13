pub mod grader;
pub mod model;
pub mod service;

/// 复习模块错误。review 向下依赖 dictionary（读词条）与 memory（写结果）——符合分层。
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("dictionary error: {0}")]
    Dictionary(#[from] crate::dictionary::DictionaryError),
    #[error("memory error: {0}")]
    Memory(#[from] crate::memory::MemoryError),
    #[error("review {0} not found")]
    NotFound(i64),
    #[error("review {0} already completed")]
    AlreadyCompleted(i64),
    #[error("invalid hint number {0}")]
    InvalidHint(u32),
    #[error("expected hint {expected}, got {requested}")]
    UnexpectedHint { requested: u32, expected: u32 },
    #[error("word data unavailable for review")]
    NoWordData,
}
