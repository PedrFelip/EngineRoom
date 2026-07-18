/**
 * Som de movimentação de peças.
 *
 * Classifica um lance (SAN) em mover ou capturar e toca o arquivo
 * correspondente em `public/sounds/`.
 *
 * Conjunto de sons: "Standard" do lichess (CC0). Esse conjunto não possui
 * arquivos distintos para Castle, Promote, Check nem Mate; todo lance
 * não-capturante usa o som de Move.
 *
 * Mapeamento tipo → arquivo:
 *   move    → Move.mp3
 *   capture → Capture.mp3
 */

/** Categoria de som associada a um lance. */
export type SoundType = 'move' | 'capture'

/** Função que de fato toca um som (abstrai o Audio API, injetável em testes). */
export type PlaySound = (type: SoundType, volume: number) => void

/**
 * Classifica um lance (notação SAN) no tipo de som correspondente.
 *
 * Apenas capturas (`x` no SAN) recebem som distinto; todo o resto
 * (lance silencioso, roque, promoção, xeque, mate) usa o som de Move.
 */
export function classifyMove(san: string): SoundType {
  if (san.includes('x')) return 'capture'
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
