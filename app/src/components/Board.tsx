import { Chess } from 'chess.js'
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
  /** Classificação do último lance, exibida como selo sobre a casa de destino. */
  lastMoveClassification?: Classification | null
  onMove?: (from: string, to: string, promotion?: string) => boolean
}

type PromotionPiece = 'q' | 'r' | 'b' | 'n'

interface PendingPromotion {
  from: string
  to: string
  color: 'w' | 'b'
  fen: string
}

const promotionOptions: Array<{
  piece: PromotionPiece
  label: string
  symbol: { w: string; b: string }
}> = [
  { piece: 'q', label: 'Dama', symbol: { w: '♕', b: '♛' } },
  { piece: 'r', label: 'Torre', symbol: { w: '♖', b: '♜' } },
  { piece: 'b', label: 'Bispo', symbol: { w: '♗', b: '♝' } },
  { piece: 'n', label: 'Cavalo', symbol: { w: '♘', b: '♞' } },
]

function promotionColor(fen: string, from: string, to: string) {
  if (!['1', '8'].includes(to[1])) return null
  const move = new Chess(fen)
    .moves({ verbose: true })
    .find(
      (candidate) =>
        candidate.from === from && candidate.to === to && candidate.promotion,
    )
  return move?.color ?? null
}

function legalDests(fen: string): Map<Key, Key[]> {
  const chess = new Chess(fen)
  const dests = new Map<Key, Key[]>()
  for (const move of chess.moves({ verbose: true })) {
    const from = move.from as Key
    const destinations = dests.get(from) ?? []
    destinations.push(move.to as Key)
    dests.set(from, destinations)
  }
  return dests
}

function turnColor(fen: string): 'white' | 'black' {
  return fen.split(' ')[1] === 'b' ? 'black' : 'white'
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
  lastMoveClassification = null,
  onMove,
}: BoardProps) {
  const elRef = useRef<HTMLDivElement>(null)
  const cgRef = useRef<Api | null>(null)
  const [pendingPromotion, setPendingPromotion] =
    useState<PendingPromotion | null>(null)
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
      viewOnly: false,
      turnColor: turnColor(fen),
      movable: onMove
        ? {
            free: false,
            color: turnColor(fen),
            dests: legalDests(fen),
            events: { after: (from, to) => onMove(from, to) },
          }
        : undefined,
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
    const hasPendingPromotion = pendingPromotion?.fen === fen
    cgRef.current?.set({
      fen,
      orientation,
      lastMove: lastMove ? toKeys(lastMove) : undefined,
      turnColor: turnColor(fen),
      movable: onMove
        ? {
            free: false,
            color: hasPendingPromotion ? undefined : turnColor(fen),
            dests: hasPendingPromotion ? new Map() : legalDests(fen),
            events: {
              after: (from, to) => {
                const color = promotionColor(fen, from, to)
                if (color) {
                  setPendingPromotion({ from, to, color, fen })
                  return
                }
                onMove(from, to)
              },
            },
          }
        : {
            color: undefined,
            dests: new Map(),
          },
      drawable: { enabled: true, visible: true, shapes: shapesFrom(arrows) },
    })
  }, [fen, orientation, lastMove, arrows, onMove, pendingPromotion])

  const choosePromotion = (piece: PromotionPiece) => {
    if (!pendingPromotion || pendingPromotion.fen !== fen || !onMove) return
    const accepted = onMove(pendingPromotion.from, pendingPromotion.to, piece)
    setPendingPromotion(null)
    if (!accepted) cgRef.current?.set({ fen })
  }

  const badgeSquare = lastMove && lastMoveClassification ? lastMove[1] : null
  const badgePos = badgeSquare ? squarePosition(badgeSquare, orientation) : null
  const files = orientation === 'white' ? 'abcdefgh' : 'hgfedcba'
  const ranks = orientation === 'white' ? '87654321' : '12345678'

  return (
    <div className='board-shell w-full'>
      <div className='board-frame relative'>
        <div ref={elRef} className='aspect-square w-full' />
        {pendingPromotion?.fen === fen ? (
          <div className='absolute inset-0 z-20 flex items-center justify-center bg-black/45'>
            <div
              className='rounded-xl border border-edge bg-panel p-3 shadow-xl'
              role='dialog'
              aria-label='Escolha a peça para promoção'
            >
              <p className='mb-2 text-center text-sm font-medium text-ink'>
                Promover para
              </p>
              <div className='flex gap-2'>
                {promotionOptions.map(({ piece, label, symbol }) => (
                  <button
                    key={piece}
                    type='button'
                    className='flex size-12 items-center justify-center rounded-lg border border-edge bg-panel-2 text-4xl text-ink transition-colors hover:bg-edge/50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent'
                    aria-label={label}
                    onClick={() => choosePromotion(piece)}
                  >
                    {symbol[pendingPromotion.color]}
                  </button>
                ))}
              </div>
            </div>
          </div>
        ) : null}
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
