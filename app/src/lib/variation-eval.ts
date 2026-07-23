/**
 * Loop minimal de refino sobre uma única engine (SF18) já viva. Aponta a
 * engine para um FEN com `go infinite` e emite a posição refinada via
 * `onMerge` conforme `info depth N multipv K …` chega.
 *
 * Puro sobre `EnginePort` (test seam): a engine é sempre injetada, nunca
 * instanciada aqui. O handler de linhas é registrado no construction e
 * removido em `stop()` — descartar a session sem chamar `stop()` vaza o
 * handler.
 */
import type { EnginePort, RawLine, RawPosition } from './analyze'
import type { InfoScore } from './uci'
import { parseInfo, scoreToCp } from './uci'

export interface VariationEvalCallbacks {
  onMerge: (pos: RawPosition) => void
}

export interface VariationEvalSession {
  /** Envia `setoption Multipv` + `position fen <current>` + `go infinite`. */
  start(): Promise<void>
  /**
   * Troca de posição: para o search atual (`stop`), limpa o estado acumulado,
   * e começa um novo `go infinite` no FEN informado.
   */
  setFen(fen: string): Promise<void>
  /**
   * Para qualquer search em curso via UCI `stop` e desregistra o handler de
   * linhas. A engine segue viva (idle) — só pra ser morta pelo `dispose` do
   * port dono. Idempotente.
   */
  stop(): Promise<void>
}

interface Slot {
  depth: number
  score: InfoScore
  pv: string[]
}

type SlotMap = Map<number, Slot>

function ingest(line: string, slots: SlotMap): boolean {
  const info = parseInfo(line)
  if (!info?.score) return false
  const idx = info.multipv ?? 1
  const prev = slots.get(idx)
  if (prev && (info.depth ?? 0) < prev.depth) return false
  slots.set(idx, {
    depth: info.depth ?? 0,
    score: info.score,
    pv: info.pv ?? [],
  })
  return true
}

function buildPosition(fen: string, slots: SlotMap): RawPosition {
  const entries = [...slots.entries()].sort((a, b) => a[0] - b[0])
  const lines: RawLine[] = entries.map(([idx, s]) => ({
    multipv: idx,
    cp: scoreToCp(s.score) ?? 0,
    pv: s.pv,
  }))
  const primary = entries.find(([idx]) => idx === 1)?.[1] ?? entries[0]?.[1]
  return {
    fen,
    cp: primary ? (scoreToCp(primary.score) ?? 0) : 0,
    depth: primary?.depth ?? 0,
    pv: primary?.pv ?? [],
    lines,
  }
}

export function createVariationEvalSession(
  port: EnginePort,
  initial: { fen: string; multipv: number },
  cb: VariationEvalCallbacks,
): VariationEvalSession {
  let curFen = initial.fen
  const slots: SlotMap = new Map()
  let off: (() => void) | null = port.onLine((line) => {
    if (ingest(line, slots)) cb.onMerge(buildPosition(curFen, slots))
  })

  return {
    async start() {
      await port.send(
        `setoption name Multipv value ${Math.max(1, initial.multipv)}`,
      )
      await port.send(`position fen ${curFen}`)
      await port.send('go infinite')
    },
    async setFen(fen: string) {
      curFen = fen
      slots.clear()
      await port.send('stop')
      await port.send(`position fen ${curFen}`)
      await port.send('go infinite')
    },
    async stop() {
      if (off) {
        off()
        off = null
      }
      await port.send('stop')
    },
  }
}
