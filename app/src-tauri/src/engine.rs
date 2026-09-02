use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;
use tokio::sync::{mpsc, oneshot};

/// The sidecar identifier passed to `Shell::sidecar`. Must be the **basename only**
/// (e.g. "stockfish"), NOT the full `externalBin` path ("binaries/stockfish").
/// tauri-build copies `src-tauri/binaries/stockfish-<triple>` to
/// `<target_dir>/stockfish`, and the resolver looks it up next to the main exe.
const SIDECAR: &str = "stockfish";

/// Event name emitted to the frontend for UCI lines relevantes à revisão.
/// Linhas `info` intermediárias são compactadas antes de cruzar o IPC.
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

/// Reduz o volume de eventos durante uma busca sem mudar o contrato UCI visto
/// pelo frontend. Stockfish produz muitas linhas `info`; para o resultado da
/// posição, só importam as linhas da maior profundidade, uma por MultiPV,
/// imediatamente antes de `bestmove`.
#[derive(Default)]
struct UciOutputFilter {
    searching: bool,
    depth: Option<u32>,
    latest_lines: BTreeMap<u32, String>,
}

impl UciOutputFilter {
    fn on_command(&mut self, command: &str) {
        if command.trim().starts_with("go ") {
            self.searching = true;
            self.depth = None;
            self.latest_lines.clear();
        }
    }

    fn on_line(&mut self, line: String) -> Vec<String> {
        if !self.searching {
            return vec![line];
        }

        if line.starts_with("info ") {
            self.record_info(line);
            return Vec::new();
        }

        if line.starts_with("bestmove") {
            self.searching = false;
            self.depth = None;
            let mut output = std::mem::take(&mut self.latest_lines)
                .into_values()
                .collect::<Vec<_>>();
            output.push(line);
            return output;
        }

        vec![line]
    }

    fn record_info(&mut self, line: String) {
        let mut tokens = line.split_whitespace();
        let mut depth = None;
        let mut multipv = 1;
        let mut has_score = false;

        while let Some(token) = tokens.next() {
            match token {
                "depth" => depth = tokens.next().and_then(|value| value.parse().ok()),
                "multipv" => {
                    multipv = tokens
                        .next()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(1)
                }
                "score" => has_score = true,
                _ => {}
            }
        }

        let Some(depth) = depth else {
            return;
        };
        if !has_score {
            return;
        }

        match self.depth {
            Some(current) if depth < current => {}
            Some(current) if depth == current => {
                self.latest_lines.insert(multipv, line);
            }
            _ => {
                self.depth = Some(depth);
                self.latest_lines.clear();
                self.latest_lines.insert(multipv, line);
            }
        }
    }
}

fn forward_line(app: &AppHandle, filter: &Arc<Mutex<UciOutputFilter>>, line: String) {
    let output = filter.lock().expect("filtro UCI envenenado").on_line(line);
    for line in output {
        let _ = app.emit(LINE_EVENT, line);
    }
}

fn spawn_engine(
    app: &AppHandle,
) -> Result<(mpsc::UnboundedSender<String>, oneshot::Sender<()>), String> {
    let command = app
        .shell()
        .sidecar(SIDECAR)
        .map_err(|e| format!("Não foi possível localizar o Stockfish embarcado: {e}"))?;

    let (mut rx, child) = command
        .spawn()
        .map_err(|e| format!("Falha ao iniciar o Stockfish embarcado: {e}"))?;

    let filter = Arc::new(Mutex::new(UciOutputFilter::default()));
    // Divide chunks em linhas antes de filtrá-las. O shell plugin pode
    // agrupar mais de uma linha UCI no mesmo evento stdout.
    let app_reader = app.clone();
    let reader_filter = Arc::clone(&filter);
    tauri::async_runtime::spawn(async move {
        let mut reported_exit = false;
        let mut stdout = Vec::new();
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(bytes) => {
                    stdout.extend_from_slice(&bytes);
                    while let Some(end) = stdout.iter().position(|byte| *byte == b'\n') {
                        let raw = stdout.drain(..=end).collect::<Vec<_>>();
                        let line = String::from_utf8_lossy(&raw).trim().to_string();
                        if !line.is_empty() {
                            forward_line(&app_reader, &reader_filter, line);
                        }
                    }
                }
                CommandEvent::Terminated(p) => {
                    if !reported_exit {
                        reported_exit = true;
                        let _ = app_reader.emit(
                            EXIT_EVENT,
                            EngineExit {
                                code: p.code,
                                signal: p.signal,
                                error: None,
                            },
                        );
                    }
                }
                CommandEvent::Error(err) => {
                    if !reported_exit {
                        reported_exit = true;
                        let _ = app_reader.emit(
                            EXIT_EVENT,
                            EngineExit {
                                code: None,
                                signal: None,
                                error: Some(err),
                            },
                        );
                    }
                }
                _ => {}
            }
        }
        let line = String::from_utf8_lossy(&stdout).trim().to_string();
        if !line.is_empty() {
            forward_line(&app_reader, &reader_filter, line);
        }
        // Channel closed = the process is gone but no Terminated/Error event
        // was surfaced. Emit a generic exit so the frontend still fails fast
        // instead of hanging to its ask() timeout.
        if !reported_exit {
            let _ = app_reader.emit(
                EXIT_EVENT,
                EngineExit {
                    code: None,
                    signal: None,
                    error: Some("stdout fechado sem evento de término".into()),
                },
            );
        }
    });

    // Writer task: pumps frontend commands into stdin; kills on shutdown.
    let (tx, mut incoming) = mpsc::unbounded_channel::<String>();
    let (shutdown, mut shutdown_rx) = oneshot::channel::<()>();
    let writer_filter = Arc::clone(&filter);
    tauri::async_runtime::spawn(async move {
        let mut child = child;
        loop {
            tokio::select! {
                Some(message) = incoming.recv() => {
                    writer_filter
                        .lock()
                        .expect("filtro UCI envenenado")
                        .on_command(&message);
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

#[tauri::command]
pub fn engine_spawn(app: AppHandle, state: tauri::State<'_, EngineState>) -> Result<(), String> {
    let mut guard = state.inner.lock().map_err(|e| e.to_string())?;
    if guard.is_some() {
        return Err("A engine já está em execução.".into());
    }

    let (tx, shutdown) = spawn_engine(&app)?;
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

#[cfg(test)]
#[path = "engine/tests.rs"]
mod tests;
