import { describe, expect, it } from 'vitest'
import type { Variation, VariationMap, VariationMove } from '../types'
import type { RawPosition } from './analyze'
import { cpToWinPct } from './scoring'
import {
  applyLiveToVariation,
  applyLiveToVariationMove,
  classifyVariationMove,
  decideUserMove,
} from './variations'

describe('classifyVariationMove', () => {
  it('um lance que entrega muita vantagem é Blunder', () => {
    // Posição igual (cp 0 do POV de quem joga) e o adversário passa a ver +10
    // (cp 1000 do POV do lado a jogar após o lance).
    const result = classifyVariationMove(0, 1000)

    expect(result.classification).toBe('blunder')
  })

  it('um lance que mantém a vantagem (igual ao melhor) é Melhor', () => {
    // +1 para quem joga; o adversário passa a ver -1 (simétrico) → sem perda.
    const result = classifyVariationMove(100, -100)

    expect(result.classification).toBe('melhor')
    expect(result.winPctLoss).toBe(0)
  })

  it('normaliza winPctBefore/After pelo POV do lado que jogou (espelha buildReview)', () => {
    // winPctBefore = cpToWinPct(beforeCp); winPctAfter inverte o POV do adversário.
    const beforeCp = 200
    const afterCp = -150
    const result = classifyVariationMove(beforeCp, afterCp)

    expect(result.winPctBefore).toBeCloseTo(cpToWinPct(beforeCp), 7)
    expect(result.winPctAfter).toBeCloseTo(100 - cpToWinPct(afterCp), 7)
    expect(result.winPctLoss).toBeCloseTo(
      Math.max(0, cpToWinPct(beforeCp) - (100 - cpToWinPct(afterCp))),
      7,
    )
  })
})

describe('applyLiveToVariationMove', () => {
  function pendingMove(overrides: Partial<VariationMove> = {}): VariationMove {
    return {
      id: 'm1',
      ply: 1,
      color: 'w',
      san: 'e4',
      uci: 'e2e4',
      fenBefore: 'startpos-fen',
      fenAfter: 'after-e4-fen',
      ...overrides,
    }
  }

  it('preenche a classificação e afterCp a partir de um live-eval num lance pendente', () => {
    const move = pendingMove({ color: 'w' })
    // Brancas jogaram um blunder: adversário (pretas, a jogar) vê +10.
    const raw: RawPosition = {
      fen: 'after-e4-fen',
      cp: 1000,
      depth: 20,
      pv: ['e7e5'],
      lines: [{ multipv: 1, cp: 1000, pv: ['e7e5'] }],
    }

    const result = applyLiveToVariationMove(move, raw, 0)

    expect(result.afterCp).toBe(1000)
    expect(result.classification).toBe('blunder')
    expect(result.bestUci).toBe('e7e5')
    expect(result.depth).toBe(20)
  })

  it('normaliza as linhas candidatas para o POV das brancas (lance das brancas)', () => {
    const move = pendingMove({ color: 'w' })
    // Após lance das brancas, pretas a jogar: cp da engine é POV das pretas.
    const raw: RawPosition = {
      fen: 'f',
      cp: 100,
      depth: 15,
      pv: ['e7e5'],
      lines: [{ multipv: 1, cp: 100, pv: ['e7e5'] }],
    }

    const result = applyLiveToVariationMove(move, raw, 0)

    expect(result.lines?.[0]?.cp).toBe(-100) // flipado para POV das brancas
    expect(result.lines?.[0]?.winPct).toBeCloseTo(100 - cpToWinPct(100), 7)
  })

  it('refino progressivo: reaplicar com eval mais profundo atualiza depth sem acumular estado', () => {
    const move = pendingMove({ color: 'w' })
    const shallow: RawPosition = {
      fen: 'f',
      cp: 1000,
      depth: 12,
      pv: ['e7e5'],
      lines: [{ multipv: 1, cp: 1000, pv: ['e7e5'] }],
    }
    const deeper: RawPosition = {
      fen: 'f',
      cp: 900,
      depth: 28,
      pv: ['c7c5'],
      lines: [{ multipv: 1, cp: 900, pv: ['c7c5'] }],
    }

    const once = applyLiveToVariationMove(move, shallow, 0)
    const twice = applyLiveToVariationMove(once, deeper, 0)

    expect(twice.depth).toBe(28)
    expect(twice.afterCp).toBe(900)
    expect(twice.bestUci).toBe('c7c5')
    expect(twice.classification).toBe('blunder')
    expect(twice.lines).toHaveLength(1)
  })
})

