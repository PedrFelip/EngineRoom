import { describe, expect, it } from 'vitest'
import type { EnginePort, RawPosition } from './analyze'
import { createLiveEvalSession } from './live-eval'

const START_FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'

function fakePort() {
  const handlers = new Set<(line: string) => void>()
  const sent: string[] = []
  return {
    sent,
    port: {
      send: (line: string) => {
        sent.push(line)
      },
      onLine: (h: (line: string) => void) => {
        handlers.add(h)
        return () => {
          handlers.delete(h)
        }
      },
    } satisfies EnginePort,
    emit(line: string) {
      handlers.forEach((h) => h(line))
    },
  }
}

describe('createLiveEvalSession', () => {
  it('forwards each info line as a RawPosition via onMerge', async () => {
    const { port, emit } = fakePort()
    const merged: RawPosition[] = []
    const session = createLiveEvalSession(
      port,
      { fen: START_FEN, multipv: 1 },
      { onMerge: (pos) => merged.push(pos) },
    )

    await session.start()

    emit('info depth 20 multipv 1 score cp 30 pv e2e4 e7e5')

    expect(merged).toHaveLength(1)
    expect(merged[0]).toMatchObject({
      fen: START_FEN,
      cp: 30,
      depth: 20,
      pv: ['e2e4', 'e7e5'],
    })
  })
})
