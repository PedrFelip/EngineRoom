import type { RemoteGame } from './chess-com-import'

export function normalizeChessComUsername(input: unknown): string | null {
  if (typeof input !== 'string') return null
  const username = input.trim().toLowerCase()
  return /^[a-z0-9_-]{1,50}$/.test(username) ? username : null
}

/** Catalog presentation only; review metadata still comes from the PGN. */
export function chessComGamePerspective(game: RemoteGame, username: string) {
  const name = normalizeChessComUsername(username)
  const color =
    name === normalizeChessComUsername(game.white.username)
      ? ('white' as const)
      : name === normalizeChessComUsername(game.black.username)
        ? ('black' as const)
        : null
  if (!color) {
    return {
      opponent: null,
      color: null,
      outcome: 'neutral' as const,
      label: game.result,
    }
  }
  const opponent = color === 'white' ? game.black : game.white
  if (game.result === '1/2-1/2') {
    return { opponent, color, outcome: 'draw' as const, label: 'Empate' }
  }
  const won = (game.result === '1-0') === (color === 'white')
  return {
    opponent,
    color,
    outcome: won ? ('win' as const) : ('loss' as const),
    label: won ? 'Vitória' : 'Derrota',
  }
}
