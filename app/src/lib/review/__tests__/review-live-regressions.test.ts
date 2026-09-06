import { afterEach, describe, expect, it, vi } from 'vitest'
import type { PositionAnalysis } from '../../../types'
import { preferredAnalysis } from '../../analysis-quality'
import { selectDisplayedPosition } from '../../review-selectors'
import { createReviewStore, nodeAtPath } from '../../review-store'
import {
  depthConfig,
  existingResult,
  fakeBackend,
  fakeEnginePort,
  startSession,
} from './review-session-test-helpers'

const settings = {
  searchSeconds: 2,
  lines: 3,
  threadsAuto: false,
  threads: 2,
  memoryMb: 64,
  moveFeedbackEnabled: false,
}
afterEach(() => vi.useRealTimers())
function position(
  depth: number,
  purpose: 'playback' | 'refinement' = 'refinement',
  movetimeMs = 2000,
): PositionAnalysis {
  return {
    ...existingResult().positions[0],
    depth,
    cp: depth * 10,
    search: { purpose, movetimeMs, multipv: 1 },
  }
}
describe('regressões de avaliação e variações', () => {
  it('recupera a avaliação na base sem emprestar a nota a outra posição', () => {
    const store = createReviewStore()
    const result = existingResult()
    result.positions[0].cp = 180
    store.setResult(result)
    store.first()
    store.makeMove('d2', 'd4')
    expect(selectDisplayedPosition(store.getState())).toBeNull()
    store.prev()
    expect(selectDisplayedPosition(store.getState())).toBe(result.positions[0])
  })
  it('preserva notas profundas da partida e do store durante playback', () => {
    const store = createReviewStore()
    const result = existingResult()
    result.positions[0].depth = 24
    store.setResult(result)
    store.first()
    store.setLiveAnalysis(position(3).fen, position(3, 'playback'))
    expect(selectDisplayedPosition(store.getState())).toBe(result.positions[0])
    const deep = position(30)
    store.setLiveAnalysis(deep.fen, deep)
    store.setLiveAnalysis(deep.fen, position(5, 'playback'))
    expect(store.getState().liveAnalysis.positions[deep.fen]).toBe(deep)
    expect(selectDisplayedPosition(store.getState())).toBe(deep)
  })
  it('desempata por finalidade, cobertura e tempo comparável', () => {
    const full = position(20)
    const deeper = position(21, 'playback')
    const longer = position(20, 'refinement', 5000)
    expect(preferredAnalysis(full, position(20, 'playback'))).toBe(full)
    expect(preferredAnalysis(full, deeper)).toBe(deeper)
    expect(preferredAnalysis(longer, full)).toBe(longer)
    expect(preferredAnalysis(full, longer)).toBe(longer)
    const more = {
      ...full,
      lines: [{ multipv: 1, san: 'e4', cp: 1, winPct: 50, pv: ['e2e4'] }],
    }
    expect(preferredAnalysis(full, more)).toBe(more)
    expect(preferredAnalysis(more, full)).toBe(more)
  })
  it('reutiliza PVs na raiz e após sair, preservando classificação e continuação', () => {
    const store = createReviewStore()
    store.setResult(existingResult())
    store.first()
    const path = store.exploreLine(['e2e4', 'e7e5', 'g1f3'])
    store.setVariationClassification(path[0], 'bom')
    const before = store.getState().variation
    store.prev()
    expect(store.exploreLine(['e2e4', 'e7e5'])).toEqual(path.slice(0, 2))
    expect(store.getState().variation?.roots).toBe(before?.roots)
    store.exitVariation()
    expect(store.exploreLine(['e2e4', 'e7e5', 'g1f3'])).toEqual(path)
    expect(store.getState().variations).toHaveLength(1)
    expect(store.getState().variation?.roots[0].classification).toBe('bom')
  })
  it('preserva alternativas e devolve o caminho correto para playback', () => {
    const store = createReviewStore()
    store.setResult(existingResult())
    store.first()
    const oldPath = store.exploreLine(['e2e4', 'e7e5', 'g1f3'])
    const before = store.getState().variation
    store.prev()
    const path = store.exploreLine(['e2e4', 'c7c5', 'g1f3'])
    const variation = store.getState().variation
    if (!variation || !before) throw new Error('Variação esperada')
    expect(path[0]).toBe(oldPath[0])
    expect(variation.roots[0].children.map((n) => n.uci)).toEqual([
      'e7e5',
      'c7c5',
    ])
    expect(before.roots[0].children.map((n) => n.uci)).toEqual(['e7e5'])
    store.goToVariation(variation.id, path.slice(0, 2))
    expect(
      nodeAtPath(variation.roots, store.getState().variation?.path ?? [])?.uci,
    ).toBe('c7c5')
    store.goToVariation(variation.id, path.slice(0, 1))
    expect(store.exploreLine(['c7c5', 'g1f3'])).toEqual(path)
    expect(store.getState().variation?.roots[0].children).toHaveLength(2)
  })
  it('rejeita PV ilegal sem publicar prefixos', () => {
    const store = createReviewStore()
    store.setResult(existingResult())
    store.first()
    const before = store.getState()
    expect(store.exploreLine(['e2e4', 'e2e4'])).toEqual([])
    expect(store.getState()).toBe(before)
  })
  it('aguarda descarte após timeout antes de criar outra engine', async () => {
    vi.useFakeTimers()
    const oldPort = fakeEnginePort()
    const send = oldPort.send.bind(oldPort)
    oldPort.send = (cmd) => {
      if (cmd.startsWith('go ') || cmd === 'stop') oldPort.sent.push(cmd)
      else send(cmd)
    }
    let release = () => {}
    const disposal = new Promise<void>((resolve) => {
      release = resolve
    })
    oldPort.dispose = vi.fn(() => disposal)
    const nextPort = fakeEnginePort()
    const nextSend = nextPort.send.bind(nextPort)
    nextPort.send = (cmd) => {
      if (cmd.startsWith('go ')) nextPort.sent.push(cmd)
      else nextSend(cmd)
    }
    const backend = fakeBackend(oldPort)
    backend.createEnginePort = vi
      .fn()
      .mockResolvedValueOnce(oldPort)
      .mockResolvedValue(nextPort)
    const { session, store } = startSession({
      config: depthConfig({ initialResult: existingResult() }),
      backend,
    })
    await session.start()
    try {
      const [first, , second] = existingResult().positions
      session.analyzePosition({ fen: first.fen }, settings)
      await vi.advanceTimersByTimeAsync(0)
      expect(oldPort.sent).toContain('go movetime 2000')
      await vi.advanceTimersByTimeAsync(12000)
      expect(oldPort.dispose).toHaveBeenCalledOnce()
      session.analyzePosition({ fen: second.fen }, settings)
      await vi.advanceTimersByTimeAsync(0)
      expect(backend.createEnginePort).toHaveBeenCalledTimes(1)
      release()
      await vi.advanceTimersByTimeAsync(0)
      expect(nextPort.sent).toContain('go movetime 2000')
      oldPort.emit('info depth 30 score cp 900 pv e2e4')
      oldPort.emit('bestmove e2e4')
      expect(backend.createEnginePort).toHaveBeenCalledTimes(2)
      expect(
        store.getState().liveAnalysis.positions[second.fen],
      ).toBeUndefined()
      nextPort.emit('info depth 25 score cp 42 pv g1f3')
      nextPort.emit('bestmove g1f3')
      await vi.advanceTimersByTimeAsync(0)
      expect(
        store.getState().liveAnalysis.positions[second.fen]?.search,
      ).toEqual({ purpose: 'refinement', movetimeMs: 2000, multipv: 3 })
      expect(store.getState().liveAnalysis.positions[second.fen]?.cp).toBe(42)
      expect(store.getState().liveAnalysis.positions[first.fen]).toBeUndefined()
    } finally {
      release()
      session.dispose()
    }
  })
  it('cancela explicitamente, preserva avaliações e permite nova busca', async () => {
    const port = fakeEnginePort()
    const send = port.send.bind(port)
    port.send = (cmd) => {
      if (!cmd.startsWith('go ')) send(cmd)
    }
    const { session, store } = startSession({
      config: depthConfig({ initialResult: existingResult() }),
      backend: fakeBackend(port),
    })
    await session.start()
    try {
      const fen = position(20).fen
      store.setLiveAnalysis(fen, position(20))
      const positions = store.getState().liveAnalysis.positions
      session.analyzePosition({ fen }, settings)
      await vi.waitFor(() =>
        expect(store.getState().liveAnalysis.status).toBe('running'),
      )
      session.cancelLiveAnalysis()
      expect(store.getState().liveAnalysis.status).toBe('cancelled')
      expect(store.getState().liveAnalysis.positions).toBe(positions)
      port.emit('info depth 1 score cp 0 pv e2e4')
      port.emit('bestmove e2e4')
      port.send = send
      session.analyzePosition({ fen }, settings)
      await vi.waitFor(() =>
        expect(store.getState().liveAnalysis.status).toBe('idle'),
      )
    } finally {
      session.dispose()
    }
  })
})
