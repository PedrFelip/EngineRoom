mod installation;

use installation::InstallationGate;
use serde::Serialize;
use std::time::Duration;
use tauri::ipc::Channel;
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::{Mutex, OnceCell};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current_version: String,
    available_version: Option<String>,
    notes: Option<String>,
    supported: bool,
}

#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum UpdateProgress {
    Downloading { downloaded: u64, total: Option<u64> },
    Installing,
    Restarting,
    RestartRequired,
}

#[derive(Default)]
pub struct Updates {
    checked: OnceCell<Result<UpdateInfo, String>>,
    pending: Mutex<Option<Update>>,
    installation: InstallationGate,
}

impl Updates {
    /// Reject new installations and report whether the critical phase has finished.
    pub fn prepare_exit(&self) -> bool {
        self.installation.close();
        self.installation.is_idle()
    }

    /// Wait for file replacement even when the command awaiting it was dropped.
    pub async fn drain_installation(&self) {
        self.installation.drain().await;
    }
}

/// Cache the startup result, including failures, so navigation never checks again.
#[tauri::command]
pub async fn app_update_check(
    app: tauri::AppHandle,
    state: tauri::State<'_, Updates>,
) -> Result<UpdateInfo, String> {
    state
        .checked
        .get_or_init(|| async {
            let mut info = UpdateInfo {
                current_version: app.package_info().version.to_string(),
                available_version: None,
                notes: None,
                supported: !cfg!(debug_assertions),
            };
            if !info.supported {
                return Ok(info);
            }
            let cleanup_app = app.clone();
            let update = app
                .updater_builder()
                .timeout(Duration::from_secs(20))
                .on_before_exit(move || {
                    // Windows exits inside install(), bypassing the Tauri exit event.
                    // Only drain resources here: this thread still owns the installation guard.
                    tauri::async_runtime::block_on(crate::shutdown(&cleanup_app));
                })
                .build()
                .map_err(|e| e.to_string())?
                .check()
                .await
                .map_err(|e| e.to_string())?;
            if let Some(mut update) = update {
                update.timeout = Some(Duration::from_secs(15 * 60));
                info.available_version = Some(update.version.clone());
                info.notes = update.body.clone();
                *state.pending.lock().await = Some(update);
            }
            Ok(info)
        })
        .await
        .clone()
}

#[tauri::command]
pub async fn app_update_install(
    app: tauri::AppHandle,
    state: tauri::State<'_, Updates>,
    on_progress: Channel<UpdateProgress>,
) -> Result<(), String> {
    let pending = state
        .pending
        .try_lock()
        .map_err(|_| "Uma atualização já está em andamento.".to_string())?;
    let update = pending.as_ref().ok_or("Nenhuma atualização disponível.")?;
    let mut downloaded = 0;
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = on_progress.send(UpdateProgress::Downloading { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    let installation = state.installation.begin()?;
    let _ = on_progress.send(UpdateProgress::Installing);
    let update = update.clone();
    let installed = tauri::async_runtime::spawn_blocking(move || {
        let _installation = installation;
        update.install(bytes)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|result| result.map_err(|e| e.to_string()));
    if let Err(error) = installed {
        use tauri::Manager;
        // Windows may reject the installer after the before-exit hook drained resources.
        if app.state::<crate::Shutdown>().0.get().is_some() {
            let _ = on_progress.send(UpdateProgress::RestartRequired);
        }
        return Err(error);
    }
    let _ = on_progress.send(UpdateProgress::Restarting);
    crate::shutdown(&app).await;
    app.request_restart();
    Ok(())
}

#[tauri::command]
pub fn app_update_restart(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if app.state::<crate::Shutdown>().0.get().is_none() || !app.state::<Updates>().prepare_exit() {
        return Err("O aplicativo ainda não está pronto para reiniciar.".into());
    }
    app.request_restart();
    Ok(())
}
