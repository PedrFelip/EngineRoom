mod db;
mod engine;
mod system;

use engine::EngineState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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
        .invoke_handler(tauri::generate_handler![
            db::cache_get,
            db::cache_put,
            db::cache_clear,
            db::games_save,
            db::games_list,
            db::games_get,
            db::games_delete,
            db::games_clear,
            db::storage_stats,
            engine::engine_spawn,
            engine::engine_send,
            engine::engine_stop,
            system::system_resources,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
