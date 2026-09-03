import { GitBranch } from 'lucide-react'
import { memo, useEffect, useMemo, useRef } from 'react'
import type { ReviewVariation } from '../lib/review-store'
import type { MoveAnalysis } from '../types'
import ClassificationBadge from './ClassificationBadge'

interface MoveListProps {
  moves: MoveAnalysis[]
  currentPly: number
  onSelect: (ply: number) => void
  onBranchFrom: (ply: number) => void
  variations?: ReviewVariation[]
  activeVariation?: ReviewVariation | null
  onSelectVariation?: (variationId: string, path: string[]) => void
}

interface Row {
  num: number
  white?: MoveAnalysis
  black?: MoveAnalysis
}

const MoveButton = memo(function MoveButton({
  move,
  active,
  onSelect,
  onBranchFrom,
}: {
  move: MoveAnalysis
  active: boolean
  onSelect: (ply: number) => void
  onBranchFrom: (ply: number) => void
}) {
  return (
    <div className='group/move flex min-w-0 flex-1 items-center'>
      <button
        type='button'
        data-active={active ? 'true' : undefined}
        onClick={() => onSelect(move.ply)}
        className={`flex min-h-8 min-w-0 flex-1 items-center gap-1.5 rounded-[calc(var(--radius)-3px)] px-2 text-left font-mono text-[13px] transition-colors ${
          active
            ? 'bg-accent text-accent-foreground shadow-sm ring-1 ring-border'
            : 'text-ink-dim hover:bg-accent/70 hover:text-ink'
        }`}
      >
        <ClassificationBadge classification={move.classification} />
        <span>{move.san}</span>
      </button>
      <button
        type='button'
        onClick={() => onBranchFrom(move.ply - 1)}
        className='ml-0.5 flex size-7 shrink-0 items-center justify-center rounded-md text-ink-faint opacity-60 transition hover:bg-brand/10 hover:text-brand group-hover/move:opacity-100'
        aria-label={`Criar alternativa a ${move.san}`}
        title={`Criar alternativa antes de ${move.san}`}
      >
        <GitBranch size={12} aria-hidden='true' />
      </button>
    </div>
  )
})

export default function MoveList({
  moves,
  currentPly,
  onSelect,
  onBranchFrom,
  variations = [],
  activeVariation = null,
  onSelectVariation,
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
  }, [currentPly, activeVariation?.path])

  const rows = useMemo(() => {
    const nextRows: Row[] = []
    moves.forEach((move) => {
      const num = Math.ceil(move.ply / 2)
      if (!nextRows[num - 1]) nextRows[num - 1] = { num }
      if (move.color === 'w') nextRows[num - 1].white = move
      else nextRows[num - 1].black = move
    })
    const lastVariationRow = variations.reduce(
      (last, variation) =>
        Math.max(last, Math.ceil((variation.basePly + 1) / 2)),
      0,
    )
    while (nextRows.length < lastVariationRow) {
      nextRows.push({ num: nextRows.length + 1 })
    }
    return nextRows
  }, [moves, variations])

  return (
    <div ref={rootRef} className='flex flex-col gap-0.5'>
      {rows.map((row) => (
        <div key={row.num}>
          {row.white || row.black ? (
            <div className='flex items-center gap-1.5'>
              <span className='flex size-7 shrink-0 items-center justify-center rounded-[calc(var(--radius)-3px)] bg-muted/60 font-mono text-[10px] text-muted-foreground'>
                {row.num}.
              </span>
              {row.white ? (
                <MoveButton
                  move={row.white}
                  active={row.white.ply === currentPly}
                  onSelect={onSelect}
                  onBranchFrom={onBranchFrom}
                />
              ) : (
                <span className='flex-1' />
              )}
              {row.black ? (
                <MoveButton
                  move={row.black}
                  active={row.black.ply === currentPly}
                  onSelect={onSelect}
                  onBranchFrom={onBranchFrom}
                />
              ) : (
                <span className='flex-1' />
              )}
            </div>
          ) : null}
          {variations
            .filter(
              (variation) => Math.ceil((variation.basePly + 1) / 2) === row.num,
            )
            .map((variation) => (
              <VariationLine
                key={variation.id}
                variation={variation}
                active={variation.id === activeVariation?.id}
                onSelect={onSelectVariation}
              />
            ))}
        </div>
      ))}
    </div>
  )
}

