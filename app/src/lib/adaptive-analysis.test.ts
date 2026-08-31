import { describe, expect, it } from 'vitest'
import {
  ADAPTIVE_PROFILES,
  rankCriticalMoves,
  selectRefinementTargets,
} from './adaptive-analysis'
import type { PlayedMove, RawPosition } from './analyze'

const SAC_BEFORE =
  'rnbqkbnr/pppppppp/8/8/2B1P3/8/PPPP1PPP/RNBQK1NR w KQkq - 0 1'
const SAC_AFTER = 'rnbqkbnr/ppppBppp/8/8/4P3/8/PPPP1PPP/RNBQK1NR b KQkq - 0 1'

const move: PlayedMove = {
  ply: 1,
  color: 'w',
  san: 'Bxf7+',
  uci: 'c4f7',
  fenBefore: SAC_BEFORE,
}

const raw: RawPosition[] = [
  {
    fen: SAC_BEFORE,
    cp: 30,
    depth: 12,
    pv: ['c4f7'],
    lines: [
      { multipv: 1, cp: 30, pv: ['c4f7'] },
      { multipv: 2, cp: 10, pv: ['d2d4'] },
    ],
  },
  {
    fen: SAC_AFTER,
    cp: -30,
    depth: 12,
    pv: ['e8f7'],
    lines: [
      { multipv: 1, cp: -30, pv: ['e8f7'] },
      { multipv: 2, cp: -20, pv: ['e8e7'] },
    ],
  },
]

describe('política de análise adaptativa', () => {
  it('marca sacrifício correto como candidato duro a refinamento', () => {
    const [critical] = rankCriticalMoves([move], raw)

    expect(critical.hard).toBe(true)
    expect(critical.brilliantCandidate).toBe(true)
    expect(critical.reasons).toContain('candidato a brilhante')
  })

  it('refina somente as posições antes e depois do lance selecionado', () => {
    const critical = rankCriticalMoves([move], raw)
    const targets = selectRefinementTargets(
      critical,
      raw.length,
      ADAPTIVE_PROFILES.fast,
    )

    expect(targets.map((target) => target.positionIndex).sort()).toEqual([0, 1])
    expect(targets.every((target) => target.budget === 'high')).toBe(true)
  })

  it('marca como duro um candidato a único lance bom', () => {
    const candidateRaw: RawPosition[] = [
      {
        ...raw[0],
        cp: 0,
        lines: [
          { multipv: 1, cp: 0, pv: ['c4f7'] },
          { multipv: 2, cp: -200, pv: ['d2d4'] },
        ],
      },
      { ...raw[1], cp: 0 },
    ]
    const [critical] = rankCriticalMoves([move], candidateRaw)

    expect(critical.greatCandidate).toBe(true)
    expect(critical.hard).toBe(true)
    expect(critical.reasons).toContain('candidato a ótimo')
  })

  it('não aprofunda lance que ainda pertence ao livro', () => {
    const [critical] = rankCriticalMoves([move], raw, 1)

    expect(critical.score).toBe(0)
    expect(
      selectRefinementTargets([critical], raw.length, ADAPTIVE_PROFILES.deep),
    ).toEqual([])
  })

  it('usa MultiPV maior já na triagem dos dois perfis', () => {
    expect(ADAPTIVE_PROFILES.fast.triageMultipv).toBeGreaterThan(1)
    expect(ADAPTIVE_PROFILES.deep.triageMultipv).toBeGreaterThan(
      ADAPTIVE_PROFILES.fast.triageMultipv,
    )
  })
})
