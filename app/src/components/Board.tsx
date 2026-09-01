import { Chessground } from 'chessground'
import type { Api } from 'chessground/api'
import type { Key } from 'chessground/types'
import { memo, useEffect, useRef, useState } from 'react'
import 'chessground/assets/chessground.base.css'
import 'chessground/assets/chessground.brown.css'
import 'chessground/assets/chessground.cburnett.css'
import type { Classification } from '../types'
import { ClassGlyph } from './ClassificationBadge'

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
function squarePosition(square: string, orientation: 'white' | 'black') {
  const file = square.charCodeAt(0) - 97
  const rank = Number.parseInt(square[1], 10) - 1
  const column = orientation === 'white' ? file : 7 - file
  const row = orientation === 'white' ? 7 - rank : rank
  return { column, row }
}

const Board = memo(function Board({
  fen,
  orientation = 'white',
  lastMove = null,
  arrows = [],
  viewOnly = true,
  lastMoveClassification = null,
}: BoardProps) {
  const elRef = useRef<HTMLDivElement>(null)
  const cgRef = useRef<Api | null>(null)
  const [boardBounds, setBoardBounds] = useState({
    left: 0,
    top: 0,
    width: 0,
    height: 0,
  })

  // biome-ignore lint/correctness/useExhaustiveDependencies: monta o Chessground uma única vez; updates vão via .set() no effect abaixo
  useEffect(() => {
    if (!elRef.current) return
    cgRef.current = Chessground(elRef.current, {
      fen,
      orientation,
      lastMove: lastMove ? toKeys(lastMove) : undefined,
      coordinates: false,
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
    const host = elRef.current
    const board = host?.querySelector('cg-board')
    if (!host || !board) return

    const updateBounds = () => {
      const hostRect = host.getBoundingClientRect()
      const boardRect = board.getBoundingClientRect()
      setBoardBounds({
        left: boardRect.left - hostRect.left,
        top: boardRect.top - hostRect.top,
        width: boardRect.width,
        height: boardRect.height,
      })
    }

    updateBounds()
    const observer = new ResizeObserver(updateBounds)
    observer.observe(host)
    observer.observe(board)
    return () => observer.disconnect()
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
  const badgePos = badgeSquare ? squarePosition(badgeSquare, orientation) : null
  const files = orientation === 'white' ? 'abcdefgh' : 'hgfedcba'
  const ranks = orientation === 'white' ? '87654321' : '12345678'

  return (
    <div className='board-shell w-full'>
      <div className='board-frame relative'>
        <div ref={elRef} className='aspect-square w-full' />
        {badgePos && lastMoveClassification ? (
          <div
            className='pointer-events-none absolute'
            style={{
              left:
                boardBounds.left + (badgePos.column * boardBounds.width) / 8,
              top: boardBounds.top + (badgePos.row * boardBounds.height) / 8,
              width: boardBounds.width / 8,
              height: boardBounds.height / 8,
            }}
          >
            <span className='absolute -right-1.5 -top-1.5'>
              <ClassGlyph
                classification={lastMoveClassification}
                size='board'
              />
            </span>
          </div>
        ) : null}
      </div>
      <div className='board-ranks' aria-hidden='true'>
        {[...ranks].map((rank) => (
          <span key={rank}>{rank}</span>
        ))}
      </div>
      <div className='board-files' aria-hidden='true'>
        {[...files].map((file) => (
          <span key={file}>{file}</span>
        ))}
      </div>
    </div>
  )
})

export default Board