describe('decideUserMove', () => {
  it('lance diferente do próximo da linha principal abre variação no ply atual', () => {
    // Usuário está no ply 2 (após 1...e5) e joga Cf6 em vez de Cf3 (linha principal).
    const decision = decideUserMove('g8f6', 2, 'f1c4')

    expect(decision).toEqual({ kind: 'variation', parentPly: 2 })
  })

  it('lance igual ao próximo da linha principal apenas avança', () => {
    const decision = decideUserMove('f1c4', 2, 'f1c4')

    expect(decision).toEqual({ kind: 'advance' })
  })

  it('no fim da linha principal (sem próximo lance), qualquer lance abre variação', () => {
    const decision = decideUserMove('d2d4', 40, null)

    expect(decision).toEqual({ kind: 'variation', parentPly: 40 })
  })
})

describe('applyLiveToVariation', () => {
  function vmap(variation: Variation): VariationMap {
    return { [variation.parentPly]: [variation] }
  }

  function vmove(overrides: Partial<VariationMove> = {}): VariationMove {
    return {
      id: 'm1',
      ply: 1,
      color: 'w',
      san: 'd4',
      uci: 'd2d4',
      fenBefore: 'fb',
      fenAfter: 'fa',
      ...overrides,
    }
  }

  function variation(overrides: Partial<Variation> = {}): Variation {
    return { id: 'v1', parentPly: 2, moves: [vmove()], ...overrides }
  }

  it('atualiza a análise do lance-alvo no mapa de variações', () => {
    const map = vmap(variation())
    const raw: RawPosition = { fen: 'fa', cp: 1000, depth: 20, pv: ['e7e5'] }
    const beforeCp = () => 0

    const result = applyLiveToVariation(
      map,
      { variationId: 'v1', moveId: 'm1' },
      raw,
      beforeCp,
    )

    expect(result[2][0].moves[0].classification).toBe('blunder')
    expect(result[2][0].moves[0].afterCp).toBe(1000)
  })

  it('usa afterCp do lance anterior como beforeCp para lances além do primeiro', () => {
    const map = vmap(
      variation({
        moves: [
          vmove({ id: 'm1', ply: 1, color: 'w', afterCp: -50 }),
          vmove({ id: 'm2', ply: 2, color: 'b', san: 'e5', uci: 'e7e5' }),
        ],
      }),
    )
    const raw: RawPosition = { fen: 'f', cp: 200, depth: 18, pv: ['g1f3'] }

    const result = applyLiveToVariation(
      map,
      { variationId: 'v1', moveId: 'm2' },
      raw,
      // resolução real: ply>1 busca o afterCp do lance anterior
      (v, m) => (m.ply === 1 ? 0 : v.moves[m.ply - 2]?.afterCp),
    )

    // beforeCp veio do afterCp de m1 (-50); classificação bate com a fórmula.
    expect(result[2][0].moves[1].winPctBefore).toBeCloseTo(cpToWinPct(-50), 7)
    expect(result[2][0].moves[1].classification).toBe(
      classifyVariationMove(-50, 200).classification,
    )
  })

  it('mantém o lance pendente (sem alterar o mapa) quando beforeCp é indefinido', () => {
    const map = vmap(
      variation({
        moves: [
          vmove({ id: 'm1', ply: 1, color: 'w' /* sem afterCp */ }),
          vmove({ id: 'm2', ply: 2, color: 'b' }),
        ],
      }),
    )
    const raw: RawPosition = { fen: 'f', cp: 0, depth: 10, pv: [] }

    const result = applyLiveToVariation(
      map,
      { variationId: 'v1', moveId: 'm2' },
      raw,
      (v, m) => (m.ply === 1 ? 0 : v.moves[m.ply - 2]?.afterCp),
    )

    expect(result).toBe(map) // mesma referência — nada mudou
    expect(result[2][0].moves[1].classification).toBeUndefined()
  })

  it('não quebra quando o alvo não existe no mapa', () => {
    const map = vmap(variation())
    const raw: RawPosition = { fen: 'f', cp: 0, depth: 10, pv: [] }

    const result = applyLiveToVariation(
      map,
      { variationId: 'inexistente', moveId: 'x' },
      raw,
      () => 0,
    )

    expect(result).toBe(map)
  })
})
