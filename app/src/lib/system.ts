import { invoke } from "@tauri-apps/api/core";

export interface SystemResources {
  threads: number;
  memory_mb: number;
}

/** Logical CPU cores + total system RAM (MB), used to size the Stockfish engine. */
export function getSystemResources(): Promise<SystemResources> {
  return invoke<SystemResources>("system_resources");
}

/// Bounds the Stockfish hash table to ~70% of system RAM, clamped to a sane range
/// (256 MB floor, 32 GB ceiling). More hash rarely helps and starving the OS hurts.
export function recommendedHashMb(memoryMb: number): number {
  const hash = Math.floor(memoryMb * 0.7);
  return Math.min(Math.max(hash, 256), 32 * 1024);
}
