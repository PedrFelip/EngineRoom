import { useEffect, useRef } from 'react'
import type { MoveAnalysis } from '../types'
import ClassificationBadge from './ClassificationBadge'

interface MoveListProps {
  moves: MoveAnalysis[]
  currentPly: number
  onSelect: (ply: number) => void
}

interface Row {
  num: number
  white?: MoveAnalysis
  black?: MoveAnalysis
}

function MoveButton({
  move,
  active,
  onSelect,
}: {
  move: MoveAnalysis
  active: boolean
  onSelect: (ply: number) => void
}) {
  return (
    <button
      type='button'
      data-active={active ? 'true' : undefined}
      onClick={() => onSelect(move.ply)}
      className={`flex flex-1 items-center gap-1.5 rounded px-2 py-1 text-left font-mono text-sm transition ${
        active
          ? 'bg-brand/20 ring-1 ring-brand/50 text-ink'
          : 'text-ink-dim hover:bg-panel-3/50'
      }`}
    >
      <ClassificationBadge classification={move.classification} />
      <span>{move.san}</span>
    </button>
  )
}

export default function MoveList({
  moves,
  currentPly,
  onSelect,
}: MoveListProps) {
  const rootRef = useRef<HTMLDivElement>(null)

  // biome-ignore lint/correctness/useExhaustiveDependencies: o efeito precisa re-rodar quando currentPly muda para rolar até o lance ativo, mesmo sem usar o valor diretamente no body
  useEffect(() => {
    const el = rootRef.current?.querySelector('[data-active="true"]')
    el?.scrollIntoView({ block: 'nearest' })
  }, [currentPly])

  const rows: Row[] = []
  moves.forEach((m) => {
    const num = Math.ceil(m.ply / 2)
    if (!rows[num - 1]) rows[num - 1] = { num }
    if (m.color === 'w') rows[num - 1].white = m
    else rows[num - 1].black = m
  })

  return (
    <div ref={rootRef} className='flex flex-col gap-0.5'>
      {rows.map((row) => (
        <div key={row.num} className='flex items-center gap-1'>
          <span className='w-8 shrink-0 text-right font-mono text-xs text-ink-faint'>
            {row.num}.
          </span>
          {row.white ? (
            <MoveButton
              move={row.white}
              active={row.white.ply === currentPly}
              onSelect={onSelect}
            />
          ) : (
            <span className='flex-1' />
          )}
          {row.black ? (
            <MoveButton
              move={row.black}
              active={row.black.ply === currentPly}
              onSelect={onSelect}
            />
          ) : (
            <span className='flex-1' />
          )}
        </div>
      ))}
    </div>
  )
}
