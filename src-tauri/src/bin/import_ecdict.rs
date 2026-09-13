//! 无头 ECDICT 导入工具：与应用内导入命令共用 EcdictProvider。
//! 用法：import-ecdict <stardict.db> <lexica.db>
//! 目标库不存在时自动创建并迁移；已导入过则幂等跳过（0 新增）。

use std::path::PathBuf;
use std::time::Instant;

use lexica_lib::database::{self, migrate};
use lexica_lib::dictionary::provider::{DictionaryProvider, EcdictProvider};

fn main() {
    let mut args = std::env::args().skip(1);
    let (source, target) = match (args.next(), args.next()) {
        (Some(s), Some(t)) if args.next().is_none() => (s, t),
        _ => {
            eprintln!("usage: import-ecdict <stardict.db> <lexica.db>");
            std::process::exit(2);
        }
    };

    let target = PathBuf::from(target);
    if let Some(dir) = target.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).expect("create target dir");
        }
    }
    let conn = database::open(&target).expect("open target db");
    migrate::migrate(&conn).expect("run migrations");

    let provider = EcdictProvider {
        source_path: PathBuf::from(source),
    };
    let started = Instant::now();
    let imported = provider.import(&conn).expect("import ECDICT");
    println!(
        "[import-ecdict] imported {imported} words in {:.1}s",
        started.elapsed().as_secs_f32()
    );
}
