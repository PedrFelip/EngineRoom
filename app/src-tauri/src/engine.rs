use std::process::Stdio;
use std::sync::Mutex;
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

/// Event name emitted to the frontend for every UCI line printed by the engine.
pub const LINE_EVENT: &str = "engine://line";

/// Event name emitted to the frontend when the engine process exits — cleanly
/// or crashing. Without this, a swallowed crash (SIGSEGV/OOM) looks identical
/// to "still thinking" from the frontend, which hangs `ask()` to its timeout.
pub const EXIT_EVENT: &str = "engine://exit";

/// Payload of [`EXIT_EVENT`]: why/how the engine process ended.
#[derive(Clone, serde::Serialize)]
struct EngineExit {
    /// Exit code, when known (clean exit or code-bearing termination).
    code: Option<i32>,
    /// Signal number that killed the process, if any (e.g. 11 = SIGSEGV).
    signal: Option<i32>,
    /// Plugin error string (UTF-8/IO failure), when that's the cause.
    error: Option<String>,
}

/// Holds the currently running engine process (if any).
#[derive(Default)]
pub struct EngineState {
    inner: Mutex<Option<EngineHandle>>,
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

            // Forward stdout lines to the frontend.
            let app_reader = app.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(event) = rx.recv().await {
                    if let CommandEvent::Stdout(bytes) = event {
                        let line = String::from_utf8_lossy(&bytes).trim().to_string();
                        if !line.is_empty() {
                            let _ = app_reader.emit(LINE_EVENT, line);
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
            tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(raw)) = lines.next_line().await {
                    let line = raw.trim().to_string();
                    if !line.is_empty() {
                        let _ = app_reader.emit(LINE_EVENT, line);
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
    path: Option<String>,
) -> Result<(), String> {
    let mut guard = state.inner.lock().map_err(|e| e.to_string())?;
    if guard.is_some() {
        return Err("A engine já está em execução.".into());
    }

    let kind = match path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(p) => SpawnKind::Path(p.to_string()),
        None => SpawnKind::Sidecar,
    };

    let (tx, shutdown) = spawn_engine(&app, kind)?;
    *guard = Some(EngineHandle { tx, shutdown });
    Ok(())
}

#[tauri::command]
pub fn engine_send(state: tauri::State<'_, EngineState>, line: String) -> Result<(), String> {
    let guard = state.inner.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(handle) => handle
            .tx
            .send(line)
            .map_err(|_| "Não foi possível enviar comando à engine.".into()),
        None => Err("A engine não está em execução.".into()),
    }
}

#[tauri::command]
pub fn engine_stop(state: tauri::State<'_, EngineState>) -> Result<(), String> {
    let mut guard = state.inner.lock().map_err(|e| e.to_string())?;
    if let Some(handle) = guard.take() {
        let _ = handle.shutdown.send(());
    }
    Ok(())
}
