import { describe, it, expect } from 'vitest'
import {
  isUciOk,
  isReadyOk,
  parseIdName,
  parseIdAuthor,
  parseBestMove,
  parseInfo,
  scoreToCp,
} from './uci'

describe('isUciOk / isReadyOk', () => {
  it('detects uciok (trim-tolerant)', () => {
    expect(isUciOk('uciok')).toBe(true)
    expect(isUciOk('  uciok\n')).toBe(true)
    expect(isUciOk('uciokay')).toBe(false)
    expect(isReadyOk('readyok')).toBe(true)
    expect(isReadyOk('uciok')).toBe(false)
  })
})

describe('parseIdName / parseIdAuthor', () => {
  it('extracts the engine name', () => {
    expect(parseIdName('id name Stockfish 18')).toBe('Stockfish 18')
    expect(parseIdName('id author the Stockfish developers')).toBeNull()
    expect(parseIdAuthor('id author the Stockfish developers')).toBe(
      'the Stockfish developers',
    )
  })

  it('returns null for unrelated lines', () => {
    expect(parseIdName('uciok')).toBeNull()
    expect(parseIdName('info depth 1')).toBeNull()
  })
})

describe('parseBestMove', () => {
  it('parses a normal move', () => {
    expect(parseBestMove('bestmove e2e4')).toEqual({ from: 'e2', to: 'e4' })
  })

  it('parses a promotion move', () => {
    expect(parseBestMove('bestmove e7e8q')).toEqual({
      from: 'e7',
      to: 'e8',
      promotion: 'q',
    })
  })

  it('returns null for (none) and malformed lines', () => {
    expect(parseBestMove('bestmove (none)')).toBeNull()
    expect(parseBestMove('info depth 1')).toBeNull()
    expect(parseBestMove('bestmove e2')).toBeNull()
  })

  it('includes the ponder move suffix is ignored (only first token)', () => {
    expect(parseBestMove('bestmove g1f3 ponder e7e5')).toEqual({
      from: 'g1',
      to: 'f3',
    })
  })
})

describe('parseInfo', () => {
  it('parses a rich info line with cp score and pv', () => {
    const line =
      'info depth 20 seldepth 28 multipv 1 score cp 34 nodes 123456 nps 100000 time 1234 pv e2e4 e7e5 g1f3'
    expect(parseInfo(line)).toEqual({
      depth: 20,
      seldepth: 28,
      multipv: 1,
      nodes: 123456,
      nps: 100000,
      time: 1234,
      score: { kind: 'cp', value: 34 },
      pv: ['e2e4', 'e7e5', 'g1f3'],
    })
  })

  it('parses mate scores', () => {
    expect(parseInfo('info depth 12 score mate 3 pv h7h8q')).toEqual({
      depth: 12,
      score: { kind: 'mate', value: 3 },
      pv: ['h7h8q'],
    })
  })

  it('parses negative mate scores', () => {
    expect(parseInfo('info score mate -5')).toEqual({
      score: { kind: 'mate', value: -5 },
    })
  })

  it('handles lowerbound/upperbound flags', () => {
    expect(parseInfo('info depth 1 score cp 30 lowerbound pv a2a3')).toEqual({
      depth: 1,
      score: { kind: 'cp', value: 30, lowerbound: true },
      pv: ['a2a3'],
    })
    expect(parseInfo('info depth 1 score cp 30 upperbound')).toEqual({
      depth: 1,
      score: { kind: 'cp', value: 30, upperbound: true },
    })
  })

  it('returns null for non-info lines', () => {
    expect(parseInfo('uciok')).toBeNull()
    expect(parseInfo('bestmove e2e4')).toBeNull()
  })

  it('ignores free-form info string lines', () => {
    const out = parseInfo('info string some free text with pv e2e4 inside')
    expect(out?.pv).toBeUndefined()
  })
})

describe('scoreToCp', () => {
  it('passes through centipawn scores', () => {
    expect(scoreToCp({ kind: 'cp', value: 42 })).toBe(42)
    expect(scoreToCp({ kind: 'cp', value: -17 })).toBe(-17)
  })

  it('converts mate to a large signed magnitude (closer mate = larger)', () => {
    expect(scoreToCp({ kind: 'mate', value: 1 })).toBe(99999)
    expect(scoreToCp({ kind: 'mate', value: 5 })).toBe(99995)
    expect(scoreToCp({ kind: 'mate', value: -3 })).toBe(-99997)
  })

  it('trata mate 0 como derrota do lado a jogar (posição de xeque-mate)', () => {
    expect(scoreToCp({ kind: 'mate', value: 0 })).toBe(-100000)
  })

  it('returns null when there is no score', () => {
    expect(scoreToCp(undefined)).toBeNull()
  })
})
