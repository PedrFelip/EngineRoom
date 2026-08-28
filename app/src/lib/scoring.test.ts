import { describe, expect, it } from 'vitest'
import {
  CLASSIFICATION_LABELS,
  classifyMove,
  cpToWinPct,
  detectBrilliant,
  formatEval,
  gameAccuracy,
  sideToMoveAtPly,
  whiteCp,
  whiteWinPct,
} from './scoring'

describe('cpToWinPct', () => {
  it('retorna 50% em posição igual (cp = 0)', () => {
    expect(cpToWinPct(0)).toBe(50)
  })

  it('cp positivo favorece o lado a jogar e cresce monotonicamente', () => {
    expect(cpToWinPct(100)).toBeGreaterThan(50)
    expect(cpToWinPct(500)).toBeGreaterThan(cpToWinPct(100))
    expect(cpToWinPct(1000)).toBeGreaterThan(cpToWinPct(500))
    expect(cpToWinPct(1000)).toBeLessThan(100)
  })

  it('cp negativo desfavorece o lado a jogar e é simétrico a +cp', () => {
    expect(cpToWinPct(-100)).toBeLessThan(50)
    expect(cpToWinPct(-500)).toBeLessThan(cpToWinPct(-100))
    expect(cpToWinPct(-1000)).toBeGreaterThan(0)
    expect(cpToWinPct(150) + cpToWinPct(-150)).toBeCloseTo(100, 5)
  })

  it('satura em ~100% / ~0% para avaliações de xeque-mate', () => {
    expect(cpToWinPct(100000)).toBeCloseTo(100, 0)
    expect(cpToWinPct(-100000)).toBeCloseTo(0, 0)
  })
})

describe('whiteCp', () => {
  it('mantém o sinal quando as brancas jogam', () => {
    expect(whiteCp(50, 'w')).toBe(50)
    expect(whiteCp(-30, 'w')).toBe(-30)
  })

  it('inverte o sinal quando as pretas jogam', () => {
    expect(whiteCp(50, 'b')).toBe(-50)
    expect(whiteCp(-30, 'b')).toBe(30)
  })

  it('preserva a sentinela de mate (sem arredondar/clamp)', () => {
    expect(whiteCp(99995, 'b')).toBe(-99995)
    expect(whiteCp(-99997, 'b')).toBe(99997)
  })
})

describe('whiteWinPct', () => {
  it('é 50% em cp 0 independente do lado a jogar', () => {
    expect(whiteWinPct(0, 'w')).toBe(50)
    expect(whiteWinPct(0, 'b')).toBe(50)
  })

  it('cp positivo favorece as brancas em qualquer POV', () => {
    expect(whiteWinPct(500, 'w')).toBeGreaterThan(50)
    // cp 500 POV das pretas = brancas em desvantagem
    expect(whiteWinPct(500, 'b')).toBeLessThan(50)
  })

  it('é simétrico: whiteWinPct(cp, b) ≈ 100 - whiteWinPct(cp, w)', () => {
    expect(whiteWinPct(200, 'b')).toBeCloseTo(100 - whiteWinPct(200, 'w'), 5)
  })
})

describe('sideToMoveAtPly', () => {
  const moves = [
    { color: 'w' as const },
    { color: 'b' as const },
    { color: 'w' as const },
    { color: 'b' as const },
  ]

  it('posição inicial (ply 0) = brancas', () => {
    expect(sideToMoveAtPly(moves, 0)).toBe('w')
  })

  it('próximo lance define o lado a jogar', () => {
    expect(sideToMoveAtPly(moves, 1)).toBe('b')
    expect(sideToMoveAtPly(moves, 2)).toBe('w')
  })

  it('posição final = oposto do último lance', () => {
    expect(sideToMoveAtPly(moves, 4)).toBe('w')
  })

  it('lista vazia devolve brancas (posição inicial)', () => {
    expect(sideToMoveAtPly([], 0)).toBe('w')
  })
})

