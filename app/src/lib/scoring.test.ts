import { describe, expect, it } from 'vitest'
import {
  CLASSIFICATION_LABELS,
  classifyMove,
  cpToWinPct,
  formatEval,
  gameAccuracy,
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

describe('gameAccuracy', () => {
  it('partida perfeita (todos loss 0) tem precisão 100', () => {
    expect(gameAccuracy([0, 0, 0])).toBe(100)
    expect(gameAccuracy([])).toBe(100)
  })

  it('aplica a fórmula chess.com por lance e tira a média', () => {
    expect(gameAccuracy([10])).toBeCloseTo(69.7, 1)
    expect(gameAccuracy([0, 10])).toBeCloseTo(84.9, 1)
    expect(gameAccuracy([30])).toBeLessThan(30)
    expect(gameAccuracy([30])).toBeGreaterThan(0)
  })
})

describe('CLASSIFICATION_LABELS', () => {
  it('mapeia cada classificação ao seu rótulo em pt-BR', () => {
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
