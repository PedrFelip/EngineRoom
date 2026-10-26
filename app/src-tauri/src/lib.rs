mod db;
mod engine;
mod imports;
mod review;
mod system;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let benchmark_mode = std::env::args().any(|arg| arg == "--bench-uci-ipc");
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            use tauri::Manager;
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open_file(&dir.join("engineroom.db"))?;
            app.manage(db::DbState(std::sync::Mutex::new(conn)));
            app.manage(imports::state()?);
            Ok(())
        })
        .manage(review::ReviewSessions::default())
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                use tauri::Manager;
                window
                    .state::<review::ReviewSessions>()
                    .close_window(window.label());
                window
                    .state::<imports::ImportTasks>()
                    .close_window(window.label());
            }
        })
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
            review::review_session_open,
            review::review_session_analyze_position,
            review::review_session_cancel_live,
            review::review_session_close,
            review::engine_probe,
            review::repository::games_get_review_config,
            engine::benchmark_uci_enabled,
            engine::benchmark_uci_send,
            engine::benchmark_uci_send_batch,
            engine::benchmark_uci_report,
            system::system_resources,
            imports::chess_com_search_start,
            imports::chess_com_search_cancel,
        ])
        .build(tauri::generate_context!());
    let app = match app {
        Ok(app) => app,
        Err(error) => {
            eprintln!("Failed to initialize EngineRoom: {error}");
            std::process::exit(1);
        }
    };
    let shutdown_complete = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    app.run(move |app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            use std::sync::atomic::Ordering;
            use tauri::Manager;
            if shutdown_complete.load(Ordering::SeqCst) {
                return;
            }
            api.prevent_exit();
            let cleanup = match app.state::<review::ReviewSessions>().begin_shutdown() {
                review::ShutdownAction::Cleanup(cleanup) => Some(cleanup),
                review::ShutdownAction::Wait | review::ShutdownAction::Exit => None,
            };
            let imports_cleanup = app.state::<imports::ImportTasks>().begin_shutdown();
            if cleanup.is_some() || imports_cleanup.is_some() {
                let app = app.clone();
                let shutdown_complete = shutdown_complete.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(cleanup) = cleanup {
                        cleanup.await;
                    }
                    if let Some(cleanup) = imports_cleanup {
                        cleanup.await;
                    }
                    shutdown_complete.store(true, Ordering::SeqCst);
                    app.exit(0);
                });
            }
        }
    });
}
