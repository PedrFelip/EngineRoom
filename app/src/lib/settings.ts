export type Theme = 'dark' | 'light'

export interface Settings {
  theme: Theme
  /** Toca som ao avançar um lance na revisão. */
  soundEnabled: boolean
  /** Volume do som de movimentação, em [0, 1]. */
  soundVolume: number
  /** Habilita o mock da futura análise exploratória durante a revisão. */
  reviewEngineEnabled: boolean
  reviewSearchSeconds: number
  reviewAnalysisLines: number
  reviewThreadsAuto: boolean
  reviewThreads: number
  reviewMemoryMb: number
}

export const SETTINGS_KEY = 'engineroom.settings.v1'

export const DEFAULT_SETTINGS: Settings = {
  theme: 'dark',
  soundEnabled: true,
  soundVolume: 0.7,
  reviewEngineEnabled: true,
  reviewSearchSeconds: 8,
  reviewAnalysisLines: 1,
  reviewThreadsAuto: true,
  reviewThreads: 1,
  reviewMemoryMb: 16,
}

function clampInteger(
  value: unknown,
  min: number,
  max: number,
  fallback: number,
) {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.max(min, Math.min(max, Math.round(value)))
    : fallback
}

export function recommendedReviewThreads(logicalCores: number): number {
  return Math.max(1, Math.ceil(logicalCores / 3))
}

export function loadSettings(): Settings {
  if (typeof localStorage === 'undefined') return { ...DEFAULT_SETTINGS }
  try {
    const raw = localStorage.getItem(SETTINGS_KEY)
    if (!raw) return { ...DEFAULT_SETTINGS }
    const parsed = JSON.parse(raw) as Partial<Settings>
    return {
      theme: parsed.theme === 'light' ? 'light' : 'dark',
      soundEnabled: parsed.soundEnabled !== false,
      soundVolume: clampVolume(parsed.soundVolume),
      reviewEngineEnabled: parsed.reviewEngineEnabled !== false,
      reviewSearchSeconds: clampInteger(
        parsed.reviewSearchSeconds,
        1,
        30,
        DEFAULT_SETTINGS.reviewSearchSeconds,
      ),
      reviewAnalysisLines: clampInteger(
        parsed.reviewAnalysisLines,
        1,
        5,
        DEFAULT_SETTINGS.reviewAnalysisLines,
      ),
      reviewThreadsAuto: parsed.reviewThreadsAuto !== false,
      reviewThreads: clampInteger(
        parsed.reviewThreads,
        1,
        256,
        DEFAULT_SETTINGS.reviewThreads,
      ),
      reviewMemoryMb: clampInteger(
        parsed.reviewMemoryMb,
        16,
        1024,
        DEFAULT_SETTINGS.reviewMemoryMb,
      ),
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

export function saveSettings(settings: Settings): void {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings))
  } catch {
    /* ignore quota / privacy errors */
  }
}
