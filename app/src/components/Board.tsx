import { Chessground } from 'chessground'
import type { Api } from 'chessground/api'
import type { Key } from 'chessground/types'
import { useEffect, useRef } from 'react'
import 'chessground/assets/chessground.base.css'
import 'chessground/assets/chessground.brown.css'
import 'chessground/assets/chessground.cburnett.css'
import { CLASSIFICATION_LABELS } from '../lib/scoring'
import type { Classification } from '../types'

const BADGE_COLOR: Record<Classification, string> = {
  livro: 'bg-book',
  melhor: 'bg-best',
  excelente: 'bg-excellent',
  bom: 'bg-good',
  imprecisao: 'bg-mistake',
  erro: 'bg-erro',
  blunder: 'bg-blunder',
}

export interface BoardArrow {
  from: string
  to: string
  brush?: 'green' | 'red' | 'blue' | 'yellow'
}

export interface BoardProps {
  fen: string
  orientation?: 'white' | 'black'
  lastMove?: [string, string] | null
  arrows?: BoardArrow[]
  viewOnly?: boolean
  /** Classificação do último lance, exibida como selo sobre a casa de destino. */
  lastMoveClassification?: Classification | null
}

function toKeys(pair: [string, string]): Key[] {
  return [pair[0] as Key, pair[1] as Key]
}

function shapesFrom(arrows: BoardArrow[]) {
  return arrows.map((a) => ({
    orig: a.from as Key,
    dest: a.to as Key,
    brush: a.brush ?? 'green',
  }))
}

/** Posição (%,%) do canto superior esquerdo da casa, conforme a orientação. */
function squareTopLeft(square: string, orientation: 'white' | 'black') {
  const file = square.charCodeAt(0) - 97
  const rank = Number.parseInt(square[1], 10) - 1
  const left =
    orientation === 'white' ? (file / 8) * 100 : ((7 - file) / 8) * 100
  const top =
    orientation === 'white' ? ((7 - rank) / 8) * 100 : (rank / 8) * 100
  return { left, top }
}

export default function Board({
  fen,
  orientation = 'white',
  lastMove = null,
  arrows = [],
  viewOnly = true,
  lastMoveClassification = null,
}: BoardProps) {
  const elRef = useRef<HTMLDivElement>(null)
  const cgRef = useRef<Api | null>(null)

  // biome-ignore lint/correctness/useExhaustiveDependencies: monta o Chessground uma única vez; updates vão via .set() no effect abaixo
  useEffect(() => {
    if (!elRef.current) return
    cgRef.current = Chessground(elRef.current, {
      fen,
      orientation,
      lastMove: lastMove ? toKeys(lastMove) : undefined,
      coordinates: true,
      viewOnly,
      highlight: { lastMove: true, check: true },
      animation: { enabled: true, duration: 200 },
      drawable: { enabled: true, visible: true, shapes: shapesFrom(arrows) },
    })
    return () => {
      cgRef.current?.destroy()
      cgRef.current = null
    }
  }, [])

  useEffect(() => {
    cgRef.current?.set({
      fen,
      orientation,
      lastMove: lastMove ? toKeys(lastMove) : undefined,
      viewOnly,
      drawable: { enabled: true, visible: true, shapes: shapesFrom(arrows) },
    })
  }, [fen, orientation, lastMove, arrows, viewOnly])

  const badgeSquare = lastMove && lastMoveClassification ? lastMove[1] : null
  const badgePos = badgeSquare ? squareTopLeft(badgeSquare, orientation) : null

  return (
    <div className='relative w-full'>
      <div ref={elRef} className='aspect-square w-full' />
      {badgePos && lastMoveClassification ? (
        <div
          className='pointer-events-none absolute'
          style={{
            left: `${badgePos.left}%`,
            top: `${badgePos.top}%`,
            width: '12.5%',
            height: '12.5%',
          }}
        >
          <span
            role='img'
            aria-label={CLASSIFICATION_LABELS[lastMoveClassification]}
            title={CLASSIFICATION_LABELS[lastMoveClassification]}
            className={`absolute -right-1 -top-1 block h-3.5 w-3.5 rounded-full border-2 border-bg shadow-md ${BADGE_COLOR[lastMoveClassification]}`}
          />
        </div>
      ) : null}
    </div>
  )
}
