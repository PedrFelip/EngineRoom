/**
 * Som de movimentação de peças.
 *
 * Classifica um lance (SAN) num tipo de som (mover, capturar, xeque, xeque-mate)
 * e toca o arquivo correspondente em `public/sounds/`.
 *
 * Conjunto de sons: "Standard" do lichess (CC0). Esse conjunto não possui arquivos
 * distintos para Castle (`O-O`) nem Promote (`=`); nesses casos o som é o mesmo
 * de um lance silencioso (Move).
 *
 * Mapeamento tipo → arquivo:
 *   move    → Move.mp3
 *   capture → Capture.mp3
 *   check   → Check.mp3
 *   mate    → Checkmate.mp3
 */

/** Categoria de som associada a um lance. */
export type SoundType = 'move' | 'capture' | 'check' | 'mate'

/** Função que de fato toca um som (abstrai o Audio API, injetável em testes). */
export type PlaySound = (type: SoundType, volume: number) => void

/**
 * Classifica um lance (notação SAN) no tipo de som correspondente.
 *
 * Prioridade quando o lance combina marcadores (ex.: `Qxe7#` é captura e mate):
 * mate > captura > xeque > silencioso.
 *
 * Observação: o conjunto "Standard" do lichess não possui arquivos distintos
 * para Castle (`O-O`) nem Promote (`=`); nesses casos o lance é classificado
 * conforme os demais marcadores presentes (default = move). O mate (`#`) é
 * mapeado para `Checkmate.mp3` — atualmente idêntico a `Check.mp3` neste
 * conjunto, mas o tipo fica reservado para futura troca de som.
 */
export function classifyMove(san: string): SoundType {
  if (san.includes('#')) return 'mate'
  if (san.includes('x')) return 'capture'
  if (san.includes('+')) return 'check'
  return 'move'
}

/**
 * Classifica o lance (SAN) e toca o som correspondente via `play`.
 *
 * `play` é injetável (default = player Web Audio real em `public/sounds/`).
 * Essa injeção mantém a função testável sem precisar mockar o Audio API.
 */
export function playMoveSound(
  san: string,
  volume: number,
  play: PlaySound = defaultPlay,
): void {
  play(classifyMove(san), volume)
}

/** Caminho do arquivo de áudio para cada tipo de som. */
const SOUND_FILE: Record<SoundType, string> = {
  move: '/sounds/Move.mp3',
  capture: '/sounds/Capture.mp3',
  check: '/sounds/Check.mp3',
  mate: '/sounds/Checkmate.mp3',
}

/**
 * Player Web Audio (fronteira de sistema). Cacheia um `<audio>` por tipo para
 * evitar re-decodificar a cada jogada. Volume é clampado em [0, 1].
 *
 * Erros de autoplay (sem gesto prévio do usuário) são silenciados: o webview
 * do Tauri geralmente libera o áudio após o primeiro clique/toque.
 */
const audioCache = new Map<SoundType, HTMLAudioElement>()

export const defaultPlay: PlaySound = (type, volume) => {
  if (typeof Audio === 'undefined') return
  let audio = audioCache.get(type)
  if (!audio) {
    audio = new Audio(SOUND_FILE[type])
    audioCache.set(type, audio)
  }
  audio.volume = Math.max(0, Math.min(1, volume))
  audio.currentTime = 0
  audio.play().catch(() => {
    /* autoplay bloqueado — ignora */
  })
}
