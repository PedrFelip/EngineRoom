import { afterEach, describe, expect, it, vi } from 'vitest'
import type { EnginePort } from '../analysis-types'
import { evalPosition } from '../engine-analysis'

afterEach(() => vi.useRealTimers())

describe('evalPosition — conclusão e cancelamento da busca ao vivo', () => {
  it('interrompe a engine se o timeout de segurança expirar', async () => {
    vi.useFakeTimers()
    const send = vi.fn()
    const off = vi.fn()
    const port: EnginePort = { send, onLine: () => off }
    const result = evalPosition(
      port,
      'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1',
      { mode: 'time', movetimeMs: 100 },
      1000,
    )
    const rejected = expect(result).rejects.toThrow('não respondeu')
    await vi.advanceTimersByTimeAsync(100)
    expect(send).not.toHaveBeenCalledWith('stop')
    await vi.advanceTimersByTimeAsync(900)
    await rejected
    expect(send).toHaveBeenCalledWith('stop')
    expect(off).toHaveBeenCalledOnce()
  })

  it('não transforma uma busca sem score em avaliação zero', async () => {
    let onLine: (line: string) => void = () => {}
    const port: EnginePort = {
      send(command) {
        if (command.startsWith('go ')) onLine('bestmove e2e4')
      },
      onLine(handler) {
        onLine = handler
        return () => {
          onLine = () => {}
        }
      },
    }

    await expect(
      evalPosition(
        port,
        'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1',
        { mode: 'time', movetimeMs: 100 },
        1000,
      ),
    ).rejects.toThrow('sem avaliação')
  })

  it.each([0, 24])(
    'conserva o score %i recebido ao cancelar uma busca com stop',
    async (cp) => {
      vi.useFakeTimers()
      const sent: string[] = []
      let onLine: (line: string) => void = () => {}
      const port: EnginePort = {
        send(command) {
          sent.push(command)
          if (command === 'stop') {
            onLine(`info depth 18 multipv 1 score cp ${cp} pv e2e4 e7e5`)
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
      )
      await vi.advanceTimersByTimeAsync(100)
      expect(sent).not.toContain('stop')
      await port.send('stop')
      const result = await resultPromise

      expect(sent).toContain('go movetime 100')
      expect(sent).toContain('stop')
      expect(result.cp).toBe(cp)
      expect(result.depth).toBe(18)
    },
  )
})
