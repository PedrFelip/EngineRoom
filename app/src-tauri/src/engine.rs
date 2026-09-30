//! Shared stdout framing and retained IPC benchmark transport.
use std::collections::BTreeMap;
use tauri::AppHandle;
use tokio::sync::mpsc;

/// Transporte sem sidecar usado exclusivamente pelo benchmark de IPC. Mantém
/// o mesmo enfileiramento e filtro de comandos do writer real, sem deixar uma
/// busca do Stockfish contaminar a medição.
pub struct BenchmarkUciState {
    tx: mpsc::UnboundedSender<String>,
}

impl Default for BenchmarkUciState {
    fn default() -> Self {
        let (tx, mut incoming) = mpsc::unbounded_channel::<String>();
        tauri::async_runtime::spawn(async move {
            let mut filter = UciOutputFilter::default();
            while let Some(message) = incoming.recv().await {
                filter.on_command(&message);
            }
        });
        Self { tx }
    }
}

pub struct BenchmarkMode(pub bool);

/// Reduz o volume de eventos durante uma busca sem mudar o contrato UCI visto
/// pelo frontend. Stockfish produz muitas linhas `info`; para o resultado da
/// posição, só importam as linhas da maior profundidade, uma por MultiPV,
/// imediatamente antes de `bestmove`.
#[allow(dead_code)]
#[derive(Default)]
struct UciOutputFilter {
    searching: bool,
    latest_lines: BTreeMap<u32, (u32, String)>,
}

#[allow(dead_code)]
impl UciOutputFilter {
    fn on_command(&mut self, command: &str) {
        if command.trim().starts_with("go ") {
            self.searching = true;
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
            let mut output = std::mem::take(&mut self.latest_lines)
                .into_values()
                .map(|(_, line)| line)
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

        let should_replace = self
            .latest_lines
            .get(&multipv)
            .is_none_or(|(current_depth, _)| depth >= *current_depth);
        if should_replace {
            self.latest_lines.insert(multipv, (depth, line));
        }
    }
}

/// Consome as linhas UCI completas acumuladas em `stdout`.
///
/// Um evento do shell pode conter várias linhas. Guardamos o sufixo sem `\n`
/// para o próximo evento e removemos todo o prefixo já consumido apenas uma
/// vez; remover cada linha individualmente deslocaria o restante do buffer a
/// cada `drain`.
pub(super) fn consume_stdout_lines(stdout: &mut Vec<u8>, mut on_line: impl FnMut(String)) {
    let mut start = 0;

    for end in 0..stdout.len() {
        if stdout[end] != b'\n' {
            continue;
        }

        let line = String::from_utf8_lossy(&stdout[start..end])
            .trim()
            .to_string();
        if !line.is_empty() {
            on_line(line);
        }
        start = end + 1;
    }

    if start > 0 {
        stdout.drain(..start);
    }
}

#[cfg(test)]
fn enqueue_uci_lines(
    tx: &mpsc::UnboundedSender<String>,
    lines: impl IntoIterator<Item = String>,
) -> Result<(), String> {
    for line in lines {
        tx.send(line)
            .map_err(|_| "Não foi possível enviar comando à engine.".to_string())?;
    }
    Ok(())
}

fn ensure_benchmark_mode(mode: &BenchmarkMode) -> Result<(), String> {
    if mode.0 {
        Ok(())
    } else {
        Err("Comando disponível somente no modo de benchmark.".into())
    }
}

fn enqueue_benchmark_lines(
    state: &BenchmarkUciState,
    lines: impl IntoIterator<Item = String>,
) -> Result<(), String> {
    for line in lines {
        state
            .tx
            .send(line)
            .map_err(|_| "Não foi possível enviar comando ao benchmark UCI.".to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn benchmark_uci_enabled(mode: tauri::State<'_, BenchmarkMode>) -> bool {
    mode.0
}

#[tauri::command]
pub fn benchmark_uci_send(
    mode: tauri::State<'_, BenchmarkMode>,
    state: tauri::State<'_, BenchmarkUciState>,
    line: String,
) -> Result<(), String> {
    ensure_benchmark_mode(&mode)?;
    enqueue_benchmark_lines(&state, [line])
}

#[tauri::command]
pub fn benchmark_uci_send_batch(
    mode: tauri::State<'_, BenchmarkMode>,
    state: tauri::State<'_, BenchmarkUciState>,
    lines: Vec<String>,
) -> Result<(), String> {
    ensure_benchmark_mode(&mode)?;
    if lines.is_empty() {
        return Err("O batch UCI não pode estar vazio.".into());
    }
    enqueue_benchmark_lines(&state, lines)
}

#[tauri::command]
pub fn benchmark_uci_report(
    app: AppHandle,
    mode: tauri::State<'_, BenchmarkMode>,
    report: serde_json::Value,
) -> Result<(), String> {
    ensure_benchmark_mode(&mode)?;
    println!("UCI_IPC_BENCHMARK_JSON={report}");
    app.exit(0);
    Ok(())
}

#[cfg(test)]
#[path = "engine/tests.rs"]
mod tests;
