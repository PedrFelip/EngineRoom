mod db;
mod engine;
mod imports;
mod review;
mod system;
mod updates;

#[derive(Default)]
struct Shutdown(tokio::sync::OnceCell<()>);

async fn shutdown(app: &tauri::AppHandle) {
    use tauri::Manager;
    app.state::<Shutdown>()
        .0
        .get_or_init(|| async {
            let reviews = app.state::<review::ReviewSessions>().begin_shutdown();
            let imports = app.state::<imports::ImportTasks>().begin_shutdown();
            if let review::ShutdownAction::Cleanup(cleanup) = reviews {
                cleanup.await;
            }
            if let Some(cleanup) = imports {
                cleanup.await;
            }
        })
        .await;
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let benchmark_mode = std::env::args().any(|arg| arg == "--bench-uci-ipc");
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::Updates::default())
        .manage(Shutdown::default())
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
            updates::app_update_check,
            updates::app_update_install,
            updates::app_update_open_release_page,
            updates::app_update_restart,
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
    app.run(move |app, event| {
        if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
            use tauri::Manager;
            let installation_finished = app.state::<updates::Updates>().prepare_exit();
            if app.state::<Shutdown>().0.get().is_some() && installation_finished {
                return;
            }
            api.prevent_exit();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                app.state::<updates::Updates>().drain_installation().await;
                shutdown(&app).await;
                app.exit(code.unwrap_or(0));
            });
        }
    });
}
