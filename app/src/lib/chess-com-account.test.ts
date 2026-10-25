import { describe, expect, it } from 'vitest'
import { chessComGamePerspective } from './chess-com-account'
import type { RemoteGame } from './chess-com-import'

const game: RemoteGame = {
  url: 'https://www.chess.com/game/live/1',
  endTime: 1700000000,
  timeClass: 'blitz',
  white: { username: 'Alice', rating: 1500 },
  black: { username: 'Bob', rating: 1600 },
  result: '1-0',
  pgn: '1. e4 e5 1-0',
}
describe('perspectiva da partida do Chess.com', () => {
  it('identifica adversário, cor e resultado para ambos os lados', () => {
    expect(chessComGamePerspective(game, 'ALICE')).toEqual({
      opponent: game.black,
      color: 'white',
      outcome: 'win',
      label: 'Vitória',
    })
    expect(chessComGamePerspective(game, 'bob')).toEqual({
      opponent: game.white,
      color: 'black',
      outcome: 'loss',
      label: 'Derrota',
    })
    expect(
      chessComGamePerspective({ ...game, result: '0-1' }, 'bob').outcome,
    ).toBe('win')
    expect(
      chessComGamePerspective({ ...game, result: '1/2-1/2' }, 'alice').label,
    ).toBe('Empate')
  })

  it('não atribui vitória ao jogador quando o username não coincide', () => {
    expect(chessComGamePerspective(game, 'outro')).toEqual({
      opponent: null,
      color: null,
      outcome: 'neutral',
      label: '1-0',
    })
  })
})
