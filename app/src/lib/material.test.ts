import { describe, expect, it } from 'vitest'
import { materialBalance, materialDeltaAfterReplies } from './material'

const START_FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
const WHITE_NO_QUEEN =
  'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR w KQkq - 0 1'
const SAC_BEFORE =
  'rnbqkbnr/pppppppp/8/8/2B1P3/8/PPPP1PPP/RNBQK1NR w KQkq - 0 1'
const BLACK_SAC_BEFORE =
  'rnbqk1nr/pppppppp/8/2b1p3/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1'
const TRADE_BEFORE =
  'rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 3'

describe('materialBalance', () => {
  it('posição inicial tem saldo zero', () => {
    expect(materialBalance(START_FEN)).toBe(0)
  })

  it('brancas sem a dama devolvem -9', () => {
    expect(materialBalance(WHITE_NO_QUEEN)).toBe(-9)
  })

  it('soma corretamente peões e peças dos dois lados', () => {
    // brancas sem o peão do rei: 2T+2C+2B+D+7P = 38; pretas completas = 39
    expect(
      materialBalance('rnbqkbnr/pppppppp/8/8/8/5N2/PPPP1PPP/RNBQKB1R'),
    ).toBe(38 - 39)
  })
})

describe('materialDeltaAfterReplies', () => {
  it('sacrifício de bispo por peão com recaptura = -2 (Bxf7+ Kxf7)', () => {
    expect(materialDeltaAfterReplies(SAC_BEFORE, 'c4f7', 'e8f7')).toBe(-2)
  })

  it('capitura sem recaptura é ganho material (+1)', () => {
    expect(materialDeltaAfterReplies(TRADE_BEFORE, 'e4d5', 'g8f6')).toBe(1)
  })

  it('troca simétrica de peões tem delta zero (exd5 Qxd5)', () => {
    expect(materialDeltaAfterReplies(TRADE_BEFORE, 'e4d5', 'd8d5')).toBe(0)
  })

  it('espelha o ponto de vista para quem joga de pretas (Bxf2+ Kxf2)', () => {
    expect(materialDeltaAfterReplies(BLACK_SAC_BEFORE, 'c5f2', 'e1f2')).toBe(-2)
  })

  it('resposta nula (mate na jogada) conta só o lance jogado', () => {
    // Bxf7+ sem recaptura: só o peão capturado
    expect(materialDeltaAfterReplies(SAC_BEFORE, 'c4f7', null)).toBe(1)
  })

  it('lance ilegal devolve 0 em vez de lançar', () => {
    expect(materialDeltaAfterReplies(START_FEN, 'e1e3', null)).toBe(0)
  })
})
