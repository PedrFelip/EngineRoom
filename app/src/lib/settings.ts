export type Theme = 'dark' | 'light'

export type LivePresetId = 'leve' | 'equilibrado' | 'pesado'

export interface Settings {
  theme: Theme
  /** Empty string = use the embedded Stockfish sidecar. Otherwise an absolute path. */
  enginePath: string
  /** Toca som ao avançar um lance na revisão. */
  soundEnabled: boolean
  /** Volume do som de movimentação, em [0, 1]. */
  soundVolume: number
  /** Preset de sizing aplicado às engines no refino ao vivo. */
  livePreset: LivePresetId
  /** Liga/desliga a engine leve (variações) no refino ao vivo. Persiste entre sessões. */
  liveWideOn: boolean
}

export const SETTINGS_KEY = 'engineroom.settings.v1'

export const DEFAULT_SETTINGS: Settings = {
  theme: 'dark',
  enginePath: '',
  soundEnabled: true,
  soundVolume: 0.7,
  livePreset: 'equilibrado',
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
      livePreset: parsePreset(parsed.livePreset),
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

/** Persistido deve ser um id válido; default Equilibrado. */
function parsePreset(v: unknown): LivePresetId {
  return v === 'leve' || v === 'pesado' ? v : 'equilibrado'
}

export function saveSettings(settings: Settings): void {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings))
  } catch {
    /* ignore quota / privacy errors */
  }
}
