import { describe, expect, it } from 'vitest'
import {
  backrankSparse,
  computePhases,
  majorsAndMinors,
  mixedness,
  type Phase,
  phaseBoundaries,
  phaseOfPosition,
  regionScore,
} from './phase'

describe('majorsAndMinors', () => {
  it('posição inicial tem 14 peças maiores/menores (4N 4B 4R 2Q)', () => {
    const start = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
    expect(majorsAndMinors(start)).toBe(14)
  })

  it('apenas reis conta 0 (rei e peão não entram)', () => {
    expect(majorsAndMinors('4k3/8/8/8/8/8/8/4K3 w - - 0 1')).toBe(0)
  })

  it('uma torre conta 1', () => {
    expect(majorsAndMinors('8/8/8/4k3/8/8/8/4K2R w - - 0 1')).toBe(1)
  })

  it('duas damas (uma promovida) conta 2', () => {
    expect(majorsAndMinors('4k3/8/8/8/8/8/8/3KQq2 w - - 0 1')).toBe(2)
  })
})

describe('backrankSparse', () => {
  it('posição inicial não é esparsa (fileiras de trás cheias)', () => {
    const start = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
    expect(backrankSparse(start)).toBe(false)
  })

  it('brancas com 3 peças na 1ª fileira (desenvolvimento + roque) é esparso', () => {
    // rank1 = R,4 vazios,R(f1),K(g1),1 vazio = 3 peças
    expect(
      backrankSparse('rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/R4RK1 w k - 0 1'),
    ).toBe(true)
  })

  it('pretas com 3 peças na 8ª fileira também é esparso', () => {
    expect(
      backrankSparse('r4rk1/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w K - 0 1'),
    ).toBe(true)
  })

  it('limiar exclusivo: 4 peças na fileira NÃO é esparso', () => {
    // rank1 = R,2 vazios,Q,K,2 vazios,R = 4 peças
    expect(
      backrankSparse('rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/R2QK2R w KQk - 0 1'),
    ).toBe(false)
  })
})

describe('regionScore', () => {
  // Valores esperados derivados da tabela de referência (spec), não recomputados.
  it('white 0, black 1 → 1 + y', () => {
    expect(regionScore(1, 0, 1)).toBe(2)
  })

  it('white 0, black 2 respeita o limite y < 6 (y=6 vira 0)', () => {
    expect(regionScore(3, 0, 2)).toBe(5) // 2 + (6-3)
    expect(regionScore(6, 0, 2)).toBe(0)
  })

  it('white 0, black 3 → 3 + (7-y) quando y < 7', () => {
    expect(regionScore(4, 0, 3)).toBe(6) // 3 + (7-4)
  })

  it('white 1, black 0 → 1 + (8-y)', () => {
    expect(regionScore(1, 1, 0)).toBe(8)
  })

  it('white 1, black 1 → 5 + |4-y|', () => {
    expect(regionScore(3, 1, 1)).toBe(6) // 5 + 1
    expect(regionScore(4, 1, 1)).toBe(5) // 5 + 0
  })

  it('white 2, black 0 respeita o limite y > 2 (y=2 vira 0)', () => {
    expect(regionScore(4, 2, 0)).toBe(4) // 2 + (4-2)
    expect(regionScore(2, 2, 0)).toBe(0)
  })

  it('white 2, black 1 → 4 + (y-1)', () => {
    expect(regionScore(4, 2, 1)).toBe(7) // 4 + 3
  })

  it('white 2, black 2 → 7 (constante)', () => {
    expect(regionScore(3, 2, 2)).toBe(7)
  })

  it('white 3, black 0 → 3 + (y-1) quando y > 1', () => {
    expect(regionScore(4, 3, 0)).toBe(6) // 3 + 3
  })

  it('white 3, black 1 → 5 + (y-1)', () => {
    expect(regionScore(5, 3, 1)).toBe(9) // 5 + 4
  })

  it('white 4, black 0 → 3 + (y-1) quando y > 1', () => {
    expect(regionScore(4, 4, 0)).toBe(6)
  })

  it('contagens fora da tabela viram 0', () => {
    expect(regionScore(3, 0, 0)).toBe(0) // nenhum lado
    expect(regionScore(4, 5, 1)).toBe(0) // white > 4
    expect(regionScore(4, 1, 4)).toBe(0) // black fora do caso white=1
  })
})

