import { BarChart3, Gauge, Swords } from 'lucide-react'
import { memo, type ReactNode } from 'react'
import { CLASSIFICATION_LABELS } from '../lib/scoring'
import type { Classification, Phase, ReviewResult } from '../types'
import { ClassGlyph } from './ClassificationBadge'
import { Badge } from './ui/badge'
import { Card } from './ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from './ui/table'

const ORDER: Classification[] = [
  'livro',
  'melhor',
  'excelente',
  'bom',
  'imprecisao',
  'erro',
  'blunder',
]

const PHASES: Phase[] = ['opening', 'middlegame', 'endgame']
const PHASE_LABELS: Record<Phase, string> = {
  opening: 'Abertura',
  middlegame: 'Meio-jogo',
  endgame: 'Final',
}
function countBy(moves: { classification: Classification }[]): number[] {
  return ORDER.map(
    (classification) =>
      moves.filter((move) => move.classification === classification).length,
  )
}

interface ReviewSummaryProps {
  result: ReviewResult
}

const ReviewSummary = memo(function ReviewSummary({
  result,
}: ReviewSummaryProps) {
  const whiteMoves = result.moves.filter((move) => move.color === 'w')
  const blackMoves = result.moves.filter((move) => move.color === 'b')
  const whiteCounts = countBy(whiteMoves)
  const blackCounts = countBy(blackMoves)

  return (
    <Card className='overflow-hidden bg-card/75 shadow-sm'>
      <div className='flex items-start justify-between gap-4 border-b border-border px-4 py-4'>
        <div>
          <h2 className='text-sm font-semibold text-foreground'>
            Resumo da revisão
          </h2>
          <p className='mt-1.5 text-xs text-muted-foreground'>
            Qualidade dos lances por jogador e fase
          </p>
        </div>
        <Badge variant='outline' className='mt-0.5 font-mono tabular-nums'>
          {Math.ceil(result.moves.length / 2)} lances
        </Badge>
      </div>

      <AccuracyScoreboard accuracy={result.accuracy} />

      <section className='border-t border-border px-4 py-3'>
        <SectionTitle icon={Swords} title='Acurácia por fase' />
        <div className='mt-2.5 overflow-hidden rounded-lg border border-border/70 bg-background/35'>
          <div className='grid grid-cols-[minmax(0,1fr)_3.5rem_3.5rem] items-center border-b border-border/70 px-3 py-1.5 text-[9px] font-semibold tracking-[0.12em] text-muted-foreground uppercase'>
            <span>Fase</span>
            <span className='text-center'>Br.</span>
            <span className='text-center'>Pr.</span>
          </div>
          {PHASES.map((phase) => (
            <PhaseAccuracy
              key={phase}
              label={PHASE_LABELS[phase]}
              white={result.accuracyByPhase[phase].white}
              black={result.accuracyByPhase[phase].black}
            />
          ))}
        </div>
      </section>

      <section className='border-t border-border px-4 py-3'>
        <SectionTitle icon={BarChart3} title='Qualidade dos lances' />
        <SummaryTable>
          {ORDER.map((classification, index) => (
            <ClassificationCount
              key={classification}
              classification={classification}
              white={whiteCounts[index]}
              black={blackCounts[index]}
            />
          ))}
        </SummaryTable>
      </section>
    </Card>
  )
})

export default ReviewSummary

function AccuracyScoreboard({
  accuracy,
}: {
  accuracy: { white: number; black: number }
}) {
  return (
    <section className='px-4 py-4' aria-label='Acurácia geral'>
      <SectionTitle icon={Gauge} title='Acurácia geral' />
      <div className='mt-3 grid grid-cols-2 overflow-hidden rounded-lg shadow-sm'>
        <AccuracyPanel side='white' value={accuracy.white} />
        <AccuracyPanel side='black' value={accuracy.black} />
      </div>
    </section>
  )
}

function AccuracyPanel({
  side,
  value,
}: {
  side: 'white' | 'black'
  value: number
}) {
  const isWhite = side === 'white'
  return (
    <div
      className={
        isWhite
          ? 'flex h-20 min-w-0 items-center justify-center bg-white px-3 text-center text-black'
          : 'flex h-20 min-w-0 items-center justify-center bg-black px-3 text-center text-white'
      }
    >
      <span className='font-mono text-2xl font-bold leading-none tabular-nums'>
        {value.toFixed(1)}%
      </span>
    </div>
  )
}

function SectionTitle({
  icon: Icon,
  title,
}: {
  icon: typeof BarChart3
  title: string
}) {
  return (
    <h3 className='flex items-center gap-1.5 text-[10px] font-semibold tracking-[0.11em] text-muted-foreground uppercase'>
      <Icon size={13} strokeWidth={2.2} aria-hidden='true' />
      {title}
    </h3>
  )
}

function SummaryTable({ children }: { children: ReactNode }) {
  return (
    <Table className='mt-2.5 table-fixed text-xs'>
      <colgroup>
        <col />
        <col className='w-[3.25rem]' />
        <col className='w-[3.25rem]' />
      </colgroup>
      <TableHeader className='[&_tr]:border-0'>
        <TableRow className='border-0 hover:bg-transparent'>
          <TableHead className='h-auto p-0' scope='col' />
          <TableHead
            className='h-auto p-0 text-center text-[10px] font-medium tracking-wide uppercase'
            scope='col'
          >
            Br.
          </TableHead>
          <TableHead
            className='h-auto p-0 text-center text-[10px] font-medium tracking-wide uppercase'
            scope='col'
          >
            Pr.
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>{children}</TableBody>
    </Table>
  )
}

function PhaseAccuracy({
  label,
  white,
  black,
}: {
  label: string
  white: number
  black: number
}) {
  return (
    <div className='grid grid-cols-[minmax(0,1fr)_3.5rem_3.5rem] items-center border-b border-border/60 px-3 py-2.5 last:border-b-0'>
      <span className='flex min-w-0 items-center gap-2.5 text-xs font-medium text-foreground'>
        <span className='h-4 w-0.5 shrink-0 rounded-full bg-foreground/45' />
        <span className='truncate'>{label}</span>
      </span>
      <span className='text-center font-mono text-xs font-semibold tabular-nums text-foreground'>
        {white.toFixed(0)}%
      </span>
      <span className='text-center font-mono text-xs font-semibold tabular-nums text-foreground'>
        {black.toFixed(0)}%
      </span>
    </div>
  )
}

function ClassificationCount({
  classification,
  white,
  black,
}: {
  classification: Classification
  white: number
  black: number
}) {
  return (
    <TableRow className='border-0 hover:bg-transparent'>
      <TableCell className='p-0 pr-2'>
        <span className='flex min-w-0 items-center gap-2 text-muted-foreground'>
          <ClassGlyph classification={classification} />
          <span className='truncate'>
            {CLASSIFICATION_LABELS[classification]}
          </span>
        </span>
      </TableCell>
      <Count value={white} />
      <Count value={black} />
    </TableRow>
  )
}

function Count({ value }: { value: number }) {
  return (
    <TableCell className='p-0 py-0.5 text-center font-mono tabular-nums text-foreground'>
      {value}
    </TableCell>
  )
}
