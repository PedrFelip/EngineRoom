import { Chessground } from 'chessground'
import type { Api } from 'chessground/api'
import type { Key } from 'chessground/types'
import { useEffect, useRef } from 'react'
import 'chessground/assets/chessground.base.css'
import 'chessground/assets/chessground.brown.css'
import 'chessground/assets/chessground.cburnett.css'

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

export default function Board({
  fen,
  orientation = 'white',
  lastMove = null,
  arrows = [],
  viewOnly = true,
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

  return <div ref={elRef} className='aspect-square w-full' />
}
