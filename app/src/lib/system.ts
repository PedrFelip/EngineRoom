import { invoke } from '@tauri-apps/api/core'

export interface SystemResources {
  threads: number
  memory_mb: number
}

/** Logical CPU cores + total system RAM (MB), used to size the Stockfish engine. */
export function getSystemResources(): Promise<SystemResources> {
  return invoke<SystemResources>('system_resources')
}

/// Sizes the Stockfish hash table to ~20% of system RAM, clamped to a sane range
/// (512 MB floor, 4 GB ceiling). For fixed-depth per-position analysis this is the
/// sweet spot: enough to cache cross-position transpositions without starving the
/// OS/webview. Beyond ~4 GB the returns flatten.
export function recommendedHashMb(memoryMb: number): number {
  const hash = Math.floor(memoryMb * 0.2)
  return Math.min(Math.max(hash, 512), 4 * 1024)
}

export type LivePresetId = 'leve' | 'equilibrado' | 'pesado'

export interface EngineSizing {
  threads: number
  hashMb: number
}

/**
 * Um preset de sizing para o refino ao vivo. Cada preset define recursos para
 * a engine pesada (SF18) e a leve (SF17) — a leve sempre recebe menos por ser
 * a variação "barata".
 */
export interface LivePreset {
  id: LivePresetId
  label: string
  deep: EngineSizing
  wide: EngineSizing
}

function clamp(min: number, v: number, max: number): number {
  return Math.max(min, Math.min(max, v))
}

/**
 * Calcula os três presets com base nos recursos da máquina. Hash total nunca
 * passa de ~50% da RAM (deep + wide somados no pesado) pra não enforcar o
 * sistema.
 */
export function computePresets(sys: SystemResources): LivePreset[] {
  const cores = Math.max(1, sys.threads)
  const ram = Math.max(1024, sys.memory_mb)
  return [
    {
      id: 'leve',
      label: 'Leve',
      deep: { threads: 1, hashMb: 128 },
      wide: { threads: 1, hashMb: 64 },
    },
    {
      id: 'equilibrado',
      label: 'Equilibrado',
      deep: {
        threads: Math.max(2, Math.floor(cores / 2)),
        hashMb: clamp(256, Math.floor(ram / 8), 1024),
      },
      wide: { threads: 2, hashMb: 128 },
    },
    {
      id: 'pesado',
      label: 'Pesado',
      deep: {
        threads: cores,
        hashMb: clamp(512, Math.floor(ram / 4), 4096),
      },
      wide: { threads: 2, hashMb: 256 },
    },
  ]
}

/** Preset default em primeira execução — Equilibrado. */
export const DEFAULT_PRESET_ID: LivePresetId = 'equilibrado'
