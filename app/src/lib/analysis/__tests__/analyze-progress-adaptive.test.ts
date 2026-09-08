import { describe, expect, it } from 'vitest'
import {
  allHitsCache,
  fakeCache,
  fakePort,
  START_FEN,
} from './analyze-test-helpers'
import {
  type AnalysisProgress,
  analyzeGame,
  analyzeGameAdaptive,
  configureEngine,
  defaultGoTimeout,
  type EnginePort,
  type WinPctUpdate,
} from './analyze-test-runtime'

describe('analyzeGame — onDetailedProgress', () => {
  it('emite uma atualização indexada por posição', async () => {
    const port = fakePort(() => ({ cp: 0, pv: ['e2e4'] }))
    const updates: WinPctUpdate[] = []
    await analyzeGame('1. e4 e5', { mode: 'depth', depth: 20 }, port, 1, {
      onDetailedProgress: (_progress, update) => {
        if (update) updates.push(update)
      },
    })

    expect(updates).toHaveLength(3)
    expect(updates.map((update) => update.index)).toEqual([0, 1, 2])
  })

  it('entrega win% no ponto de vista das brancas', async () => {
    const port = fakePort((fen) =>
      fen === START_FEN ? { cp: 0, pv: ['e2e4'] } : { cp: 500, pv: ['d8h4'] },
    )
    const updates: WinPctUpdate[] = []
    const review = await analyzeGame(
      '1. e4',
      { mode: 'depth', depth: 20 },
      port,
      1,
      {
        onDetailedProgress: (_progress, update) => {
          if (update) updates.push(update)
        },
      },
    )

    expect(updates).toHaveLength(2)
    expect(updates[0].winPct).toBeCloseTo(50, 0)
    // após e4 as pretas estão a jogar com cp 500 → win% das brancas cai < 50
    expect(updates[1].winPct).toBeLessThan(50)
    expect(updates).toEqual(
      review.positions.map((position, index) => ({
        index,
        winPct: position.winPct,
      })),
    )
  })

  it('emite atualizações para posições vindas do cache (não só do engine)', async () => {
    let gos = 0
    let lineCb: ((line: string) => void) | null = null
    const port: EnginePort = {
      send(cmd) {
        const c = cmd.trim()
        if (c === 'uci') lineCb?.('uciok')
        else if (c === 'isready') lineCb?.('readyok')
        else if (c.startsWith('go')) gos++
      },
      onLine(handler) {
        lineCb = handler
        return () => {
          lineCb = null
        }
      },
    }
    const cache = allHitsCache()
    const updates: WinPctUpdate[] = []
    await analyzeGame('1. e4 e5', { mode: 'depth', depth: 20 }, port, 1, {
      cache,
      onDetailedProgress: (_progress, update) => {
        if (update) updates.push(update)
      },
    })

    expect(gos).toBe(0)
    expect(updates.map((update) => update.index)).toEqual([0, 1, 2])
  })

  it('emite atualizações para posições terminais resolvidas sem a engine', async () => {
    const port = fakePort(() => ({ cp: 0, pv: [] }))
    const updates: WinPctUpdate[] = []
    const review = await analyzeGame(
      '1. f3 e5 2. g4 Qh4#',
      { mode: 'depth', depth: 20 },
      port,
      1,
      {
        onDetailedProgress: (_progress, update) => {
          if (update) updates.push(update)
        },
      },
    )

    // 5 posições; a última é xeque-mate (resolvida por terminalCp, sem engine)
    expect(updates).toHaveLength(review.positions.length)
    expect(updates).toHaveLength(5)
    expect(updates).toEqual(
      review.positions.map((position, index) => ({
        index,
        winPct: position.winPct,
      })),
    )
  })
})

describe('defaultGoTimeout', () => {
  it('modo depth: orçamento fixo generoso (180s) — busca não tem limite inerente', () => {
    expect(defaultGoTimeout({ mode: 'depth', depth: 15 })).toBe(180_000)
    expect(defaultGoTimeout({ mode: 'depth', depth: 25 })).toBe(180_000)
  })

  it('modo time: 3·movetimeMs + 10s de folga — engine se auto-limita a movetimeMs', () => {
    expect(defaultGoTimeout({ mode: 'time', movetimeMs: 1000 })).toBe(13_000)
    expect(defaultGoTimeout({ mode: 'time', movetimeMs: 5000 })).toBe(25_000)
  })
})