function VariationLine({
  variation,
  active,
  onSelect,
}: {
  variation: ReviewVariation
  active: boolean
  onSelect?: (variationId: string, path: string[]) => void
}) {
  return (
    <div className='my-1 ml-4 sm:ml-8'>
      <div className='flex flex-col gap-1.5'>
        {variation.roots.map((root) => (
          <VariationBlock
            key={root.id}
            start={root}
            parentPath={[]}
            basePly={variation.basePly}
            activePath={variation.path}
            active={active}
            onSelect={(path) => onSelect?.(variation.id, path)}
          />
        ))}
      </div>
    </div>
  )
}

interface VariationBlockProps {
  start: ReviewVariation['roots'][number]
  parentPath: string[]
  basePly: number
  activePath: string[]
  active: boolean
  onSelect?: (path: string[]) => void
}

interface SegmentMove {
  move: ReviewVariation['roots'][number]
  path: string[]
  ply: number
}

interface ChildBranch {
  move: ReviewVariation['roots'][number]
  parentPath: string[]
}

function buildSegment(
  start: ReviewVariation['roots'][number],
  parentPath: string[],
  basePly: number,
): { moves: SegmentMove[]; branches: ChildBranch[] } {
  const moves: SegmentMove[] = []
  const branches: ChildBranch[] = []
  let move: ReviewVariation['roots'][number] | undefined = start
  let path = parentPath

  while (move) {
    path = [...path, move.id]
    moves.push({ move, path, ply: basePly + path.length })
    for (const alternative of move.children.slice(1)) {
      branches.push({ move: alternative, parentPath: path })
    }
    const continuation: ReviewVariation['roots'][number] | undefined =
      move.children[0]
    if (!continuation) break
    move = continuation
  }

  return { moves, branches }
}

function samePath(first: string[], second: string[]): boolean {
  return (
    first.length === second.length &&
    first.every((part, index) => part === second[index])
  )
}

function pathStartsWith(path: string[], prefix: string[]): boolean {
  return prefix.every((part, index) => path[index] === part)
}

function movePrefix(ply: number, firstInBlock: boolean): string {
  const moveNumber = Math.ceil(ply / 2)
  if (ply % 2 === 1) return `${moveNumber}.`
  return firstInBlock ? `${moveNumber}...` : ''
}

function VariationBlock({
  start,
  parentPath,
  basePly,
  activePath,
  active,
  onSelect,
}: VariationBlockProps) {
  const segment = buildSegment(start, parentPath, basePly)
  const blockPath = segment.moves[0]?.path ?? []
  const activeBlock = active && pathStartsWith(activePath, blockPath)

  return (
    <div
      className={`rounded-lg border p-1.5 transition-colors ${
        activeBlock
          ? 'border-brand/35 bg-brand/6'
          : 'border-edge-soft bg-panel-2/55'
      }`}
    >
      <div className='flex flex-wrap items-center gap-1'>
        {segment.moves.map(({ move, path, ply }, index) => {
          const prefix = movePrefix(ply, index === 0)
          const activeMove = active && samePath(path, activePath)
          return (
            <button
              key={move.id}
              type='button'
              onClick={() => onSelect?.(path)}
              data-active={activeMove ? 'true' : undefined}
              className={`rounded-md px-2 py-1.5 font-mono text-xs transition-colors ${
                activeMove
                  ? 'bg-brand text-bg shadow-sm'
                  : 'text-ink-dim hover:bg-accent hover:text-ink'
              }`}
            >
              {prefix ? (
                <span
                  className={`mr-1 text-[10px] ${
                    activeMove ? 'text-bg/65' : 'text-ink-faint'
                  }`}
                >
                  {prefix}
                </span>
              ) : null}
              {move.san}
            </button>
          )
        })}
      </div>
      {segment.branches.length > 0 ? (
        <div className='mt-1.5 flex flex-col gap-1.5 border-l border-brand/25 pl-2 sm:pl-3'>
          {segment.branches.map((branch) => (
            <VariationBlock
              key={branch.move.id}
              start={branch.move}
              parentPath={branch.parentPath}
              basePly={basePly}
              activePath={activePath}
              active={active}
              onSelect={onSelect}
            />
          ))}
        </div>
      ) : null}
    </div>
  )
}
