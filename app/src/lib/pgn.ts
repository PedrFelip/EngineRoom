import { Chess } from "chess.js";
import type { PgnMeta } from "../types";

export type PgnParseResult =
  | { ok: true; meta: PgnMeta }
  | { ok: false; error: string };

const UNKNOWN = "?";

function cleanPlayer(name: string | null | undefined): string {
  if (!name || name === UNKNOWN) return "Jogador";
  return name.trim();
}

export function parsePgn(pgn: string): PgnParseResult {
  const trimmed = pgn.trim();
  if (!trimmed) return { ok: false, error: "Cole um PGN ou selecione um arquivo." };

  try {
    const chess = new Chess();
    chess.loadPgn(trimmed);

    const h = chess.header();
    const plies = chess.history().length;

    if (plies === 0) {
      return { ok: false, error: "O PGN não contém lances." };
    }

    const meta: PgnMeta = {
      white: cleanPlayer(h.White),
      black: cleanPlayer(h.Black),
      whiteElo: h.WhiteElo ?? null,
      blackElo: h.BlackElo ?? null,
      result: h.Result && h.Result !== UNKNOWN ? h.Result : "*",
      event: h.Event && h.Event !== UNKNOWN ? h.Event : null,
      plies,
    };

    return { ok: true, meta };
  } catch {
    return {
      ok: false,
      error: "PGN inválido. Verifique se o texto está no formato PGN correto.",
    };
  }
}

export function resultLabel(result: string): string {
  switch (result) {
    case "1-0":
      return "1–0 Brancas";
    case "0-1":
      return "0–1 Pretas";
    case "1/2-1/2":
      return "Empate";
    default:
      return "Em andamento";
  }
}
