import { invoke } from "@tauri-apps/api/core";

export interface SystemResources {
  threads: number;
  memory_mb: number;
}

/** Logical CPU cores + total system RAM (MB), used to size the Stockfish engine. */
export function getSystemResources(): Promise<SystemResources> {
  return invoke<SystemResources>("system_resources");
}

/// Sizes the Stockfish hash table to ~20% of system RAM, clamped to a sane range
/// (512 MB floor, 4 GB ceiling). For fixed-depth per-position analysis this is the
/// sweet spot: enough to cache cross-position transpositions without starving the
/// OS/webview. Beyond ~4 GB the returns flatten.
export function recommendedHashMb(memoryMb: number): number {
  const hash = Math.floor(memoryMb * 0.2);
  return Math.min(Math.max(hash, 512), 4 * 1024);
}
