import type { Phase } from '../types'

export type { Phase }

/**
 * Fases do jogo (Abertura / Meio-jogo / Final) a partir do estado do tabuleiro,
 * lido do FEN. Núcleo puro, sem efeitos colaterais, sem chess.js.
 *
 * Sinais (todos derivados do posicionamento de peças):
 *  - Contagem de peças maiores/menores (cavalos, bispos, torres, damas);
 *  - Densidade da fileira de trás (sinal de desenvolvimento/roque);
 *  - Interação entre peças dos dois lados (mixedness).
 *
 * `phaseOfPosition(fen)` classifica uma posição isolada. `computePhases` aplica
 * um travamento monotônico sobre a sequência — a fase só avança, nunca regredir
 * (uma promoção que aumente a contagem não desfaz a fase já atingida).
 */

/** Meio-jogo quando a contagem de maiores/menores cai para este valor ou menos. */
const MIDGAME_MAX_PIECES = 10
/** Final quando a contagem de maiores/menores cai para este valor ou menos. */
const ENDGAME_MAX_PIECES = 6
/** Meio-jogo quando a interação entre as peças (mixedness) ultrapassa este valor. */
const MIXEDNESS_THRESHOLD = 150

/**
 * Conta as peças maiores e menores (cavalos, bispos, torres e damas) dos dois
 * lados a partir do FEN. Reis e peões não entram. Inicial = 14.
 */
export function majorsAndMinors(fen: string): number {
  const placement = fen.split(' ')[0]
  let count = 0
  for (const ch of placement) {
    if ('nbrqNBRQ'.includes(ch)) count++
  }
  return count
}

/**
 * Fileira de trás "rala": menos de 4 peças do lado na casa inicial indica que
 * as peças se desenvolveram (e/ou o rei rocou). Brancas na 1ª fileira (ranks[7]),
 * pretas na 8ª fileira (ranks[0]).
 */
export function backrankSparse(fen: string): boolean {
  const ranks = fen.split(' ')[0].split('/')
  const count = (rank: string, isUpper: boolean): number => {
    let n = 0
    for (const ch of rank) {
      const upper = ch >= 'A' && ch <= 'Z'
      if ((isUpper && upper) || (!isUpper && ch >= 'a' && ch <= 'z')) n++
    }
    return n
  }
  return count(ranks[7], true) < 4 || count(ranks[0], false) < 4
}

/**
 * Pontuação de interação de uma região 2×2 dado o número de peças brancas e
 * pretas ali e a linha y (1..7). Valores mais altos indicam peças dos dois
 * lados conflitando numa mesma zona. Tabela de referência fixa.
 */
export function regionScore(y: number, white: number, black: number): number {
  switch (white) {
    case 0:
      switch (black) {
        case 1:
          return 1 + y
        case 2:
          return y < 6 ? 2 + (6 - y) : 0
        case 3:
          return y < 7 ? 3 + (7 - y) : 0
        case 4:
          return y < 7 ? 3 + (7 - y) : 0
        default:
          return 0
      }
    case 1:
      switch (black) {
        case 0:
          return 1 + (8 - y)
        case 1:
          return 5 + Math.abs(4 - y)
        case 2:
          return 4 + (7 - y)
        case 3:
          return 5 + (7 - y)
        default:
          return 0
      }
    case 2:
      switch (black) {
        case 0:
          return y > 2 ? 2 + (y - 2) : 0
        case 1:
          return 4 + (y - 1)
        case 2:
          return 7
        default:
          return 0
      }
    case 3:
      switch (black) {
        case 0:
          return y > 1 ? 3 + (y - 1) : 0
        case 1:
          return 5 + (y - 1)
        default:
          return 0
      }
    case 4:
      switch (black) {
        case 0:
          return y > 1 ? 3 + (y - 1) : 0
        default:
          return 0
      }
    default:
      return 0
  }
}

/**
 * Soma de interação das 49 regiões 2×2 (grade 7×7) do tabuleiro. Cada região
 * contribui com `regionScore(y, brancas, pretas)`. Valores altos indicam peças
 * dos dois lados disputando as mesmas zonas (sinal de meio-jogo).
 */
