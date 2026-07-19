use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

/// The sidecar identifier passed to `Shell::sidecar`. Must be the **basename only**
/// (e.g. "stockfish"), NOT the full `externalBin` path ("binaries/stockfish").
/// tauri-build copies `src-tauri/binaries/stockfish-<triple>` to
/// `<target_dir>/stockfish`, and the resolver looks it up next to the main exe.
const SIDECAR: &str = "stockfish";

/// Event name emitted to the frontend for every UCI line printed by an engine.
pub const LINE_EVENT: &str = "engine://line";

/// Payload of [`LINE_EVENT`]: pairs an engine id with the UCI line it produced,
/// so the frontend can route lines to the right consumer when more than one
/// engine is alive.
#[derive(Serialize, Clone)]
pub struct EngineLine {
    pub id: String,
    pub line: String,
}

impl EngineLine {
    fn new(id: &str, line: &str) -> Self {
        Self {
            id: id.to_string(),
            line: line.to_string(),
        }
    }
}

/// Holds the currently running engine processes (if any), keyed by an
/// arbitrary id chosen by the caller (e.g. "primary", "live-wide").
#[derive(Default)]
pub struct EngineState {
    inner: Mutex<HashMap<String, EngineHandle>>,
}

impl EngineState {
    /// Registers a new engine under `id`. Fails if `id` is already in use —
    /// callers must `remove(id)` first to replace an engine.
    fn insert(&self, id: String, handle: EngineHandle) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|e| e.to_string())?;
        if guard.contains_key(&id) {
            return Err(format!("Engine '{id}' já está em execução."));
        }
        guard.insert(id, handle);
        Ok(())
    }

    /// Routes `line` to the stdin channel of the engine registered as `id`.
    fn send(&self, id: &str, line: String) -> Result<(), String> {
        let guard = self.inner.lock().map_err(|e| e.to_string())?;
        match guard.get(id) {
            Some(handle) => handle
                .tx
                .send(line)
                .map_err(|_| format!("Engine '{id}' desconectada.")),
            None => Err(format!("Engine '{id}' não está em execução.")),
        }
    }

    /// Removes `id` from the registry and signals its writer task to kill the
    /// child process. Returns `true` if `id` was registered, `false` otherwise.
    fn remove(&self, id: &str) -> bool {
        let Ok(mut guard) = self.inner.lock() else {
            return false;
        };
        match guard.remove(id) {
            Some(handle) => {
                let _ = handle.shutdown.send(());
                true
            }
            None => false,
        }
    }
}

struct EngineHandle {
    /// Channel used to send UCI commands to the engine's stdin.
    tx: mpsc::UnboundedSender<String>,
    /// Signalling this stops the writer task and kills the child.
    shutdown: oneshot::Sender<()>,
}

enum SpawnKind {
    Sidecar,
    Path(String),
}

fn spawn_engine(
    app: &AppHandle,
    id: &str,
    kind: SpawnKind,
) -> Result<(mpsc::UnboundedSender<String>, oneshot::Sender<()>), String> {
    match kind {
        SpawnKind::Sidecar => {
            let command = app
                .shell()
                .sidecar(SIDECAR)
                .map_err(|e| format!("Não foi possível localizar o Stockfish embarcado: {e}"))?;

            let (mut rx, child) = command
                .spawn()
                .map_err(|e| format!("Falha ao iniciar o Stockfish embarcado: {e}"))?;

            // Forward stdout lines to the frontend, tagged with the engine id.
            let app_reader = app.clone();
            let id_owned = id.to_string();
            tauri::async_runtime::spawn(async move {
                while let Some(event) = rx.recv().await {
                    if let CommandEvent::Stdout(bytes) = event {
                        let line = String::from_utf8_lossy(&bytes).trim().to_string();
                        if !line.is_empty() {
                            let _ = app_reader.emit(LINE_EVENT, EngineLine::new(&id_owned, &line));
                        }
                    }
                }
            });

            // Writer task: pumps frontend commands into stdin; kills on shutdown.
            let (tx, mut incoming) = mpsc::unbounded_channel::<String>();
            let (shutdown, mut shutdown_rx) = oneshot::channel::<()>();
            tauri::async_runtime::spawn(async move {
                let mut child = child;
                loop {
                    tokio::select! {
                        Some(message) = incoming.recv() => {
                            let payload = format!("{}\n", message);
                            let _ = child.write(payload.as_bytes());
                        }
                        _ = &mut shutdown_rx => {
                            let _ = child.kill();
                            break;
                        }
                    }
                }
            });

            Ok((tx, shutdown))
        }
        SpawnKind::Path(path) => {
            let mut child = Command::new(&path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("Falha ao iniciar '{}': {e}", path))?;

            let stdout = child.stdout.take().ok_or("stdout indisponível")?;
            let stdin = child.stdin.take().ok_or("stdin indisponível")?;

            let app_reader = app.clone();
            let id_owned = id.to_string();
            tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(raw)) = lines.next_line().await {
                    let line = raw.trim().to_string();
                    if !line.is_empty() {
                        let _ = app_reader.emit(LINE_EVENT, EngineLine::new(&id_owned, &line));
                    }
                }
            });

            let (tx, mut incoming) = mpsc::unbounded_channel::<String>();
            let (shutdown, mut shutdown_rx) = oneshot::channel::<()>();
            tauri::async_runtime::spawn(async move {
                let mut stdin = stdin;
                let mut child = child;
                loop {
                    tokio::select! {
                        Some(message) = incoming.recv() => {
                            let payload = format!("{}\n", message);
                            if stdin.write_all(payload.as_bytes()).await.is_ok() {
                                let _ = stdin.flush().await;
                            }
                        }
                        _ = &mut shutdown_rx => {
                            let _ = child.kill().await;
                            break;
                        }
                    }
                }
            });

            Ok((tx, shutdown))
        }
    }
}