describe('classifyMove', () => {
  it('lance igual ao melhor (loss 0) é Melhor', () => {
    expect(classifyMove(0)).toBe('melhor')
  })

  it('perda minúscula de win% é Excelente', () => {
    expect(classifyMove(1.5)).toBe('excelente')
  })

  it('perda pequena de win% é Bom', () => {
    expect(classifyMove(4)).toBe('bom')
  })

  it('perda média de win% é Imprecisão', () => {
    expect(classifyMove(8)).toBe('imprecisao')
  })

  it('perda grande de win% é Erro', () => {
    expect(classifyMove(15)).toBe('erro')
  })

  it('perda enorme de win% é Blunder', () => {
    expect(classifyMove(30)).toBe('blunder')
  })

  it('lance de abertura (isBook) é Livro, mesmo com perda alta', () => {
    expect(classifyMove(30, true)).toBe('livro')
  })
})

describe('detectBrilliant', () => {
  const ok = {
    winPctLoss: 0,
    winPctBefore: 52,
    winPctAfter: 50,
    materialDelta: -2,
    hasSecondLine: false,
  }

  it('aceita o melhor lance com sacrifício de 2 peões', () => {
    expect(detectBrilliant(ok)).toBe(true)
  })

  it('exige sacrifício de pelo menos 2 peões', () => {
    expect(detectBrilliant({ ...ok, materialDelta: -1.99 })).toBe(false)
    expect(detectBrilliant({ ...ok, materialDelta: 0 })).toBe(false)
  })

  it('rejeita quem fica em posição ruim depois do lance', () => {
    expect(detectBrilliant({ ...ok, winPctAfter: 34.9 })).toBe(false)
    expect(detectBrilliant({ ...ok, winPctAfter: 35 })).toBe(true)
  })

  it('rejeita quem já partiu de vitória esmagadora', () => {
    expect(detectBrilliant({ ...ok, winPctBefore: 85 })).toBe(true)
    expect(detectBrilliant({ ...ok, winPctBefore: 85.1 })).toBe(false)
  })

  it('exige o lance exato sem 2ª linha; com ela tolera quase-melhor', () => {
    expect(detectBrilliant({ ...ok, winPctLoss: 0.1 })).toBe(false)
    expect(
      detectBrilliant({ ...ok, winPctLoss: 0.5, hasSecondLine: true }),
    ).toBe(true)
    expect(
      detectBrilliant({ ...ok, winPctLoss: 0.6, hasSecondLine: true }),
    ).toBe(false)
  })
})

describe('gameAccuracy', () => {
  it('partida perfeita (todos loss 0) tem precisão 100', () => {
    expect(gameAccuracy([0, 0, 0])).toBe(100)
    expect(gameAccuracy([])).toBe(100)
  })

  it('calibra com a fórmula do Lichess (103.1668·exp(-0.04354·loss) - 3)', () => {
    expect(gameAccuracy([10])).toBeCloseTo(63.7, 1)
    expect(gameAccuracy([0, 10])).toBeCloseTo(80.0, 1)
    expect(gameAccuracy([30])).toBeCloseTo(24.9, 1)
  })
})

describe('CLASSIFICATION_LABELS', () => {
  it('mapeia cada classificação ao seu rótulo em pt-BR', () => {
    expect(CLASSIFICATION_LABELS.brilhante).toBe('Brilhante')
    expect(CLASSIFICATION_LABELS.melhor).toBe('Melhor')
    expect(CLASSIFICATION_LABELS.excelente).toBe('Excelente')
    expect(CLASSIFICATION_LABELS.bom).toBe('Bom')
    expect(CLASSIFICATION_LABELS.imprecisao).toBe('Imprecisão')
    expect(CLASSIFICATION_LABELS.erro).toBe('Erro')
    expect(CLASSIFICATION_LABELS.blunder).toBe('Blunder')
    expect(CLASSIFICATION_LABELS.livro).toBe('Livro')
  })
})

describe('formatEval', () => {
  it('formata centipawns em peões com sinal', () => {
    expect(formatEval(0)).toBe('+0.00')
    expect(formatEval(120)).toBe('+1.20')
    expect(formatEval(-50)).toBe('-0.50')
    expect(formatEval(1000)).toBe('+10.00')
  })

  it('formata mate em N (POV brancas)', () => {
    expect(formatEval(99999)).toBe('#1')
    expect(formatEval(99997)).toBe('#3')
    expect(formatEval(-99997)).toBe('-#3')
  })
})