describe('mixedness', () => {
  it('posição inicial: peças separadas, sem conflito de região → 0', () => {
    const start = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
    expect(mixedness(start)).toBe(0)
  })

  it('tabuleiro com peças brancas e pretas entrelaçadas no centro ultrapassa 150', () => {
    // peões alternados em fileiras adjacentes geram alta interação por região
    const clashing =
      '7k/pppppppp/pppppppp/PPPPPPPP/pppppppp/PPPPPPPP/PPPPPPPP/K7 w - - 0 1'
    expect(mixedness(clashing)).toBeGreaterThan(150)
  })
})

describe('phaseOfPosition', () => {
  const start = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'

  it('posição inicial é Abertura (14 peças, fileiras cheias, sem mistura)', () => {
    expect(phaseOfPosition(start)).toBe('opening')
  })

  it('poucas peças (≤ 6) é Final', () => {
    // K+R vs K: 1 peça maior/menor
    expect(phaseOfPosition('8/4k3/8/8/8/8/8/4K2R w - - 0 1')).toBe('endgame')
  })

  it('contagem entre 7 e 10 é Meio-jogo (mesmo sem desenvolvimento)', () => {
    // 8 peças maiores/menores, fileiras de trás com ≥4 (não esparsas), sem mistura
    expect(
      phaseOfPosition('r1b1k1nr/pppppppp/8/8/8/8/PPPPPPPP/R1B1K1NR w - - 0 1'),
    ).toBe('middlegame')
  })

  it('material cheio (14) mas fileira de trás rala é Meio-jogo (desenvolvimento)', () => {
    // 14 peças, mas brancas com 3 na 1ª fileira (roque + desenvolvimento)
    expect(
      phaseOfPosition(
        'rnbqkbnr/pppppppp/8/8/8/1N1BQBN1/PPPPPPPP/R4RK1 w KQ - 0 1',
      ),
    ).toBe('middlegame')
  })
})

describe('computePhases', () => {
  // FENs de referência (sem ply — a fase vem só do tabuleiro):
  //  start = 14 peças, sem desenvolvimento → Abertura
  //  mid8  = 8 peças maiores/menores, fileiras de trás cheias → Meio-jogo (contagem)
  //  endKR = 1 peça maior (K+R vs K) → Final
  const start = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
  const mid8 = 'r1b1k1nr/pppppppp/8/8/8/8/PPPPPPPP/R1B1K1NR w - - 0 1'
  const endKR = '8/4k3/8/8/8/8/8/4K2R w - - 0 1'

  it('arco natural Abertura → Meio-jogo → Final', () => {
    expect(
      computePhases([{ fen: start }, { fen: mid8 }, { fen: endKR }]),
    ).toEqual(['opening', 'middlegame', 'endgame'])
  })

  it('não regredir: atingido o Final, posição com mais peças permanece Final', () => {
    // endKR(raw Final) → mid8(raw Meio-jogo, 8 peças): a fase não pode voltar
    expect(computePhases([{ fen: endKR }, { fen: mid8 }])).toEqual([
      'endgame',
      'endgame',
    ])
  })

  it('vetor vazio retorna vazio', () => {
    expect(computePhases([])).toEqual([])
  })
})

describe('phaseBoundaries', () => {
  it('arco completo: último ply de cada fase', () => {
    const p: Phase[] = ['opening', 'middlegame', 'endgame']
    expect(phaseBoundaries(p)).toEqual({ openingEnd: 0, middlegameEnd: 1 })
  })

  it('várias posições por fase', () => {
    const p: Phase[] = [
      'opening',
      'opening',
      'middlegame',
      'middlegame',
      'endgame',
      'endgame',
    ]
    expect(phaseBoundaries(p)).toEqual({ openingEnd: 1, middlegameEnd: 3 })
  })

  it('sem Final: middlegameEnd é o último índice', () => {
    const p: Phase[] = ['opening', 'opening', 'middlegame', 'middlegame']
    expect(phaseBoundaries(p)).toEqual({ openingEnd: 1, middlegameEnd: 3 })
  })

  it('sem Meio-jogo: faixa do meio fica com largura 0', () => {
    const p: Phase[] = ['opening', 'opening', 'endgame', 'endgame']
    expect(phaseBoundaries(p)).toEqual({ openingEnd: 1, middlegameEnd: 1 })
  })
})