#[tauri::command]
pub fn engine_spawn(
    app: AppHandle,
    state: tauri::State<'_, EngineState>,
    id: String,
    path: Option<String>,
) -> Result<(), String> {
    let kind = match path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(p) => SpawnKind::Path(p.to_string()),
        None => SpawnKind::Sidecar,
    };

    let (tx, shutdown) = spawn_engine(&app, &id, kind)?;
    state.insert(id, EngineHandle { tx, shutdown })
}

#[tauri::command]
pub fn engine_send(
    state: tauri::State<'_, EngineState>,
    id: String,
    line: String,
) -> Result<(), String> {
    state.send(&id, line)
}

#[tauri::command]
pub fn engine_stop(state: tauri::State<'_, EngineState>, id: String) -> Result<(), String> {
    state.remove(&id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns a handle plus its receiver so the caller can keep the receiver
    /// alive (otherwise `tx` reports as disconnected).
    fn dummy_handle() -> (EngineHandle, mpsc::UnboundedReceiver<String>) {
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        let (shutdown, _shutdown_rx) = oneshot::channel::<()>();
        (EngineHandle { tx, shutdown }, rx)
    }

    #[test]
    fn two_engines_with_different_ids_coexist() {
        // Behavior: the Rust layer must allow multiple engines to live simultaneously,
        // each addressed by id, with sends routed independently.
        let state = EngineState::default();
        let (h1, _rx1) = dummy_handle();
        state
            .insert("primary".to_string(), h1)
            .expect("primeiro insert deve sucesso");
        let (h2, _rx2) = dummy_handle();
        state
            .insert("live-wide".to_string(), h2)
            .expect("segundo insert deve sucesso");

        state
            .send("primary", "uci".to_string())
            .expect("send para primary deve sucesso");
        state
            .send("live-wide", "uci".to_string())
            .expect("send para live-wide deve sucesso");
    }

    #[test]
    fn inserting_duplicate_id_is_rejected() {
        // Behavior: trying to spawn a second engine under an id already in use
        // fails clearly, leaving the original engine untouched.
        let state = EngineState::default();
        let (h1, _rx1) = dummy_handle();
        state.insert("primary".to_string(), h1).unwrap();

        let (h2, _rx2) = dummy_handle();
        let err = state
            .insert("primary".to_string(), h2)
            .expect_err("duplicate insert deve falhar");
        assert!(
            err.contains("primary"),
            "mensagem de erro deve mencionar o id: {err}"
        );

        // Engine original segue enviável.
        state
            .send("primary", "uci".to_string())
            .expect("engine original deve continuar ativa");
    }

    #[test]
    fn stop_only_removes_targeted_engine() {
        // Behavior: stopping one engine by id leaves all others running.
        let state = EngineState::default();
        let (h1, _rx1) = dummy_handle();
        state.insert("primary".to_string(), h1).unwrap();
        let (h2, _rx2) = dummy_handle();
        state.insert("live-wide".to_string(), h2).unwrap();

        assert!(
            state.remove("primary"),
            "remove deve reportar sucesso para primary"
        );
        assert!(
            !state.remove("primary"),
            "remove numa segunda chamada deve reportar ausência"
        );
        state
            .send("live-wide", "uci".to_string())
            .expect("live-wide deve seguir ativa após parar primary");
    }

    #[test]
    fn send_to_unknown_id_reports_which_id_is_missing() {
        // Behavior: when a caller routes a line to an id that isn't registered,
        // the error mentions that id so the caller can diagnose the mismatch.
        let state = EngineState::default();
        let err = state
            .send("live-wide", "uci".to_string())
            .expect_err("send pra id inexistente deve falhar");
        assert!(
            err.contains("live-wide"),
            "erro deve mencionar o id procurado: {err}"
        );
    }

    #[test]
    fn line_payload_carries_engine_id_for_frontend_routing() {
        // Behavior: when an engine emits a line, the frontend must be able to
        // tell which engine produced it. The payload serializes as {"id","line"}
        // so each Tauri event is self-describing.
        let payload = EngineLine::new("live-wide", "uciok");
        let json = serde_json::to_string(&payload).unwrap();
        assert_eq!(json, r#"{"id":"live-wide","line":"uciok"}"#);
    }
}