describe('analyzeGameAdaptive', () => {
  it.each([false, true])(
    'flushes completed refinement on engine failure, preserving its cause (flush fails: %s)',
    async (flushFails) => {
      let callback: (line: string) => void = () => {}
      let triage = 0
      let refinement = 0
      const writes: Array<{ value: number; count: number }> = []
      const port: EnginePort = {
        send(command) {
          if (command === 'uci') callback('uciok')
          else if (command === 'isready') callback('readyok')
          else if (command.startsWith('go ')) {
            if (command === 'go movetime 120') triage++
            else if (++refinement === 2)
              throw new Error('refinement engine failed')
            const cp = triage === 11 ? 500 : 0
            callback(`info depth 12 multipv 1 score cp ${cp} pv e2e4`)
            callback('info depth 12 multipv 2 score cp -20 pv d2d4')
            callback('bestmove e2e4')
          }
        },
        onLine(handler) {
          callback = handler
          return () => {
            callback = () => {}
          }
        },
      }
      const cache = fakeCache({
        async putMany(entries, _mode, value) {
          writes.push({ value, count: entries.length })
          if (value === 1500 && flushFails) throw new Error('flush failed')
        },
      })
      await expect(
        analyzeGameAdaptive(
          '1. a3 a6 2. h3 h6 3. f3 f6 4. g3 g6 5. Kf2 Kf7',
          'fast',
          port,
          { cache },
        ),
      ).rejects.toThrow('refinement engine failed')
      expect(refinement).toBe(2)
      expect(writes.filter((write) => write.value === 1500)).toEqual([
        { value: 1500, count: 1 },
      ])
    },
  )

  it('aprofunda só a posição decisiva, sem repetir a partida inteira', async () => {
    const sent: string[] = []
    let lineCb: ((line: string) => void) | null = null
    let currentFen = ''
    let triageCount = 0
    let criticalFen = ''
    const progress: Array<{
      progress: AnalysisProgress
      update?: WinPctUpdate
    }> = []
    const cacheReads: Array<{ fens: string[]; value: number }> = []
    const cache = fakeCache({
      async get() {
        throw new Error('o refinamento deve prefetchar o cache em lote')
      },
      async getBulk(fens, _mode, value) {
        cacheReads.push({ fens, value })
        return fens.map(() => null)
      },
    })
    const port: EnginePort = {
      send(cmd) {
        const command = cmd.trim()
        sent.push(command)
        if (command === 'uci') lineCb?.('uciok')
        else if (command === 'isready') lineCb?.('readyok')
        else if (command.startsWith('position fen')) {
          currentFen = command.slice('position fen'.length).trim()
        } else if (command.startsWith('go movetime')) {
          if (command === 'go movetime 120') {
            triageCount++
            if (triageCount === 11) criticalFen = currentFen
          }
          const cp = currentFen === criticalFen && criticalFen ? 500 : 0
          lineCb?.(`info depth 12 multipv 1 score cp ${cp} pv e2e4`)
          lineCb?.('info depth 12 multipv 2 score cp -20 pv d2d4')
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

    const review = await analyzeGameAdaptive(
      '1. a3 a6 2. h3 h6 3. f3 f6 4. g3 g6 5. Kf2 Kf7',
      'fast',
      port,
      {
        cache,
        onDetailedProgress: (snapshot, update) =>
          progress.push({ progress: snapshot, update }),
      },
    )

    expect(review.positions).toHaveLength(11)
    expect(
      sent.filter((command) => command === 'go movetime 120'),
    ).toHaveLength(11)
    expect(
      sent.filter((command) => command === 'go movetime 1500'),
    ).toHaveLength(2)
    expect(sent).not.toContain('go movetime 600')
    expect(cacheReads.map((read) => [read.fens.length, read.value])).toEqual([
      [11, 120],
      [2, 1500],
    ])
    const refinement = progress.filter(
      ({ progress: snapshot }) => snapshot.stage === 'refinement',
    )
    expect(
      refinement.map(({ progress: snapshot }) => snapshot.completed),
    ).toEqual([0, 1, 2])
    expect(refinement.map(({ progress: snapshot }) => snapshot.total)).toEqual([
      2, 2, 2,
    ])
    expect(
      refinement.map(({ progress: snapshot }) => snapshot.remainingBudgetMs),
    ).toEqual([3000, 1500, 0])
    expect(
      progress.filter(
        ({ progress: snapshot }) => snapshot.stage === 'triage',
      )[0]?.update?.index,
    ).toBe(0)
    expect(refinement[0]?.update).toBeUndefined()
    const triageIndexes = new Set(
      progress
        .filter(({ progress: snapshot }) => snapshot.stage === 'triage')
        .map(({ update }) => update?.index),
    )
    expect(refinement.slice(1).map(({ update }) => update?.index)).toHaveLength(
      2,
    )
    expect(
      refinement
        .slice(1)
        .every(({ update }) => triageIndexes.has(update?.index)),
    ).toBe(true)
    expect(progress.at(-1)?.progress.stage).toBe('finalizing')
  })
})

describe('configureEngine', () => {
  /** Port que responde o handshake UCI sinicamente e grava todos os sends. */
  function recordingPort(): {
    port: EnginePort
    sent: string[]
  } {
    const sent: string[] = []
    let cb: ((line: string) => void) | null = null
    return {
      sent,
      port: {
        send(cmd: string) {
          sent.push(cmd.trim())
          const c = cmd.trim()
          if (c === 'uci') {
            cb?.('id name Stockfish 18')
            cb?.('uciok')
          } else if (c === 'isready') {
            cb?.('readyok')
          }
        },
        onLine(handler: (line: string) => void) {
          cb = handler
          return () => {
            cb = null
          }
        },
      },
    }
  }

  /** Port muda: registra sends mas nunca emite linha alguma. */
  function silentPort(): { port: EnginePort; sent: string[] } {
    const sent: string[] = []
    return {
      sent,
      port: {
        send(cmd: string) {
          sent.push(cmd.trim())
        },
        onLine() {
          return () => {}
        },
      },
    }
  }

  it('envia uci/isready e setoption Threads/Hash/Multipv', async () => {
    const { port, sent } = recordingPort()
    await configureEngine(port, {
      threads: 4,
      hashMb: 512,
      multipv: 3,
    })
    expect(sent).toEqual([
      'uci',
      'isready',
      'setoption name Threads value 4',
      'setoption name Hash value 512',
      'setoption name Multipv value 3',
      'isready',
    ])
  })

  it('omite Threads/Hash quando não fornecidos', async () => {
    const { port, sent } = recordingPort()
    await configureEngine(port, { multipv: 1 })
    expect(sent).toEqual([
      'uci',
      'isready',
      'setoption name Multipv value 1',
      'isready',
    ])
  })

  it('rejeita com mensagem clara quando a engine não responde (timeout)', async () => {
    const { port } = silentPort()
    await expect(
      configureEngine(port, { multipv: 1, timeoutMs: 25 }),
    ).rejects.toThrow(/não respondeu a 'uci'/)
  })
})
