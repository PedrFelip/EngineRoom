import { useEffect, useRef } from 'react'
import type { MoveAnalysis, Variation, VariationMap } from '../types'
import ClassificationBadge from './ClassificationBadge'

interface MoveListProps {
  moves: MoveAnalysis[]
  currentPly: number
  onSelect: (ply: number) => void
  variations: VariationMap
  currentVariation: {
    variationId: string
    parentPly: number
    ply: number
  } | null
  onSelectVariation: (
    variationId: string,
    parentPly: number,
    ply: number,
  ) => void
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

function VariationButton({
  move,
  active,
  onSelect,
}: {
  move: Variation['moves'][number]
  active: boolean
  onSelect: () => void
}) {
  return (
    <button
      type='button'
      data-active={active ? 'true' : undefined}
      onClick={onSelect}
      className={`flex items-center gap-1.5 rounded px-1.5 py-0.5 text-left font-mono text-xs transition ${
        active
          ? 'bg-brand/20 ring-1 ring-brand/50 text-ink'
          : 'text-ink-dim hover:bg-panel-3/50'
      }`}
    >
      {move.classification ? (
        <ClassificationBadge classification={move.classification} />
      ) : (
        <span
          className='inline-block h-2.5 w-2.5 shrink-0 rounded-full bg-ink-faint/40'
          title='Analisando…'
        />
      )}
      <span>{move.san}</span>
    </button>
  )
}

export default function MoveList({
  moves,
  currentPly,
  onSelect,
  variations,
  currentVariation,
  onSelectVariation,
}: MoveListProps) {
  const rootRef = useRef<HTMLDivElement>(null)

  // biome-ignore lint/correctness/useExhaustiveDependencies: o efeito precisa re-rodar quando currentPly muda para rolar até o lance ativo, mesmo sem usar o valor diretamente no body
  useEffect(() => {
    const el = rootRef.current?.querySelector('[data-active="true"]')
    el?.scrollIntoView({ block: 'nearest' })
  }, [currentPly, currentVariation])

  const rows: Row[] = []
  moves.forEach((m) => {
    const num = Math.ceil(m.ply / 2)
    if (!rows[num - 1]) rows[num - 1] = { num }
    if (m.color === 'w') rows[num - 1].white = m
    else rows[num - 1].black = m
  })

  return (
    <div ref={rootRef} className='flex flex-col gap-0.5'>
      {rows.map((row) => {
        const rowPlies = [row.white?.ply, row.black?.ply].filter(
          (p): p is number => p != null && (variations[p]?.length ?? 0) > 0,
        )
        return (
          <div key={row.num} className='flex flex-col gap-0.5'>
            <div className='flex items-center gap-1'>
              <span className='w-8 shrink-0 text-right font-mono text-xs text-ink-faint'>
                {row.num}.
              </span>
              {row.white ? (
                <MoveButton
                  move={row.white}
                  active={row.white.ply === currentPly && !currentVariation}
                  onSelect={onSelect}
                />
              ) : (
                <span className='flex-1' />
              )}
              {row.black ? (
                <MoveButton
                  move={row.black}
                  active={row.black.ply === currentPly && !currentVariation}
                  onSelect={onSelect}
                />
              ) : (
                <span className='flex-1' />
              )}
            </div>
            {rowPlies.flatMap((parentPly) =>
              (variations[parentPly] ?? []).map((v) => (
                <div
                  key={v.id}
                  className='ml-10 flex flex-wrap items-center gap-1 rounded border-l-2 border-brand/40 bg-panel-3/30 px-2 py-1'
                >
                  {v.moves.map((m) => (
                    <VariationButton
                      key={m.id}
                      move={m}
                      active={
                        currentVariation?.variationId === v.id &&
                        currentVariation?.ply === m.ply
                      }
                      onSelect={() =>
                        onSelectVariation(v.id, v.parentPly, m.ply)
                      }
                    />
                  ))}
                </div>
              )),
            )}
          </div>
        )
      })}
    </div>
  )
}
