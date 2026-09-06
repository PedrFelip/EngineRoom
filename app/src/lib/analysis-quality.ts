import type { PositionAnalysis } from '../types'

/** Profundidade alcançada primeiro; em empate, prefira refino e cobertura.
 * Tempo pedido só desempata buscas com os mesmos parâmetros.
 */
export function preferredAnalysis(
  previous: PositionAnalysis | undefined,
  incoming: PositionAnalysis,
): PositionAnalysis {
  if (!previous || previous.fen !== incoming.fen) return incoming
  if (previous.depth !== incoming.depth) {
    return incoming.depth > previous.depth ? incoming : previous
  }
  const oldSearch = previous.search
  const newSearch = incoming.search
  if (oldSearch?.purpose !== newSearch?.purpose) {
    if (newSearch?.purpose === 'playback') return previous
    if (oldSearch?.purpose === 'playback') return incoming
  }
  if (previous.lines.length !== incoming.lines.length) {
    return incoming.lines.length > previous.lines.length ? incoming : previous
  }
  if (
    oldSearch &&
    newSearch &&
    oldSearch.multipv === newSearch.multipv &&
    oldSearch.movetimeMs > newSearch.movetimeMs
  )
    return previous
  return incoming
}
