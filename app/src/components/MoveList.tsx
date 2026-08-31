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
      className={`flex min-h-8 flex-1 items-center gap-1.5 rounded-[calc(var(--radius)-3px)] px-2 text-left font-mono text-[13px] transition-colors ${
        active
          ? 'bg-accent text-accent-foreground shadow-sm ring-1 ring-border'
          : 'text-ink-dim hover:bg-accent/70 hover:text-ink'
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

  // Mantém o lance ativo visível apenas dentro do painel de lances. Usar
  // scrollIntoView aqui também rola a página e dá a impressão de que o foco
  // saiu do tabuleiro ao navegar pelas setas.
  // biome-ignore lint/correctness/useExhaustiveDependencies: o efeito precisa re-rodar quando currentPly muda para rolar até o lance ativo
  useEffect(() => {
    const root = rootRef.current
    const panel = root?.parentElement
    const active = root?.querySelector<HTMLElement>('[data-active="true"]')
    if (!panel || !active) return

    const panelRect = panel.getBoundingClientRect()
    const activeRect = active.getBoundingClientRect()
    if (activeRect.top < panelRect.top) {
      panel.scrollTop += activeRect.top - panelRect.top
    } else if (activeRect.bottom > panelRect.bottom) {
      panel.scrollTop += activeRect.bottom - panelRect.bottom
    }
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
        <div key={row.num} className='flex items-center gap-1.5'>
          <span className='flex size-7 shrink-0 items-center justify-center rounded-[calc(var(--radius)-3px)] bg-muted/60 font-mono text-[10px] text-muted-foreground'>
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
