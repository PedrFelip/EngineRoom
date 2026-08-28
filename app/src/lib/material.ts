/**
 * Utilidades de material: saldo de peças a partir do FEN e delta material de
 * um lance jogado incluindo a recaptura esperada (melhor resposta do
 * oponente na PV). Puro — chess.js entra apenas como motor de regras para
 * aplicar lances (roque, en passant, promoção) sem reimplementá-los.
 */

import { Chess } from 'chess.js'

/** Valores padrão de peças em peões (rei = 0, nunca é capturado). */
const PIECE_VALUES: Record<string, number> = {
  p: 1,
  n: 3,
  b: 3,
  r: 5,
  q: 9,
  k: 0,
}

/**
 * Saldo material (em peões, positivo para as brancas) a partir do placement
 * do FEN. Ex.: posição inicial devolve 0; brancas sem a dama devolvem -9.
 */
export function materialBalance(fen: string): number {
  let balance = 0
  for (const ch of fen.split(' ')[0]) {
    const value = PIECE_VALUES[ch.toLowerCase()]
    if (value === undefined) continue
    balance += ch >= 'A' && ch <= 'Z' ? value : -value
  }
  return balance
}

/**
 * Delta material (em peões) do ponto de vista de quem jogou, após o lance e a
 * melhor resposta do oponente: positivo = ganhou material, negativo = entregou
 * material (candidato a sacrifício). Resposta nula (xeque-mate/afogamento na
 * jogada) conta só o lance jogado. Devolve 0 se algum lance for ilegal.
 */
export function materialDeltaAfterReplies(
  fenBefore: string,
  playedUci: string,
  replyUci: string | null,
): number {
  try {
    const chess = new Chess(fenBefore)
    chess.move({
      from: playedUci.slice(0, 2),
      to: playedUci.slice(2, 4),
      promotion: playedUci[4],
    })
    if (replyUci) {
      chess.move({
        from: replyUci.slice(0, 2),
        to: replyUci.slice(2, 4),
        promotion: replyUci[4],
      })
    }
    const moverIsWhite = fenBefore.split(' ')[1] !== 'b'
    const delta = materialBalance(chess.fen()) - materialBalance(fenBefore)
    return moverIsWhite ? delta : -delta
  } catch {
    return 0
  }
}
