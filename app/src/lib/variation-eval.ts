/**
 * Loop minimal de refino sobre uma única engine (SF18) já viva. Aponta a
 * engine para um FEN com `go infinite` e emite a posição refinada via
 * `onMerge` conforme `info depth N multipv K …` chega.
 *
 * Diferente da antiga `LiveEvalSession`, não há engine leve/wide, presets,
 * toggle, ou cache — só o essencial para alimentar `applyLiveToVariation`
 * enquanto o usuário explora uma sublinha.
 *
 * Puro sobre `EnginePort` (test seam).
 */
import type { EnginePort, RawLine, RawPosition } from './analyze'
import type { InfoScore } from './uci'
import { parseInfo, scoreToCp } from './uci'

export interface VariationEvalCallbacks {
  onMerge: (pos: RawPosition) => void
}

export interface VariationEvalSession {
  /** Envia `position fen <current>` seguido de `go infinite`. */
  start(): Promise<void>
  /**
   * Troca de posição: para o search atual (`stop`), limpa o estado acumulado,
   * e começa um novo `go infinite` no FEN informado.
   */
  setFen(fen: string): Promise<void>
  /** Para qualquer search em curso via UCI `stop`. A engine segue viva (idle). */
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

  port.onLine((line) => {
    if (ingest(line, slots)) {
      console.info(
        '[var-eval] info recebida em',
        curFen.slice(0, 20),
        'slots:',
        slots.size,
      )
      cb.onMerge(buildPosition(curFen, slots))
    }
  })

  return {
    async start() {
      console.info('[var-eval] start em', curFen.slice(0, 20))
      await port.send(
        `setoption name Multipv value ${Math.max(1, initial.multipv)}`,
      )
      await port.send(`position fen ${curFen}`)
      await port.send('go infinite')
    },
    async setFen(fen: string) {
      console.info('[var-eval] setFen', fen.slice(0, 20))
      curFen = fen
      slots.clear()
      await port.send('stop')
      await port.send(`position fen ${curFen}`)
      await port.send('go infinite')
    },
    async stop() {
      console.info('[var-eval] stop')
      await port.send('stop')
    },
  }
}
