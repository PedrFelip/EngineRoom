use serde::Serialize;
use sysinfo::System;

#[derive(Serialize)]
pub struct SystemResources {
    pub threads: usize,
    pub memory_mb: u64,
}

/// Logical CPU cores and total system memory (MB).
///
/// Used by the frontend to size the Stockfish engine (`Threads` / `Hash`)
/// so the analysis uses as much of the machine as possible.
#[tauri::command]
pub fn system_resources() -> SystemResources {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let mut sys = System::new();
    sys.refresh_memory();
    let memory_mb = sys.total_memory() / (1024 * 1024);

    SystemResources { threads, memory_mb }
}
