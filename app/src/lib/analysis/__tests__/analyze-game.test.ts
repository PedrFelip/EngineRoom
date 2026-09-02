import { describe, expect, it } from 'vitest'
import { analyzeGame, type EnginePort, type RawLine } from '../../analyze'
import {
  allHitsCache,
  fakeCache,
  fakePort,
  START_FEN,
} from './analyze-test-helpers'

describe('analyzeGame', () => {
  it('aciona o engine por ply e devolve a revisão', async () => {
    const port = fakePort(() => ({ cp: 0, pv: ['e2e4'] }))
    const review = await analyzeGame(
      '1. e4 e5',
      { mode: 'depth', depth: 20 },
      port,
    )

    expect(review.positions).toHaveLength(3)
    expect(review.moves).toHaveLength(2)
    expect(review.positions.every((p) => Math.abs(p.winPct - 50) < 0.1)).toBe(
      true,
    )
    expect(review.moves.every((m) => m.winPctLoss === 0)).toBe(true)
    expect(review.accuracy.white).toBeCloseTo(95, 0)
    expect(review.accuracy.black).toBe(100)
  })

  it('propaga a avaliação por FEN até a perda de win% do lance', async () => {
    const port = fakePort((fen) =>
      fen === START_FEN ? { cp: 0, pv: ['e2e4'] } : { cp: 500, pv: ['d8h4'] },
    )
    const review = await analyzeGame(
      '1. e4',
      { mode: 'depth', depth: 20 },
      port,
    )

    expect(review.moves).toHaveLength(1)
    expect(review.moves[0].winPctLoss).toBeCloseTo(36.3, 1)
    expect(review.moves[0].bestUci).toBe('e2e4')
    expect(review.moves[0].classification).toBe('livro')
  })

  it('coleta múltiplas linhas candidatas quando multipv > 1', async () => {
    let lineCb: ((line: string) => void) | null = null
    const port: EnginePort = {
      send(cmd: string) {
        const c = cmd.trim()
        if (c === 'uci') lineCb?.('uciok')
        else if (c === 'isready') lineCb?.('readyok')
        else if (c.startsWith('go')) {
          lineCb?.('info depth 20 multipv 1 score cp 30 pv e2e4 e7e5')
          lineCb?.('info depth 20 multipv 2 score cp 10 pv d2d4 d7d5')
          lineCb?.('bestmove e2e4')
        }
      },
      onLine(handler: (line: string) => void) {
        lineCb = handler
        return () => {
          lineCb = null
        }
      },
    }

    const review = await analyzeGame(
      '1. e4 e5',
      { mode: 'depth', depth: 20 },
      port,
      2,
    )
    const lines0 = review.positions[0].lines

    expect(lines0).toHaveLength(2)
    expect(lines0[0].multipv).toBe(1)
    expect(lines0[0].san).toBe('e4')
    expect(lines0[1].multipv).toBe(2)
    expect(lines0[1].san).toBe('d4')
    expect(lines0[0].winPct).toBeGreaterThan(lines0[1].winPct)
  })

  it('resolve xeque-mate deterministicamente (sem depender da engine)', async () => {
    const port = fakePort(() => ({ cp: 0, pv: [] }))
    const review = await analyzeGame(
      '1. f3 e5 2. g4 Qh4#',
      { mode: 'depth', depth: 20 },
      port,
    )

    const last = review.positions[review.positions.length - 1]
    expect(last.winPct).toBeCloseTo(2.5, 1)

    const mateMove = review.moves[review.moves.length - 1]
    expect(mateMove.winPctLoss).toBe(0)
  })

  it('usa o cache de posições em vez de acionar o engine', async () => {
    let gos = 0
    let lineCb: ((line: string) => void) | null = null
    const port: EnginePort = {
      send(cmd: string) {
        const c = cmd.trim()
        if (c === 'uci') lineCb?.('uciok')
        else if (c === 'isready') lineCb?.('readyok')
        else if (c.startsWith('go')) {
          gos++
          lineCb?.('bestmove e2e4')
        }
      },
      onLine(handler: (line: string) => void) {
        lineCb = handler
        return () => {
          lineCb = null
        }
      },
    }
    const cache = allHitsCache()

    const review = await analyzeGame(
      '1. e4 e5',
      { mode: 'depth', depth: 20 },
      port,
      1,
      { cache },
    )
    expect(gos).toBe(0)
    expect(review.moves).toHaveLength(2)
    expect(review.moves.every((m) => m.winPctLoss === 0)).toBe(true)
  })

  it('avalia no engine e grava no cache a posição ainda não cacheada', async () => {
    const port = fakePort(() => ({ cp: 0, pv: ['e2e4'] }))
    const gravadas: {
      fen: string
      mode: string
      value: number
      multipv: number
    }[] = []
    const cache = fakeCache({
      async putMany(entries, mode, value, multipv) {
        for (const pos of entries) {
          gravadas.push({ fen: pos.fen, mode, value, multipv })
        }
      },
    })

    const review = await analyzeGame(
      '1. e4 e5',
      { mode: 'depth', depth: 18 },
      port,
      2,
      { cache },
    )

    expect(gravadas).toHaveLength(3)
    expect(gravadas.map((g) => g.fen)).toEqual([
      START_FEN,
      'rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1',
      'rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2',
    ])
    expect(
      gravadas.every(
        (g) => g.mode === 'depth' && g.value === 18 && g.multipv === 2,
      ),
    ).toBe(true)
    expect(review.moves).toHaveLength(2)
  })

  it('grava depth por linha na RawPosition que vai para o cache', async () => {
    let lineCb: ((line: string) => void) | null = null
    const port: EnginePort = {
      send(cmd) {
        const c = cmd.trim()
        if (c === 'uci') lineCb?.('uciok')
        else if (c === 'isready') lineCb?.('readyok')
        else if (c.startsWith('go')) {
          lineCb?.('info depth 28 multipv 1 score cp 35 pv e2e4 e7e5')
          lineCb?.('info depth 27 multipv 2 score cp 30 pv d2d4 d7d5')
          lineCb?.('bestmove e2e4')
        }
      },
      onLine(handler) {
        lineCb = handler
        return () => {
          lineCb = null
        }
      },
    }
    let captured: { lines: RawLine[]; depth: number } | null = null
    const cache = fakeCache({
      async putMany(entries) {
        const last = entries[entries.length - 1]
        if (last) captured = { lines: last.lines ?? [], depth: last.depth }
      },
    })

    await analyzeGame('1. e4', { mode: 'depth', depth: 20 }, port, 2, { cache })

    expect(captured).not.toBeNull()
    expect(captured?.lines[0].depth).toBe(28)
    expect(captured?.lines[1].depth).toBe(27)
    expect(captured?.depth).toBe(28)
  })

  it('em modo tempo envia `go movetime N` para a engine (nunca `go depth`)', async () => {
    const sent: string[] = []
    let lineCb: ((line: string) => void) | null = null
    const port: EnginePort = {
      send(cmd) {
        sent.push(cmd.trim())
        const c = cmd.trim()
        if (c === 'uci') lineCb?.('uciok')
        else if (c === 'isready') lineCb?.('readyok')
        else if (c.startsWith('go')) {
          lineCb?.('info depth 28 multipv 1 score cp 0 pv e2e4 e7e5')
          lineCb?.('bestmove e2e4')
        }
      },
      onLine(handler) {
        lineCb = handler
        return () => {
          lineCb = null
        }
      },
    }

    const review = await analyzeGame(
      '1. e4 e5',
      { mode: 'time', movetimeMs: 1000 },
      port,
    )

    expect(sent).toContain('go movetime 1000')
    expect(sent.some((c) => c.startsWith('go depth'))).toBe(false)
    expect(review.moves).toHaveLength(2)
    expect(review.accuracy.white).toBeCloseTo(95, 0)
    expect(review.accuracy.black).toBe(100)
  })

  it('em modo tempo consulta o cache com mode="time" e a chave movetimeMs', async () => {
    const gets: Array<{
      fen: string
      mode: string
      value: number
      multipv: number
    }> = []
    let lineCb: ((line: string) => void) | null = null
    const port: EnginePort = {
      send(cmd) {
        const c = cmd.trim()
        if (c === 'uci') lineCb?.('uciok')
        else if (c === 'isready') lineCb?.('readyok')
        else if (c.startsWith('go'))
          throw new Error('cache hit não deveria acionar a engine')
      },
      onLine(handler) {
        lineCb = handler
        return () => {
          lineCb = null
        }
      },
    }
    const hit = (fen: string) => ({
      fen,
      cp: 0,
      depth: 28,
      pv: ['e2e4'],
      lines: [{ multipv: 1, cp: 0, pv: ['e2e4'], san: 'e4' }],
    })
    const cache = fakeCache({
      async put() {
        throw new Error('cache hit não deveria gravar')
      },
      async getBulk(fens, mode, value, multipv) {
        return fens.map((fen) => {
          gets.push({ fen, mode, value, multipv })
          return hit(fen)
        })
      },
      async putMany() {
        throw new Error('cache hit não deveria gravar')
      },
    })

    await analyzeGame('1. e4 e5', { mode: 'time', movetimeMs: 5000 }, port, 1, {
      cache,
    })

    expect(gets).toHaveLength(3)
    expect(
      gets.every(
        (g) => g.mode === 'time' && g.value === 5000 && g.multipv === 1,
      ),
    ).toBe(true)
  })
})
