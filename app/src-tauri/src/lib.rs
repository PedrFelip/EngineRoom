mod db;
mod engine;
mod system;

use engine::EngineState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let benchmark_mode = std::env::args().any(|arg| arg == "--bench-uci-ipc");
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            use tauri::Manager;
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open_file(&dir.join("engineroom.db"))?;
            app.manage(db::DbState(std::sync::Mutex::new(conn)));
            Ok(())
        })
        .manage(EngineState::default())
        .manage(engine::BenchmarkUciState::default())
        .manage(engine::BenchmarkMode(benchmark_mode))
        .invoke_handler(tauri::generate_handler![
            db::cache::cache_get,
            db::cache::cache_put,
            db::cache::cache_get_bulk,
            db::cache::cache_put_many,
            db::cache::cache_clear,
            db::games::games_save,
            db::games::games_list,
            db::games::games_get,
            db::games::games_delete,
            db::games::games_clear,
            db::stats::storage_stats,
            engine::engine_spawn,
            engine::engine_send,
            engine::engine_send_batch,
            engine::engine_stop,
            engine::benchmark_uci_enabled,
            engine::benchmark_uci_send,
            engine::benchmark_uci_send_batch,
            engine::benchmark_uci_report,
            system::system_resources,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
