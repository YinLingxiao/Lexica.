pub mod bootstrap;
pub mod engine;
pub mod model;
pub mod provider;
pub mod store;

/// 词典模块自己的错误类型。不依赖 memory/review（分层规则）。
#[derive(Debug, thiserror::Error)]
pub enum DictionaryError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("dictionary data error: {0}")]
    Data(#[from] serde_json::Error),
    #[error("invalid dictionary source: {0}")]
    InvalidSource(String),
}
