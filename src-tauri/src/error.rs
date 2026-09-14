use serde::Serialize;

use crate::dictionary::DictionaryError;
use crate::memory::MemoryError;
use crate::review::ReviewError;

/// 所有 IPC command 的统一错误。
/// 序列化为 `{ code, message }`，前端安静地内联展示 message，不弹窗。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] crate::database::DbError),

    #[error("dictionary error: {0}")]
    Dictionary(#[from] DictionaryError),

    #[error("memory error: {0}")]
    Memory(#[from] MemoryError),

    #[error("review error: {0}")]
    Review(#[from] ReviewError),

    #[error("the database lock was poisoned")]
    LockPoisoned,

    #[error("background task failed: {0}")]
    TaskJoin(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("I/O error: {0}")]
    Io(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", &error_code(self))?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

fn error_code(e: &AppError) -> &'static str {
    match e {
        AppError::Db(_) => "db",
        AppError::Dictionary(_) => "dictionary",
        AppError::Memory(_) => "memory",
        AppError::Review(_) => "review",
        AppError::LockPoisoned => "lock_poisoned",
        AppError::TaskJoin(_) => "task_join",
        AppError::Network(_) => "network",
        AppError::Io(_) => "io",
    }
}
