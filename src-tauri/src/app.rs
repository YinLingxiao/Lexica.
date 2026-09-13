use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::Manager;

use crate::database::{self, migrate};
use crate::dictionary::provider::{DictionaryProvider, SeedProvider};

/// 全程序唯一一处 `Arc<Mutex<...>>`：
/// `spawn_blocking` 闭包需要 'static 的连接句柄，而单用户桌面应用单连接足够
/// （WAL 模式下读写几乎不互斥）。其余任何地方不得再引入共享可变状态。
/// `db_path` 供词库导入命令开独立连接（导入不占这把锁）。
pub struct AppState {
    db: Arc<Mutex<Connection>>,
    pub db_path: PathBuf,
}

impl AppState {
    pub fn new(conn: Connection, db_path: PathBuf) -> Self {
        Self {
            db: Arc::new(Mutex::new(conn)),
            db_path,
        }
    }

    /// 克隆一份廉价句柄，供 `spawn_blocking` 闭包携带。
    pub fn db(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.db)
    }
}

/// 数据目录解析：默认 app_data_dir；若其中的 `db_dir.txt` 指向某目录则用之。
/// 词库全量导入后 DB 可达 1~2GB，C 盘紧张的机器把 db_dir.txt 内容写成
/// `D:\LexicaData` 即可（改动立即生效，无需重新编译；指针文件本身只有几十字节）。
fn resolve_data_dir(app: &tauri::App) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let default_dir = app.path().app_data_dir()?;
    let pointer = default_dir.join("db_dir.txt");
    if let Ok(text) = std::fs::read_to_string(&pointer) {
        let target = PathBuf::from(text.trim());
        if !target.as_os_str().is_empty() {
            std::fs::create_dir_all(&target)?;
            return Ok(target);
        }
    }
    Ok(default_dir)
}

/// 启动编排：打开数据库（不存在则创建）→ 迁移 → 幂等导入种子词库 → 注入状态。
pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let dir = resolve_data_dir(app)?;
    std::fs::create_dir_all(&dir)?;
    let db_path = dir.join("lexica.db");
    let conn = database::open(&db_path)?;
    migrate::migrate(&conn)?;
    let seeded = SeedProvider.import(&conn)?;
    if seeded > 0 {
        println!("[lexica] imported {seeded} seed words");
    }
    app.manage(AppState::new(conn, db_path));
    Ok(())
}
