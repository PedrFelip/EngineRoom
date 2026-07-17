export type EngineTierId = "fast" | "balanced" | "deep";

export interface EngineTier {
  id: EngineTierId;
  label: string;
  depth: number;
  hint: string;
}

export const ENGINE_TIERS: EngineTier[] = [
  { id: "fast", label: "Rápido", depth: 15, hint: "Pré-visualização rápida dos lances críticos." },
  { id: "balanced", label: "Equilibrado", depth: 20, hint: "Bom equilíbrio entre qualidade e tempo." },
  { id: "deep", label: "Profundo", depth: 25, hint: "Análise profunda, mais lenta por lance." },
];

export interface PgnMeta {
  white: string;
  black: string;
  whiteElo: string | null;
  blackElo: string | null;
  result: string;
  event: string | null;
  plies: number;
}

export interface ReviewConfig {
  pgn: string;
  meta: PgnMeta;
  engine: EngineTier;
}
