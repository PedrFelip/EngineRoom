mod engine;
mod system;

use engine::EngineState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(EngineState::default())
        .invoke_handler(tauri::generate_handler![
            engine::engine_spawn,
            engine::engine_send,
            engine::engine_stop,
            system::system_resources,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
