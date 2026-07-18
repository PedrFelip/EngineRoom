export interface EcoEntry {
  code: string;
  name: string;
  moves: string[];
}

/**
 * Encontra a entrada ECO cuja sequência de lances (SAN) é prefixo do jogo
 * jogado, escolhendo a mais longa (mais específica). Retorna null sem casamento.
 * Puro e sem dependências — o dataset é injetado.
 */
export function lookupEco(played: string[], dataset: EcoEntry[]): EcoEntry | null {
  let best: EcoEntry | null = null;
  for (const entry of dataset) {
    const n = entry.moves.length;
    if (n === 0 || n > played.length) continue;
    let ok = true;
    for (let i = 0; i < n; i++) {
      if (entry.moves[i] !== played[i]) {
        ok = false;
        break;
      }
    }
    if (ok && (!best || n > best.moves.length)) best = entry;
  }
  return best;
}

let cachedEntries: EcoEntry[] | null = null;

/**
 * Lookup sobre o dataset ECO offline (lichess chess-openings, A00–E99).
 * Carregado sob demanda (dynamic import) para não pesar o chunk principal.
 * Se o dataset falhar ao carregar, devolve null — abertura é enriquecimento,
 * não pode derrubar a análise. O cache só é preenchido em caso de sucesso,
 * então uma falha transitória é tentada de novo na próxima análise.
 */
export async function lookupOpening(played: string[]): Promise<EcoEntry | null> {
  if (!cachedEntries) {
    try {
      const mod = await import("../data/eco.json");
      cachedEntries = (mod.default as EcoEntry[]).slice();
    } catch {
      return null;
    }
  }
  return lookupEco(played, cachedEntries);
}
