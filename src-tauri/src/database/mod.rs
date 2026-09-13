pub mod migrate;

use std::path::Path;

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::Connection;
use thiserror::Error;

/// 统一时间戳存储格式：UTC、RFC3339、毫秒定宽（字典序 = 时间序）。
pub fn fmt_ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// 打开磁盘数据库：WAL（读写几乎不互斥）+ 外键强制 + busy_timeout
/// （词库批量导入跑在独立连接上，应用侧写入遇导入事务时短暂等待而非立刻报错）。
pub fn open(path: &Path) -> Result<Connection, DbError> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

/// 测试用内存库（:memory: 上 WAL 无意义，跳过）。
#[cfg(test)]
pub(crate) fn open_in_memory() -> Result<Connection, DbError> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

/// 已迁移到最新版本的测试库。
#[cfg(test)]
pub(crate) fn test_conn() -> Connection {
    let conn = open_in_memory().expect("open in-memory db");
    migrate::migrate(&conn).expect("run migrations");
    conn
}
