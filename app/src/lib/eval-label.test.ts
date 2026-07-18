import { describe, expect, it } from 'vitest'
import {
  cpToMate,
  evalLabel,
  finalResultLabel,
  formatCp,
  formatMate,
  sideToMoveAtPly,
} from './eval-label'

describe('cpToMate', () => {
  it('recupera mate do cp sentinela (saída de scoreToCp)', () => {
    expect(cpToMate(99997)).toBe(3)
    expect(cpToMate(-99995)).toBe(-5)
  })

  it('devolve 0 para xeque-mate já consumado (cp ±100000)', () => {
    expect(cpToMate(100000)).toBe(0)
    expect(cpToMate(-100000)).toBe(0)
  })

  it('devolve null para cp normal (abaixo da faixa sentinela)', () => {
    expect(cpToMate(140)).toBeNull()
    expect(cpToMate(-800)).toBeNull()
    expect(cpToMate(0)).toBeNull()
  })

  it('fronteira: 98999 ainda é nota, 99000 já é mate (em 1000)', () => {
    expect(cpToMate(98999)).toBeNull()
    expect(cpToMate(99000)).toBe(1000)
  })
})

describe('formatCp', () => {
  it('formata peões com sinal e uma casa decimal', () => {
    expect(formatCp(140)).toBe('+1.4')
    expect(formatCp(-80)).toBe('-0.8')
  })

  it("zero vira '0.0' (sem '+0.0')", () => {
    expect(formatCp(0)).toBe('0.0')
  })

  it('acima de 10 peões usa inteiro (cabe na barra)', () => {
    expect(formatCp(1500)).toBe('+15')
    expect(formatCp(-2700)).toBe('-27')
  })
})

describe('formatMate', () => {
  it("brancas dão mate: 'M3'", () => {
    expect(formatMate(3)).toBe('M3')
  })

  it("pretas dão mate: '-M7'", () => {
    expect(formatMate(-7)).toBe('-M7')
  })
})

describe('finalResultLabel', () => {
  // Fool's mate: pretas dão mate, brancas a jogar.
  const FOOLS_MATE =
    'rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3'
  // Mate das pretas com lance das brancas: brancas aplicarão mate → "1-0".
  // Scholar's mate final: pretas a jogar e em xeque-mate.
  const SCHOLAR_MATE =
    'r1bqkb1r/pppp1Qpp/2n2n2/4p3/2B1P3/8/PPPP1PPP/RNB1K1NR b KQkq - 0 4'
  // Afogamento clássico: rei branco + dama vs rei preto encurralado.
  const STALEMATE = '7k/5Q2/6K1/8/8/8/8/8 b - - 0 1'
  const NORMAL = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'

  it("xeque-mate das pretas → '0-1'", () => {
    expect(finalResultLabel(FOOLS_MATE)).toBe('0-1')
  })

  it("xeque-mate das brancas → '1-0'", () => {
    expect(finalResultLabel(SCHOLAR_MATE)).toBe('1-0')
  })

  it("afogamento → '½-½'", () => {
    expect(finalResultLabel(STALEMATE)).toBe('½-½')
  })

  it('posição normal → null', () => {
    expect(finalResultLabel(NORMAL)).toBeNull()
  })
})

describe('sideToMoveAtPly', () => {
  // positions[ply] é a posição APÓS ply lances; lado a jogar = cor do próximo lance.
  const moves = [
    { color: 'w' as const },
    { color: 'b' as const },
    { color: 'w' as const },
    { color: 'b' as const },
  ]

  it('no início (ply 0) é a cor do primeiro lance', () => {
    expect(sideToMoveAtPly(moves, 0)).toBe('w')
  })

  it('após lance das brancas, pretas a jogar', () => {
    expect(sideToMoveAtPly(moves, 1)).toBe('b')
    expect(sideToMoveAtPly(moves, 3)).toBe('b')
  })

  it('na última posição (ply == len) é o oposto do último lance', () => {
    expect(sideToMoveAtPly(moves, 4)).toBe('w')
  })

  it('lista vazia → brancas (default)', () => {
    expect(sideToMoveAtPly([], 0)).toBe('w')
  })
})

describe('evalLabel', () => {
  const NORMAL_FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
  const FOOLS_MATE =
    'rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3'

  it('posição terminal mostra o resultado final (prioridade sobre mate/nota)', () => {
    expect(evalLabel(-100000, FOOLS_MATE, 'w')).toBe('0-1')
  })

  it("mate em N (cp sentinela) mostra 'M3' do POV das brancas", () => {
    // cp 99997 do POV do lado a jogar (brancas) → mate brancas em 3.
    expect(evalLabel(99997, NORMAL_FEN, 'w')).toBe('M3')
  })

  it("mate das pretas: cp negativo do lado a jogar pretas vira '-M5'", () => {
    // stm pretas, cp 99995 (pretas mate em 5) → POV brancas é -99995 → "-M5".
    expect(evalLabel(99995, NORMAL_FEN, 'b')).toBe('-M5')
  })

  it("nota comum: '+1.4' do POV das brancas", () => {
    expect(evalLabel(140, NORMAL_FEN, 'w')).toBe('+1.4')
    expect(evalLabel(-80, NORMAL_FEN, 'b')).toBe('+0.8')
  })
})
