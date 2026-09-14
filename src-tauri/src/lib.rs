pub mod app;
pub mod commands;
pub mod database;
pub mod dictionary;
pub mod domain;
pub mod error;
pub mod library;
pub mod memory;
pub mod review;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|builder| {
            app::setup(builder)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::import_ecdict,
            commands::ensure_dictionary,
            commands::search_suggest,
            commands::lookup_word,
            commands::set_comprehension,
            commands::recent_words,
            commands::stats_summary,
            commands::set_word_ignored,
            commands::rebuild_memory_projection,
            commands::review_queue,
            commands::start_review_item,
            commands::request_hint,
            commands::submit_review,
            commands::fading_words,
            commands::word_note,
            commands::save_word_note,
            commands::library_words,
            commands::activity_days
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
