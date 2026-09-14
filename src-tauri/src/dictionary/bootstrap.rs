//! First-run dictionary bootstrap: download ECDICT SQLite, extract, import.
//! Oxford dictionaries cannot be redistributed; ECDICT (MIT) is the bundled free corpus.

use std::fs::{self, File};
use std::io::{copy, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::dictionary::provider::{DictionaryProvider, EcdictProvider};
use crate::error::AppError;

const READY_MARKER: &str = "ecdict.ok";
const ZIP_NAME: &str = "ecdict-sqlite-28.zip";
const DB_NAME: &str = "stardict.db";
const READY_MIN_ENTRIES: i64 = 1_000_000;

const DOWNLOAD_URLS: &[&str] = &[
    "https://github.com/skywind3000/ECDICT/releases/download/1.0.28/ecdict-sqlite-28.zip",
    "https://downloads.sourceforge.net/project/ecdict.mirror/1.0.28/ecdict-sqlite-28.zip",
];

static SETUP_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupPhase {
    Checking,
    Downloading,
    Extracting,
    Importing,
    Ready,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupProgress {
    pub phase: SetupPhase,
    /// 0.0–1.0 when known; omitted/None for indeterminate phases.
    pub progress: Option<f64>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct EnsureDictionaryResult {
    pub ready: bool,
    pub word_count: i64,
    pub imported: usize,
    pub skipped: bool,
}

fn emit(app: &AppHandle, phase: SetupPhase, progress: Option<f64>, message: impl Into<String>) {
    let _ = app.emit(
        "dictionary-setup",
        SetupProgress {
            phase,
            progress,
            message: message.into(),
        },
    );
}

fn data_dir(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn ecdict_count(conn: &Connection) -> Result<i64, AppError> {
    Ok(conn
        .query_row(
            "SELECT COUNT(*) FROM words WHERE source = 'ecdict'",
            [],
            |r| r.get(0),
        )
        .map_err(crate::database::DbError::from)?)
}

fn word_count(conn: &Connection) -> Result<i64, AppError> {
    Ok(conn
        .query_row("SELECT COUNT(*) FROM words", [], |r| r.get(0))
        .map_err(crate::database::DbError::from)?)
}

fn is_ready(conn: &Connection, dir: &Path) -> Result<bool, AppError> {
    if dir.join(READY_MARKER).is_file() {
        return Ok(true);
    }
    let n = ecdict_count(conn)?;
    if n >= READY_MIN_ENTRIES {
        let _ = fs::write(dir.join(READY_MARKER), b"1");
        return Ok(true);
    }
    Ok(false)
}

fn download_zip(app: &AppHandle, dest: &Path) -> Result<(), AppError> {
    if dest.is_file() && dest.metadata().map(|m| m.len() > 1_000_000).unwrap_or(false) {
        return Ok(());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .user_agent("Lexica/0.1 (dictionary bootstrap)")
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;

    let mut last_err = String::new();
    for url in DOWNLOAD_URLS {
        emit(
            app,
            SetupPhase::Downloading,
            Some(0.0),
            format!("Downloading dictionary…\n{url}"),
        );
        match download_one(&client, app, url, dest) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = e.to_string();
                let _ = fs::remove_file(dest);
            }
        }
    }
    Err(AppError::Network(format!(
        "dictionary download failed: {last_err}"
    )))
}

fn download_one(
    client: &reqwest::blocking::Client,
    app: &AppHandle,
    url: &str,
    dest: &Path,
) -> Result<(), AppError> {
    let mut response = client
        .get(url)
        .send()
        .map_err(|e| AppError::Network(e.to_string()))?
        .error_for_status()
        .map_err(|e| AppError::Network(e.to_string()))?;
    let total = response.content_length();
    let tmp = dest.with_extension("zip.part");
    let mut file = File::create(&tmp).map_err(|e| AppError::Io(e.to_string()))?;
    let mut buf = [0u8; 64 * 1024];
    let mut written: u64 = 0;
    let mut last_emit = 0u64;
    loop {
        let n = response
            .read(&mut buf)
            .map_err(|e| AppError::Network(e.to_string()))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| AppError::Io(e.to_string()))?;
        written += n as u64;
        if written - last_emit > 2 * 1024 * 1024 || total.map(|t| written >= t).unwrap_or(false) {
            last_emit = written;
            let progress = total.map(|t| written as f64 / t as f64);
            let mb = written as f64 / (1024.0 * 1024.0);
            let msg = match total {
                Some(t) => format!(
                    "Downloading dictionary… {:.0} / {:.0} MB",
                    mb,
                    t as f64 / (1024.0 * 1024.0)
                ),
                None => format!("Downloading dictionary… {mb:.0} MB"),
            };
            emit(app, SetupPhase::Downloading, progress, msg);
        }
    }
    file.flush().map_err(|e| AppError::Io(e.to_string()))?;
    drop(file);
    fs::rename(&tmp, dest).map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

fn extract_stardict(app: &AppHandle, zip_path: &Path, dest_db: &Path) -> Result<(), AppError> {
    if dest_db.is_file() && dest_db.metadata().map(|m| m.len() > 1_000_000).unwrap_or(false) {
        return Ok(());
    }
    emit(
        app,
        SetupPhase::Extracting,
        None,
        "Extracting dictionary…",
    );
    let file = File::open(zip_path).map_err(|e| AppError::Io(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| AppError::Io(e.to_string()))?;
    let mut found = false;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| AppError::Io(e.to_string()))?;
        let name = entry.name().replace('\\', "/");
        let base = Path::new(&name)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if !base.eq_ignore_ascii_case(DB_NAME) && !base.to_ascii_lowercase().ends_with(".db") {
            continue;
        }
        let tmp = dest_db.with_extension("db.part");
        let mut out = File::create(&tmp).map_err(|e| AppError::Io(e.to_string()))?;
        copy(&mut entry, &mut out).map_err(|e| AppError::Io(e.to_string()))?;
        drop(out);
        fs::rename(&tmp, dest_db).map_err(|e| AppError::Io(e.to_string()))?;
        found = true;
        break;
    }
    if !found {
        return Err(AppError::Io(
            "stardict.db not found inside the downloaded archive".into(),
        ));
    }
    Ok(())
}

fn import_ecdict(app: &AppHandle, db_path: &Path, source: &Path) -> Result<usize, AppError> {
    emit(
        app,
        SetupPhase::Importing,
        None,
        "Importing entries — this can take several minutes…",
    );
    let provider = EcdictProvider {
        source_path: source.to_path_buf(),
    };
    let conn = crate::database::open(db_path)?;
    let imported = provider.import(&conn)?;
    Ok(imported)
}

/// Ensure the full ECDICT corpus is present locally; download + import when missing.
pub fn ensure(app: &AppHandle, db_path: &Path) -> Result<EnsureDictionaryResult, AppError> {
    let _guard = SETUP_LOCK
        .lock()
        .map_err(|_| AppError::LockPoisoned)?;
    let dir = data_dir(db_path);
    fs::create_dir_all(&dir).map_err(|e| AppError::Io(e.to_string()))?;

    emit(app, SetupPhase::Checking, None, "Checking dictionary…");
    {
        let conn = crate::database::open(db_path)?;
        if is_ready(&conn, &dir)? {
            let count = word_count(&conn)?;
            emit(app, SetupPhase::Ready, Some(1.0), "Dictionary ready");
            return Ok(EnsureDictionaryResult {
                ready: true,
                word_count: count,
                imported: 0,
                skipped: true,
            });
        }
    }

    let zip_path = dir.join(ZIP_NAME);
    let stardict = dir.join(DB_NAME);

    download_zip(app, &zip_path)?;
    extract_stardict(app, &zip_path, &stardict)?;
    let imported = import_ecdict(app, db_path, &stardict)?;

    let conn = crate::database::open(db_path)?;
    let count = word_count(&conn)?;
    if ecdict_count(&conn)? >= READY_MIN_ENTRIES {
        let _ = fs::write(dir.join(READY_MARKER), b"1");
    }
    // Keep stardict.db for resumable re-import; drop the zip to reclaim ~200 MB.
    let _ = fs::remove_file(&zip_path);

    emit(
        app,
        SetupPhase::Ready,
        Some(1.0),
        format!("Dictionary ready · {count} entries"),
    );
    Ok(EnsureDictionaryResult {
        ready: true,
        word_count: count,
        imported,
        skipped: false,
    })
}
