export type Theme = 'dark' | 'light'

export interface Settings {
  theme: Theme
  /** Empty string = use the embedded Stockfish sidecar. Otherwise an absolute path. */
  enginePath: string
  /** Toca som ao avançar um lance na revisão. */
  soundEnabled: boolean
  /** Volume do som de movimentação, em [0, 1]. */
  soundVolume: number
  /** Threads da engine pesada durante o refino ao vivo. */
  liveThreads: number
  /** Hash (MB) da engine pesada durante o refino ao vivo. */
  liveHashMb: number
  /** Liga/desliga a engine leve (variações) no refino ao vivo. Persiste entre sessões. */
  liveWideOn: boolean
}

export const SETTINGS_KEY = 'engineroom.settings.v1'

export const DEFAULT_SETTINGS: Settings = {
  theme: 'dark',
  enginePath: '',
  soundEnabled: true,
  soundVolume: 0.7,
  liveThreads: 2,
  liveHashMb: 256,
  liveWideOn: true,
}

export function loadSettings(): Settings {
  if (typeof localStorage === 'undefined') return { ...DEFAULT_SETTINGS }
  try {
    const raw = localStorage.getItem(SETTINGS_KEY)
    if (!raw) return { ...DEFAULT_SETTINGS }
    const parsed = JSON.parse(raw) as Partial<Settings>
    return {
      theme: parsed.theme === 'light' ? 'light' : 'dark',
      enginePath:
        typeof parsed.enginePath === 'string' ? parsed.enginePath : '',
      soundEnabled: parsed.soundEnabled !== false,
      soundVolume: clampVolume(parsed.soundVolume),
      liveThreads: clampInt(parsed.liveThreads, 1, 64, DEFAULT_SETTINGS.liveThreads),
      liveHashMb: clampInt(
        parsed.liveHashMb,
        16,
        4096,
        DEFAULT_SETTINGS.liveHashMb,
      ),
      liveWideOn: parsed.liveWideOn !== false,
    }
  } catch {
    return { ...DEFAULT_SETTINGS }
  }
}

/** Volume armazenado deve ser um número em [0, 1]; caso contrário, usa o padrão. */
function clampVolume(v: unknown): number {
  return typeof v === 'number' && Number.isFinite(v)
    ? Math.max(0, Math.min(1, v))
    : DEFAULT_SETTINGS.soundVolume
}

/** Número inteiro dentro de [min, max]; default se inválido. */
function clampInt(
  v: unknown,
  min: number,
  max: number,
  fallback: number,
): number {
  if (typeof v !== 'number' || !Number.isFinite(v)) return fallback
  const i = Math.round(v)
  return Math.max(min, Math.min(max, i))
}

export function saveSettings(settings: Settings): void {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings))
  } catch {
    /* ignore quota / privacy errors */
  }
}
