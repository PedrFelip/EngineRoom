use serde::Serialize;
use sysinfo::System;

#[derive(Serialize)]
pub struct SystemResources {
    pub threads: usize,
    pub memory_mb: u64,
}

/// Physical CPU cores and total system memory (MB).
///
/// Used by the frontend to size the Stockfish engine. Stockfish's sweet spot is
/// the number of **physical** cores (more threads than that adds noise without
/// real speedup and weakens determinism), so we report physical cores and fall
/// back to logical/2 only when the OS doesn't expose them.
#[tauri::command]
pub fn system_resources() -> SystemResources {
    let logical = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let mut sys = System::new();
    sys.refresh_memory();
    let memory_mb = sys.total_memory() / (1024 * 1024);

    let threads = sys
        .physical_core_count()
        .filter(|&n| n >= 1)
        .unwrap_or_else(|| (logical / 2).max(1));

    SystemResources { threads, memory_mb }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_nonzero_resources() {
        let r = system_resources();
        assert!(r.threads >= 1);
        assert!(r.memory_mb >= 512, "memory_mb too low: {}", r.memory_mb);
        eprintln!("threads={} memory_mb={}", r.threads, r.memory_mb);
    }
}
