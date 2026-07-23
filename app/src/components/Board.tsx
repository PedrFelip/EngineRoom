import { Chessground } from 'chessground'
import type { Api } from 'chessground/api'
import type { Key } from 'chessground/types'
import { useEffect, useMemo, useRef } from 'react'
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
  /** Habilita o arraste de peças pelo usuário (linha alternativa). */
  interactive?: boolean
  /** Cor do lado a jogar (quando interativo). */
  turnColor?: 'white' | 'black' | null
  /** Destinos lícitos por casa (quando interativo). */
  dests?: Map<string, string[]> | null
  /** Emitido com o lance UCI ("e2e4"/"e7e8q") após o usuário arrastar uma peça. */
  onUserMove?: (uci: string) => void
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
  interactive = false,
  turnColor = null,
  dests = null,
  onUserMove,
}: BoardProps) {
  const elRef = useRef<HTMLDivElement>(null)
  const cgRef = useRef<Api | null>(null)
  // Mantém o handler de lance do usuário sempre atualizado sem recriar o config.
  const onUserMoveRef = useRef(onUserMove)
  onUserMoveRef.current = onUserMove

  const movable = useMemo(() => {
    if (!interactive) return undefined
    return {
      free: false,
      color: (turnColor ?? undefined) as 'white' | 'black' | 'both' | undefined,
      dests: (dests as unknown as Map<Key, Key[]>) ?? undefined,
      showDests: true,
      events: {
        after: (orig: Key, dest: Key) =>
          onUserMoveRef.current?.(`${orig}${dest}`),
      },
    }
  }, [interactive, turnColor, dests])

  // Tabuleiro interativo sempre destrava o viewOnly, ignorando o prop.
  const effectiveViewOnly = interactive ? false : viewOnly

  // biome-ignore lint/correctness/useExhaustiveDependencies: monta o Chessground uma única vez; updates vão via .set() no effect abaixo
  useEffect(() => {
    if (!elRef.current) return
    cgRef.current = Chessground(elRef.current, {
      fen,
      orientation,
      lastMove: lastMove ? toKeys(lastMove) : undefined,
      coordinates: true,
      viewOnly: effectiveViewOnly,
      highlight: { lastMove: true, check: true },
      animation: { enabled: true, duration: 200 },
      drawable: { enabled: true, visible: true, shapes: shapesFrom(arrows) },
      movable,
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
      viewOnly: effectiveViewOnly,
      drawable: { enabled: true, visible: true, shapes: shapesFrom(arrows) },
      movable,
    })
  }, [fen, orientation, lastMove, arrows, effectiveViewOnly, movable])

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
