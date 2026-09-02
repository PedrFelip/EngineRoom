import type { EnginePort, PositionCache } from '../../analyze'

export const START_FEN =
  'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
export const AFTER_E4 =
  'rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1'
export const AFTER_E5 =
  'rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2'

export function fakePort(
  evalFor: (fen: string) => { cp: number; pv: string[]; depth?: number },
): EnginePort {
  let lineCb: ((line: string) => void) | null = null
  let currentFen = ''
  return {
    send(cmd: string) {
      const c = cmd.trim()
      if (c === 'uci') lineCb?.('uciok')
      else if (c === 'isready') lineCb?.('readyok')
      else if (c.startsWith('position fen'))
        currentFen = c.slice('position fen'.length).trim()
      else if (c.startsWith('go')) {
        const { cp, pv, depth = 20 } = evalFor(currentFen)
        lineCb?.(
          `info depth ${depth} multipv 1 score cp ${cp} pv ${pv.join(' ')}`,
        )
        lineCb?.(`bestmove ${pv[0] ?? 'e2e4'}`)
      }
    },
    onLine(handler: (line: string) => void) {
      lineCb = handler
      return () => {
        lineCb = null
      }
    },
  }
}

/** Cache falso de base: miss em toda leitura, gravações no-op. */
export function fakeCache(
  overrides: Partial<PositionCache> = {},
): PositionCache {
  return {
    async get() {
      return null
    },
    async put() {},
    async getBulk(fens) {
      return fens.map(() => null)
    },
    async putMany() {},
    ...overrides,
  }
}

/** Cache falso que acerta todas as posições com avaliação neutra (depth 20)
 * e trata qualquer gravação como erro — hits não deveriam gravar. */
export function allHitsCache(): PositionCache {
  const hit = (fen: string) => ({
    fen,
    cp: 0,
    depth: 20,
    pv: ['e2e4'],
    lines: [{ multipv: 1, cp: 0, pv: ['e2e4'], san: 'e4' }],
  })
  return fakeCache({
    async get(fen) {
      return hit(fen)
    },
    async put() {
      throw new Error('cache hit não deveria gravar')
    },
    async getBulk(fens) {
      return fens.map(hit)
    },
    async putMany() {
      throw new Error('cache hit não deveria gravar')
    },
  })
}

/** Port que responde o primeiro `go` e morre no segundo. */
export function portDyingOnSecondGo(): EnginePort {
  let lineCb: ((line: string) => void) | null = null
  let goCount = 0
  return {
    send(cmd) {
      const c = cmd.trim()
      if (c === 'uci') lineCb?.('uciok')
      else if (c === 'isready') lineCb?.('readyok')
      else if (c.startsWith('go')) {
        goCount++
        if (goCount === 1) {
          lineCb?.('info depth 20 multipv 1 score cp 0 pv e2e4 e7e5')
          lineCb?.('bestmove e2e4')
        } else {
          throw new Error('engine morreu no segundo go')
        }
      }
    },
    onLine(handler) {
      lineCb = handler
      return () => {
        lineCb = null
      }
    },
  }
}
