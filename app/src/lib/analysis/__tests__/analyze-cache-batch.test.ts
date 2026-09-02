import { describe, expect, it, vi } from 'vitest'
import {
  analyzeGame,
  type EngineExitReason,
  type EnginePort,
} from '../../analyze'
import {
  fakeCache,
  fakePort,
  portDyingOnSecondGo,
  START_FEN,
} from './analyze-test-helpers'

describe('analyzeGame — cache em lote e falhas', () => {
  it('prefetcha todos os hits numa única chamada getBulk e só aciona a engine nos misses', async () => {
    let gos = 0
    let lineCb: ((line: string) => void) | null = null
    const port: EnginePort = {
      send(cmd) {
        const c = cmd.trim()
        if (c === 'uci') lineCb?.('uciok')
        else if (c === 'isready') lineCb?.('readyok')
        else if (c.startsWith('go')) {
          gos++
          lineCb?.('info depth 20 multipv 1 score cp 0 pv e2e4 e7e5')
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
    const getBulkCalls: Array<{
      fens: string[]
      mode: string
      value: number
      multipv: number
    }> = []
    const cache = fakeCache({
      async getBulk(fens, mode, value, multipv) {
        getBulkCalls.push({ fens, mode, value, multipv })
        // Hit só para a posição inicial; miss nas duas seguintes.
        return fens.map((fen, i) =>
          i === 0
            ? {
                fen,
                cp: 0,
                depth: 20,
                pv: ['e2e4'],
                lines: [{ multipv: 1, cp: 0, pv: ['e2e4'], san: 'e4' }],
              }
            : null,
        )
      },
    })

    const review = await analyzeGame(
      '1. e4 e5',
      { mode: 'depth', depth: 20 },
      port,
      1,
      { cache },
    )

    expect(getBulkCalls).toHaveLength(1)
    expect(getBulkCalls[0].fens).toHaveLength(3)
    expect(getBulkCalls[0]).toMatchObject({
      mode: 'depth',
      value: 20,
      multipv: 1,
    })
    expect(gos).toBe(2)
    expect(review.moves).toHaveLength(2)
  })

  it('descarrega todas as gravações numa única putMany ao final, em ordem (sem chamar put)', async () => {
    const port = fakePort(() => ({ cp: 0, pv: ['e2e4'] }))
    let putCalls = 0
    const putManyCalls: Array<{
      entries: RawPosition[]
      mode: string
      value: number
      multipv: number
    }> = []
    const cache = fakeCache({
      async put() {
        putCalls++
      },
      async putMany(entries, mode, value, multipv) {
        putManyCalls.push({ entries, mode, value, multipv })
      },
    })

    await analyzeGame('1. e4 e5', { mode: 'depth', depth: 18 }, port, 2, {
      cache,
    })

    expect(putCalls).toBe(0)
    expect(putManyCalls).toHaveLength(1)
    expect(putManyCalls[0].entries.map((p) => p.fen)).toEqual([
      START_FEN,
      'rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1',
      'rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2',
    ])
    expect(putManyCalls[0]).toMatchObject({
      mode: 'depth',
      value: 18,
      multipv: 2,
    })
  })

  it('descarrega incrementalmente a cada 8 posições e o restante ao final', async () => {
    const port = fakePort(() => ({ cp: 0, pv: ['e2e4'] }))
    const putManyCalls: RawPosition[][] = []
    const cache = fakeCache({
      async putMany(entries) {
        // Cópia: o buffer real é esvaziado por referência após cada flush.
        putManyCalls.push([...entries])
      },
    })

    // 8 lances = 9 posições avaliadas: flush incremental com 8 + final com 1.
    await analyzeGame(
      '1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6',
      { mode: 'depth', depth: 18 },
      port,
      1,
      { cache },
    )

    expect(putManyCalls.map((entries) => entries.length)).toEqual([8, 1])
    const fens = putManyCalls.flat().map((p) => p.fen)
    expect(fens[0]).toBe(START_FEN)
    expect(new Set(fens).size).toBe(9)
  })

  it('rejeita a análise quando um flush incremental falha (retry do catch vira warning)', async () => {
    const port = fakePort(() => ({ cp: 0, pv: ['e2e4'] }))
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    let putManyCalls = 0
    const cache = fakeCache({
      async putMany() {
        putManyCalls++
        throw new Error('disk full')
      },
    })

    await expect(
      analyzeGame(
        '1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6',
        { mode: 'depth', depth: 18 },
        port,
        1,
        { cache },
      ),
    ).rejects.toThrow(/disk full/)

    // Flush incremental na 8ª posição + retry best-effort no catch.
    expect(putManyCalls).toBe(2)
    expect(warn).toHaveBeenCalled()
    warn.mockRestore()
  })

  it('descarrega o buffer mesmo quando a análise aborta no meio (finally)', async () => {
    const port = portDyingOnSecondGo()
    const putManyCalls: RawPosition[][] = []
    const cache = fakeCache({
      async putMany(entries) {
        putManyCalls.push(entries)
      },
    })

    await expect(
      analyzeGame('1. e4 e5', { mode: 'depth', depth: 20 }, port, 1, {
        cache,
      }),
    ).rejects.toThrow(/engine morreu no segundo go/)

    // A posição inicial já tinha sido avaliada e bufferizada antes do aborto.
    expect(putManyCalls).toHaveLength(1)
    expect(putManyCalls[0]).toHaveLength(1)
    expect(putManyCalls[0][0].fen).toBe(START_FEN)
  })

  it('propaga a causa raiz da análise quando o descarrego do cache também falha', async () => {
    const port = portDyingOnSecondGo()
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    let putManyCalls = 0
    const cache = fakeCache({
      async putMany() {
        putManyCalls++
        throw new Error('disk full')
      },
    })

    await expect(
      analyzeGame('1. e4 e5', { mode: 'depth', depth: 20 }, port, 1, {
        cache,
      }),
    ).rejects.toThrow(/engine morreu no segundo go/)

    // O flush foi tentado uma vez e falhou como warning, sem substituir a
    // causa raiz (o erro da engine).
    expect(putManyCalls).toBe(1)
    expect(warn).toHaveBeenCalled()
    warn.mockRestore()
  })

  it('rejeita com o erro do descarrego quando a análise teve sucesso', async () => {
    const port = fakePort(() => ({ cp: 0, pv: ['e2e4'] }))
    const cache = fakeCache({
      async putMany() {
        throw new Error('disk full')
      },
    })

    await expect(
      analyzeGame('1. e4 e5', { mode: 'depth', depth: 20 }, port, 1, {
        cache,
      }),
    ).rejects.toThrow(/disk full/)
  })

  it('rejeita com mensagem do `go` e envia `stop` quando a busca excede goTimeoutMs', async () => {
    const sent: string[] = []
    let cb: ((line: string) => void) | null = null
    const port: EnginePort = {
      send(cmd) {
        sent.push(cmd.trim())
        const c = cmd.trim()
        if (c === 'uci') cb?.('uciok')
        else if (c === 'isready') cb?.('readyok')
      },
      onLine(handler) {
        cb = handler
        return () => {
          cb = null
        }
      },
    }

    await expect(
      analyzeGame('1. e4', { mode: 'depth', depth: 21 }, port, 1, {
        goTimeoutMs: 25,
      }),
    ).rejects.toThrow(/não respondeu a 'go depth 21'/)
    expect(sent).toContain('stop')
  })

  it('rejeita na hora (sem esperar o timeout) quando a engine morre durante o `go`', async () => {
    let lineCb: ((line: string) => void) | null = null
    let exitCb: ((r: EngineExitReason) => void) | null = null
    const port: EnginePort = {
      send(cmd) {
        const c = cmd.trim()
        if (c === 'uci') lineCb?.('uciok')
        else if (c === 'isready') lineCb?.('readyok')
        else if (c.startsWith('go'))
          exitCb?.({ code: null, signal: 11, error: undefined })
      },
      onLine(handler) {
        lineCb = handler
        return () => {
          lineCb = null
        }
      },
      onExit(handler) {
        exitCb = handler
        return () => {
          exitCb = null
        }
      },
    }

    const t0 = Date.now()
    await expect(
      analyzeGame('1. e4', { mode: 'depth', depth: 21 }, port, 1, {
        goTimeoutMs: 30_000,
      }),
    ).rejects.toThrow(/encerrou durante 'go depth 21'/)
    expect(Date.now() - t0).toBeLessThan(1000)
  })
})
