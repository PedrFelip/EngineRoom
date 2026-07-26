import { describe, expect, it } from 'vitest'
import {
  computePhases,
  nonPawnMaterial,
  type Phase,
  phaseBoundaries,
  phaseOf,
  phaseOfMaterial,
} from './phase'

describe('phaseOfMaterial', () => {
  it('material máximo (62) é Abertura', () => {
    expect(phaseOfMaterial(62)).toBe('opening')
  })

  it('limiar inferior da Abertura (50) é inclusivo — Abertura', () => {
    expect(phaseOfMaterial(50)).toBe('opening')
  })

  it('logo abaixo do limiar (49) é Meio-jogo', () => {
    expect(phaseOfMaterial(49)).toBe('middlegame')
  })

  it('logo acima do Final (25) é Meio-jogo', () => {
    expect(phaseOfMaterial(25)).toBe('middlegame')
  })

  it('limiar superior do Final (24) é inclusivo — Final', () => {
    expect(phaseOfMaterial(24)).toBe('endgame')
  })

  it('material zero é Final', () => {
    expect(phaseOfMaterial(0)).toBe('endgame')
  })
})

describe('phaseOf (material + número do lance)', () => {
  it('início da partida com material cheio é Abertura', () => {
    expect(phaseOf(0, 62)).toBe('opening')
  })

  it('material cheio até o ply 20 (10 lances completos) ainda é Abertura', () => {
    expect(phaseOf(20, 62)).toBe('opening')
  })

  it('material cheio APÓS o lance 10 vira Meio-jogo (sinal do número do lance)', () => {
    // antes dessa mudança, material 62 era sempre Abertura
    expect(phaseOf(21, 62)).toBe('middlegame')
    expect(phaseOf(40, 62)).toBe('middlegame')
  })

  it('material de meio-jogo no meio da partida é Meio-jogo', () => {
    expect(phaseOf(40, 40)).toBe('middlegame')
  })

  it('material baixo vira Final independente do número do lance', () => {
    expect(phaseOf(40, 24)).toBe('endgame')
    // mesmo cedo, se o material já é de final, é final
    expect(phaseOf(10, 20)).toBe('endgame')
  })

  it('prioridade: material de final vence sobre número do lance cedo', () => {
    expect(phaseOf(5, 10)).toBe('endgame')
  })
})

describe('nonPawnMaterial', () => {
  it('posição inicial soma 62 (4N 4B 4T 2D na escala Reinfeld)', () => {
    const start = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
    // 4·3 (cavalos) + 4·3 (bispos) + 4·5 (torres) + 2·9 (damas) = 62
    expect(nonPawnMaterial(start)).toBe(62)
  })

  it('apenas reis soma 0', () => {
    expect(nonPawnMaterial('4k3/8/8/8/8/8/8/4K3 w - - 0 1')).toBe(0)
  })

  it('uma torre soma 5 (Reinfeld)', () => {
    expect(nonPawnMaterial('8/8/8/4k3/8/8/8/4K2R w - - 0 1')).toBe(5)
  })
})

describe('computePhases', () => {
  // FENs de referência: start = material cheio (62); end = 5 (Final, uma torre);
  // highAfter = 31 (raw Meio-jogo). Cada posição carrega seu ply absoluto.
  const start = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
  const end = '8/4k3/8/8/8/8/8/4K2R w - - 0 1'
  const highAfter = 'r2qk3/8/8/8/8/8/8/R1BQK3 w - - 0 1'

  it('mesma posição, ply diferente: material cheio vira Meio-jogo após o lance 10', () => {
    // o NÚMERO DO LANCE decide a Abertura, não o material
    expect(
      computePhases([
        { fen: start, ply: 20 },
        { fen: start, ply: 21 },
      ]),
    ).toEqual(['opening', 'middlegame'])
  })

  it('arco natural Abertura → Meio-jogo → Final', () => {
    expect(
      computePhases([
        { fen: start, ply: 0 },
        { fen: start, ply: 25 },
        { fen: end, ply: 40 },
      ]),
    ).toEqual(['opening', 'middlegame', 'endgame'])
  })

  it('não regredir: atingido o Final, material maior permanece Final', () => {
    // end(raw Final, ply 40) → highAfter(raw Meio-jogo, ply 41): a fase não volta
    expect(
      computePhases([
        { fen: end, ply: 40 },
        { fen: highAfter, ply: 41 },
      ]),
    ).toEqual(['endgame', 'endgame'])
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
