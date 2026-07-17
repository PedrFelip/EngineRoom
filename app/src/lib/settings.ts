export type Theme = "dark" | "light";

export interface Settings {
  theme: Theme;
  /** Empty string = use the embedded Stockfish sidecar. Otherwise an absolute path. */
  enginePath: string;
}

export const SETTINGS_KEY = "engineroom.settings.v1";

export const DEFAULT_SETTINGS: Settings = {
  theme: "dark",
  enginePath: "",
};

export function loadSettings(): Settings {
  if (typeof localStorage === "undefined") return { ...DEFAULT_SETTINGS };
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (!raw) return { ...DEFAULT_SETTINGS };
    const parsed = JSON.parse(raw) as Partial<Settings>;
    return {
      theme: parsed.theme === "light" ? "light" : "dark",
      enginePath: typeof parsed.enginePath === "string" ? parsed.enginePath : "",
    };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export function saveSettings(settings: Settings): void {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
  } catch {
    /* ignore quota / privacy errors */
  }
}
