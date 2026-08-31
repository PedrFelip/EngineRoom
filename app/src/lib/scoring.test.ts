import { describe, expect, it } from 'vitest'
import {
  CLASSIFICATION_LABELS,
  centipawnLoss,
  classifyMove,
  cpToWinPct,
  detectBrilliant,
  formatEval,
  gameAccuracy,
  lichessVolatilityWeights,
  moveAccuracy,
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

  it('limita avaliações, inclusive mate, a ±1000 cp', () => {
    expect(cpToWinPct(100000)).toBe(cpToWinPct(1000))
    expect(cpToWinPct(-100000)).toBe(cpToWinPct(-1000))
    expect(cpToWinPct(100000)).toBeCloseTo(97.5, 1)
    expect(cpToWinPct(-100000)).toBeCloseTo(2.5, 1)
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
  const move = (color: 'w' | 'b') => ({ color })
  const initialWinPct = cpToWinPct(15)

  it('partida perfeita tem precisão 100 para ambos os lados', () => {
    expect(
      gameAccuracy(
        [move('w'), move('b'), move('w')],
        [50, initialWinPct, initialWinPct, initialWinPct],
      ),
    ).toEqual({ white: 100, black: 100 })
    expect(gameAccuracy([], [50])).toEqual({ white: 100, black: 100 })
  })

  it('combina média ponderada e harmônica das accuracies por lance', () => {
    const result = gameAccuracy(
      [move('w'), move('b'), move('w')],
      [50, initialWinPct, initialWinPct, initialWinPct - 10],
    )
    const badMove = moveAccuracy(10)
    const weights = lichessVolatilityWeights(
      [initialWinPct, initialWinPct, initialWinPct, initialWinPct - 10],
      3,
    )
    const weighted =
      (100 * weights[0] + badMove * weights[2]) / (weights[0] + weights[2])
    const harmonic = 2 / (1 / 100 + 1 / badMove)

    expect(result.white).toBeCloseTo((weighted + harmonic) / 2, 10)
    expect(result.black).toBe(100)
  })

  it('usa piso 1 na média harmônica quando um lance tem accuracy zero', () => {
    const result = gameAccuracy(
      [move('w'), move('b'), move('w')],
      [50, initialWinPct - 100, initialWinPct - 100, initialWinPct - 100],
    )
    const weights = lichessVolatilityWeights(
      [
        initialWinPct,
        initialWinPct - 100,
        initialWinPct - 100,
        initialWinPct - 100,
      ],
      3,
    )
    const weightedMean = (100 * weights[2]) / (weights[0] + weights[2])
    const harmonicMean = 2 / (1 / 1 + 1 / 100)

    expect(result.white).toBeCloseTo((weightedMean + harmonicMean) / 2, 10)
  })
})

describe('moveAccuracy', () => {
  it('reproduz a curva do Lichess com bônus de incerteza e clamp', () => {
    expect(moveAccuracy(0)).toBe(100)
    expect(moveAccuracy(5)).toBeCloseTo(80.82, 2)
    expect(moveAccuracy(10)).toBeCloseTo(64.58, 2)
    expect(moveAccuracy(20)).toBeCloseTo(41.02, 2)
    expect(moveAccuracy(100)).toBe(0)
  })
})

describe('lichessVolatilityWeights', () => {
  it('usa janelas deslizantes e limita os pesos entre 0,5 e 12', () => {
    expect(lichessVolatilityWeights([50, 50, 80], 2)).toEqual([0.5, 12])
  })

  it('produz um peso para cada lance', () => {
    const winPcts = Array.from({ length: 31 }, (_, index) => 50 + (index % 3))
    expect(lichessVolatilityWeights(winPcts, 30)).toHaveLength(30)
  })
})

describe('centipawnLoss', () => {
  it('compara o melhor score com o lance jogado no mesmo POV', () => {
    expect(centipawnLoss(40, -10)).toBe(30)
    expect(centipawnLoss(20, 80)).toBe(100)
  })

  it('não pune melhora aparente causada por ruído entre buscas', () => {
    expect(centipawnLoss(20, -30)).toBe(0)
  })

  it('preserva uma perda de mate para o limitador da accuracy', () => {
    expect(centipawnLoss(99997, 0)).toBe(99997)
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