export function mixedness(fen: string): number {
  const ranks = fen.split(' ')[0].split('/')
  // board[rank][file], rank 0 = 1ª fileira (baixo), file 0 = coluna a.
  const board: ('w' | 'b' | null)[][] = []
  for (let fenRank = 0; fenRank < 8; fenRank++) {
    const row: ('w' | 'b' | null)[] = new Array(8).fill(null)
    let file = 0
    for (const ch of ranks[fenRank]) {
      if (ch >= '1' && ch <= '8') file += Number(ch)
      else {
        row[file] = ch >= 'A' && ch <= 'Z' ? 'w' : 'b'
        file++
      }
    }
    board[7 - fenRank] = row
  }

  let acc = 0
  for (let y = 0; y <= 6; y++) {
    for (let x = 0; x <= 6; x++) {
      let w = 0
      let b = 0
      for (let dy = 0; dy <= 1; dy++) {
        for (let dx = 0; dx <= 1; dx++) {
          const cell = board[y + dy][x + dx]
          if (cell === 'w') w++
          else if (cell === 'b') b++
        }
      }
      acc += regionScore(y + 1, w, b)
    }
  }
  return acc
}

/** Ordem das fases (Abertura < Meio-jogo < Final) para o travamento monotônico. */
const PHASE_ORDER: Record<Phase, number> = {
  opening: 0,
  middlegame: 1,
  endgame: 2,
}

/**
 * Fase de cada posição (paralelo ao vetor de entrada), via `phaseOfPosition`.
 * A fase só avança: uma vez atingida uma fase, posições posteriores com mais
 * peças (p.ex. por promoção de peão em dama) não regrediram — garante faixas
 * contíguas no gráfico.
 */
export function computePhases(positions: { fen: string }[]): Phase[] {
  let current: Phase = 'opening'
  return positions.map((p) => {
    const raw = phaseOfPosition(p.fen)
    if (PHASE_ORDER[raw] > PHASE_ORDER[current]) current = raw
    return current
  })
}

/**
 * Índices (ply) onde cada fase termina, para desenhar as faixas do gráfico:
 *  - `openingEnd`: último ply da Abertura (início do Meio-jogo).
 *  - `middlegameEnd`: último ply antes do Final (início do Final).
 * Fases ausentes colapsam: sem Meio-jogo, `middlegameEnd === openingEnd`
 * (faixa do meio com largura 0); sem Final, `middlegameEnd` é o último índice.
 *
 * Para `phases` não-vazio, os limites são sempre ≥ 0: uma partida que comece
 * direto em Meio-jogo/Final (p.ex. PGN com `[FEN]`) colapsa as fases iniciais
 * ausentes pra 0 (faixas de largura 0 na borda esquerda). Vetor vazio devolve
 * o sentinela `{-1, -1}`.
 */
export function phaseBoundaries(phases: Phase[]): {
  openingEnd: number
  middlegameEnd: number
} {
  let openingEnd = -1
  let middlegameEnd = -1
  for (let i = 0; i < phases.length; i++) {
    if (phases[i] === 'opening') openingEnd = i
    if (phases[i] === 'opening' || phases[i] === 'middlegame') middlegameEnd = i
  }
  if (phases.length > 0) {
    openingEnd = Math.max(0, openingEnd)
    middlegameEnd = Math.max(0, middlegameEnd)
  }
  return { openingEnd, middlegameEnd }
}

/**
 * Fase de uma posição isolada a partir de três sinais do tabuleiro:
 *  - Final: poucas peças maiores/menores (≤ ENDGAME_MAX_PIECES).
 *  - Meio-jogo: peças já trocadas (≤ MIDGAME_MAX_PIECES), ou fileira de trás
 *    rala (desenvolvimento), ou peças dos dois lados disputando zonas (mixedness).
 *  - Abertura: caso contrário.
 * Usada para a sequência da partida e para posições isoladas de variações.
 */
export function phaseOfPosition(fen: string): Phase {
  const pieces = majorsAndMinors(fen)
  if (pieces <= ENDGAME_MAX_PIECES) return 'endgame'
  if (
    pieces <= MIDGAME_MAX_PIECES ||
    backrankSparse(fen) ||
    mixedness(fen) > MIXEDNESS_THRESHOLD
  )
    return 'middlegame'
  return 'opening'
}
