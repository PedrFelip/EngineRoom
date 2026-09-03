import { afterEach, describe, expect, it, vi } from 'vitest'
import type { EnginePort } from '../analysis-types'
import { evalPosition } from '../engine-analysis'

afterEach(() => vi.useRealTimers())

describe('evalPosition — limite rígido da busca ao vivo', () => {
  it('envia stop ao atingir o teto e conserva a última linha recebida', async () => {
    vi.useFakeTimers()
    const sent: string[] = []
    let onLine: (line: string) => void = () => {}
    const port: EnginePort = {
      send(command) {
        sent.push(command)
        if (command === 'stop') {
          onLine('info depth 18 multipv 1 score cp 24 pv e2e4 e7e5')
          onLine('bestmove e2e4')
        }
      },
      onLine(handler) {
        onLine = handler
        return () => {
          onLine = () => {}
        }
      },
    }

    const resultPromise = evalPosition(
      port,
      'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1',
      { mode: 'time', movetimeMs: 100 },
      10_000,
      100,
    )
    await vi.advanceTimersByTimeAsync(100)
    const result = await resultPromise

    expect(sent).toContain('go movetime 100')
    expect(sent).toContain('stop')
    expect(result.cp).toBe(24)
    expect(result.depth).toBe(18)
  })
})
