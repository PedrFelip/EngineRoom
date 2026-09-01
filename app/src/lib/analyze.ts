/** Pipeline de análise: núcleo puro para a revisão e I/O via EnginePort. */

import { Chess } from 'chess.js'
import type {
  AccuracyByColor,
  MoveAnalysis,
  Phase,
  PositionAnalysis,
  PvLine,
  ReviewResult,
} from '../types'
import {
  ADAPTIVE_PROFILES,
  type AdaptiveProfileId,
  rankCriticalMoves,
  selectRefinementTargets,
} from './adaptive-analysis'
import { type EcoEntry, lookupOpening } from './eco'
import { computePhases } from './phase'
import {
  ACCURACY_MODEL_VERSION,
  centipawnLoss,
  classifyMove,
  cpToWinPct,
  gameAccuracy,
  sideToMoveAtPly,
  whiteCp,
  whiteWinPct,
} from './scoring'
import type { InfoScore } from './uci'
import { isReadyOk, isUciOk, parseInfo, scoreToCp } from './uci'

export interface BookInfo {
  maxPly: number
  eco: EcoEntry | null
}

export interface RawLine {
  multipv: number
  cp: number
  pv: string[]
  san?: string | null
  /** Profundidade (plies) que esta linha atingiu na busca. */
  depth?: number
}

export interface RawPosition {
  fen: string
  cp: number
  depth: number
  pv: string[]
  lines?: RawLine[]
}

export interface PlayedMove {
  ply: number
  color: 'w' | 'b'
  san: string
  uci: string
  fenBefore: string
}

export interface PlayedGame {
  startFen: string
  moves: PlayedMove[]
}

/**
 * Constrói a revisão a partir da partida jogada e das avaliações brutas por ply.
 * `raw[i]` é a avaliação da posição após o i-ésimo ply (raw[0] = posição inicial).
 * O win% das posições é normalizado para o ponto de vista das brancas.
 */
export function buildReview(
  game: PlayedGame,
  raw: RawPosition[],
  book?: BookInfo,
): ReviewResult {
  const phases = computePhases(raw.map((r) => ({ fen: r.fen })))

  const positions: PositionAnalysis[] = raw.map((r, i) => {
    const stm = sideToMoveAtPly(game.moves, i)
    const winPct = whiteWinPct(r.cp, stm)
    const rawLines = r.lines ?? [{ multipv: 1, cp: r.cp, pv: r.pv }]
    const lines: PvLine[] = rawLines.map((l) => ({
      multipv: l.multipv,
      san: l.san ?? null,
      cp: whiteCp(l.cp, stm),
      winPct: whiteWinPct(l.cp, stm),
      pv: l.pv,
    }))
    return {
      ply: i,
      fen: r.fen,
      phase: phases[i],
      depth: r.depth,
      cp: r.cp,
      winPct,
      pv: r.pv,
      lines,
    }
  })

  const moves: MoveAnalysis[] = game.moves.map((m) => {
    const before = raw[m.ply - 1]
    const after = raw[m.ply]
    const winPctBefore = cpToWinPct(before.cp)
    const winPctAfter = 100 - cpToWinPct(after.cp)
    const winPctLoss = Math.max(0, winPctBefore - winPctAfter)
    const cpLoss = centipawnLoss(before.cp, after.cp)
    const isBook = !!book && m.ply <= book.maxPly
    const classification = classifyMove(winPctLoss, isBook)
    return {
      ply: m.ply,
      color: m.color,
      san: m.san,
      uci: m.uci,
      fenBefore: m.fenBefore,
      classification,
      winPctBefore,
      winPctAfter,
      winPctLoss,
      cpLoss,
      bestUci: before.pv[0] ?? null,
      isBook,
      eco:
        isBook && book?.eco
          ? { code: book.eco.code, name: book.eco.name }
          : null,
    }
  })

  const positionWinPcts = positions.map((position) => position.winPct)
  const accuracy: AccuracyByColor = gameAccuracy(moves, positionWinPcts)

  const accuracyByPhase = accuracyByPhaseOf(moves, phases, positionWinPcts)

  return {
    positions,
    moves,
    accuracyModel: ACCURACY_MODEL_VERSION,
    accuracy,
    accuracyByPhase,
  }
}

/**
 * Acurácia agregada (0–100) por fase do jogo. Um lance pertence à fase da
 * posição de onde partiu (`phases[ply - 1]`). Cada fase reaplica o agregador
 * completo sobre seu trecho, seguindo `phaseAccuracies` do Lichess. Lances de
 * livro continuam incluídos.
 */
export function accuracyByPhaseOf(
  moves: MoveAnalysis[],
  phases: Phase[],
  positionWinPcts: number[],
): Record<Phase, AccuracyByColor> {
  const forPhase = (phase: Phase): AccuracyByColor => {
    const phaseMoves = moves.filter((move) => phases[move.ply - 1] === phase)
    if (phaseMoves.length === 0) return gameAccuracy([], [50])
    const firstPosition = phaseMoves[0].ply - 1
    const phaseWinPcts = [
      positionWinPcts[firstPosition],
      ...phaseMoves.map((move) => positionWinPcts[move.ply]),
    ]
    return gameAccuracy(phaseMoves, phaseWinPcts)
  }
  return {
    opening: forPhase('opening'),
    middlegame: forPhase('middlegame'),
    endgame: forPhase('endgame'),
  }
}

/** Motivo pelo qual o processo da engine encerrou (evento `engine://exit`). */
export interface EngineExitReason {
  /** Código de saída, quando conhecido (saída limpa ou sinal com código). */
  code: number | null
  /** Número do sinal que matou o processo, se houver (ex.: 11 = SIGSEGV). */
  signal: number | null
  /** Erro reportado pelo plugin de shell (falha de UTF-8/IO), se for o caso. */
  error?: string
}

/**
 * Interface do engine injetável, para testar com engine falso.
 * `onExit` é opcional: quando presente, `ask()` rejeita imediatamente se a
 * engine morrer, em vez de esperar o timeout completo.
 */
export interface EnginePort {
  send(cmd: string): void | Promise<void>
  onLine(handler: (line: string) => void): () => void
  onExit?(handler: (reason: EngineExitReason) => void): () => void
}

/**
 * Como a engine deve buscar cada posição:
 *  - `depth`: fixa em N ply (`go depth N`);
 *  - `time`: fixa em N ms (`go movetime N`), estilo chess.com "Maximum Time".
 */
export type AnalyzeControl =
  | { mode: 'depth'; depth: number }
  | { mode: 'time'; movetimeMs: number }

/**
 * Valor escalar usado como chave de cache: o `depth` no modo profundidade,
 * ou `movetimeMs` no modo tempo. A diferenciação entre modos é feita pelo
 * campo `mode` (também parte da chave), evitando colisão entre
 * `depth=20` e `movetimeMs=20`, por exemplo.
 */
export function controlKeyValue(control: AnalyzeControl): number {
  return control.mode === 'depth' ? control.depth : control.movetimeMs
}

/**
 * Orçamento padrão de timeout por posição para o comando `go`:
 *  - `depth`: busca sem limite inerente — 180s como rede de segurança contra
 *    engine travada (em hardware modesto, d25 pode chegar perto disso);
 *  - `time`: a engine se auto-limita a `movetimeMs`, então 3·N + 10s cobre
 *    folga de IPC/spawn.
 */
export function defaultGoTimeout(control: AnalyzeControl): number {
  return control.mode === 'depth' ? 180_000 : 3 * control.movetimeMs + 10_000
}

/** Modo de análise: por profundidade fixa ou por tempo fixo por lance. */
export type EngineMode = AnalyzeControl['mode']

export type AnalysisProgressStage =
  | 'analyzing'
  | 'triage'
  | 'refinement'
  | 'finalizing'

/** Atualização pontual do gráfico de avaliação (POV das brancas). */
export interface WinPctUpdate {
  index: number
  winPct: number
}

/** Metadados de progresso ricos para a UI. */
export interface AnalysisProgress {
  stage: AnalysisProgressStage
  completed: number
  total: number
  currentPly: number
  phase: Phase
  cachedPositions: number
  enginePositions: number
  /** Teto de tempo ainda orcado pelo modo `movetime`; ausente em depth. */
  remainingBudgetMs?: number
}

/**
 * Cache de avaliações por posição, chaveado por (fen, mode, value, multipv),
 * onde `value` é `depth` (modo profundidade) ou `movetimeMs` (modo tempo).
 * `get` devolve null em caso de miss; `put` grava a avaliação alcançada.
 *
 * `getBulk`/`putMany` são os equivalentes em lote para uma mesma chave
 * (mode, value, multipv): o pipeline de análise faz uma única consulta de
 * prefetch e um único descarrego ao final, em vez de N chamadas seriais.
 */
export interface PositionCache {
  get(
    fen: string,
    mode: EngineMode,
    value: number,
    multipv: number,
  ): Promise<RawPosition | null>
  put(
    pos: RawPosition,
    mode: EngineMode,
    value: number,
    multipv: number,
  ): Promise<void>
  /** Prefetch dos hits para N fens, numa única chamada. Ordem preservada. */
  getBulk(
    fens: string[],
    mode: EngineMode,
    value: number,
    multipv: number,
  ): Promise<(RawPosition | null)[]>
  /** Grava N posições numa única chamada (transação). */
  putMany(
    entries: RawPosition[],
    mode: EngineMode,
    value: number,
    multipv: number,
  ): Promise<void>
}

interface ExtractedGame {
  positionFens: string[]
  moves: PlayedMove[]
}

function extractGame(pgn: string): ExtractedGame {
  const chess = new Chess()
  chess.loadPgn(pgn)
  const verbose = chess.history({ verbose: true })
  const replay = new Chess()
  const positionFens: string[] = [replay.fen()]
  const moves: PlayedMove[] = []
  verbose.forEach((m, i) => {
    const fenBefore = replay.fen()
    replay.move({ from: m.from, to: m.to, promotion: m.promotion })
    positionFens.push(replay.fen())
    moves.push({
      ply: i + 1,
      color: m.color,
      san: m.san,
      uci: m.from + m.to + (m.promotion ?? ''),
      fenBefore,
    })
  })
  return { positionFens, moves }
}

/** Faz o handshake UCI e configura Threads, Hash e MultiPV. */
export async function configureEngine(
  port: EnginePort,
  opts: {
    threads?: number
    hashMb?: number
    multipv: number
    timeoutMs?: number
  },
): Promise<void> {
  const timeoutMs = opts.timeoutMs ?? 10_000
  await ask(port, 'uci', isUciOk, timeoutMs)
  await ask(port, 'isready', isReadyOk, timeoutMs)
  if (opts.threads && opts.threads > 1) {
    await port.send(`setoption name Threads value ${opts.threads}`)
  }
  if (opts.hashMb && opts.hashMb > 0) {
    await port.send(`setoption name Hash value ${opts.hashMb}`)
  }
  await port.send(`setoption name Multipv value ${Math.max(1, opts.multipv)}`)
}

/** Aguarda uma resposta UCI, falhando por timeout ou término da engine. */
function ask(
  port: EnginePort,
  cmd: string,
  done: (line: string) => boolean,
  timeoutMs = 10_000,
): Promise<void> {
  return new Promise((resolve, reject) => {
    let off: () => void = () => {}
    let offExit: () => void = () => {}
    let timer: ReturnType<typeof setTimeout>
    const cleanup = () => {
      off()
      offExit()
      clearTimeout(timer)
    }
    off = port.onLine((line) => {
      if (done(line)) {
        cleanup()
        resolve()
      }
    })
    offExit =
      port.onExit?.((reason) => {
        cleanup()
        reject(new Error(formatEngineExit(cmd, reason)))
      }) ?? (() => {})
    timer = setTimeout(() => {
      cleanup()
      reject(new Error(`A engine não respondeu a '${cmd}' em ${timeoutMs}ms.`))
    }, timeoutMs)
    void port.send(cmd)
  })
}

/** Formata a mensagem de erro quando a engine encerra durante um comando. */
function formatEngineExit(cmd: string, reason: EngineExitReason): string {
  const detail = reason.error
    ? `: ${reason.error}`
    : reason.signal !== null
      ? ` (sinal ${reason.signal})`
      : reason.code !== null
        ? ` (código ${reason.code})`
        : ''
  return `A engine encerrou durante '${cmd}'${detail}.`
}

function uciToSan(fen: string, uci: string): string | null {
  try {
    const c = new Chess(fen)
    const m = c.move({
      from: uci.slice(0, 2),
      to: uci.slice(2, 4),
      promotion: uci[4],
    })
    return m ? m.san : null
  } catch {
    return null
  }
}

async function evalPosition(
  port: EnginePort,
  fen: string,
  control: AnalyzeControl,
  goTimeoutMs: number,
): Promise<RawPosition> {
  const byPv = new Map<
    number,
    { depth: number; score?: InfoScore; pv: string[] }
  >()
  await port.send(`position fen ${fen}`)
  const goCmd =
    control.mode === 'depth'
      ? `go depth ${control.depth}`
      : `go movetime ${control.movetimeMs}`
  try {
    await ask(
      port,
      goCmd,
      (line) => {
        const info = parseInfo(line)
        if (info?.score) {
          const idx = info.multipv ?? 1
          const prev = byPv.get(idx)
          if (!prev || (info.depth ?? 0) >= prev.depth) {
            byPv.set(idx, {
              depth: info.depth ?? 0,
              score: info.score,
              pv: info.pv ?? [],
            })
          }
        }
        return line.trim().startsWith('bestmove')
      },
      goTimeoutMs,
    )
  } catch (err) {
    // Aborta a busca órfã para que a engine volte a ficar reutilizável.
    await port.send('stop')
    throw err
  }
  const lines: RawLine[] = [...byPv.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([multipv, l]) => ({
      multipv,
      cp: scoreToCp(l.score) ?? 0,
      pv: l.pv,
      depth: l.depth,
    }))
  const principal = lines.find((l) => l.multipv === 1) ?? lines[0]
  return {
    fen,
    cp: principal?.cp ?? 0,
    depth: byPv.get(1)?.depth ?? 0,
    pv: principal?.pv ?? [],
    lines,
  }
}

function addSanToLines(pos: RawPosition): void {
  for (const line of pos.lines ?? []) {
    line.san = line.pv[0] ? uciToSan(pos.fen, line.pv[0]) : null
  }
}

function terminalPosition(fen: string, cp: number): RawPosition {
  return {
    fen,
    cp,
    depth: 0,
    pv: [],
    lines: [{ multipv: 1, cp, pv: [] }],
  }
}

function terminalCp(fen: string): number | null {
  try {
    const c = new Chess(fen)
    if (c.isCheckmate()) return -100000
    if (c.isGameOver()) return 0
    return null
  } catch {
    return null
  }
}

/** Calcula uma vez os terminais usados pelos loops e pelos orçamentos. */
function terminalCps(fens: string[]): (number | null)[] {
  return fens.map(terminalCp)
}

/** Analisa todas as posições com controle fixo e devolve a revisão completa. */
export async function analyzeGame(
  pgn: string,
  control: AnalyzeControl,
  port: EnginePort,
  multipv = 1,
  opts: {
    threads?: number
    hashMb?: number
    cache?: PositionCache
    /** Quando true, não envia `quit` ao final — a engine fica viva para refino ao vivo. */
    keepAlive?: boolean
    /** Override do timeout do `go` por posição (default via `defaultGoTimeout`). */
    goTimeoutMs?: number
    /** Atualização de progresso e, ao avaliar uma posição, seu win% pontual. */
    onDetailedProgress?: (
      progress: AnalysisProgress,
      update?: WinPctUpdate,
    ) => void
  } = {},
): Promise<ReviewResult> {
  const { positionFens, moves } = extractGame(pgn)
  const game = { startFen: positionFens[0], moves }
  const keyValue = controlKeyValue(control)
  const goTimeoutMs = opts.goTimeoutMs ?? defaultGoTimeout(control)

  await configureEngine(port, {
    threads: opts.threads,
    hashMb: opts.hashMb,
    multipv,
  })

  const hits = opts.cache
    ? await opts.cache.getBulk(positionFens, control.mode, keyValue, multipv)
    : positionFens.map(() => null)
  const pendingPuts: RawPosition[] = []
  const raw: RawPosition[] = []
  const terminals = terminalCps(positionFens)
  const phases = computePhases(positionFens.map((fen) => ({ fen })))
  let cachedPositions = 0
  let enginePositions = 0
  let remainingTimedPositions =
    control.mode === 'time'
      ? terminals.filter((term, index) => term === null && !hits[index]).length
      : 0
  try {
    for (let i = 0; i < positionFens.length; i++) {
      const fen = positionFens[i]
      const term = terminals[i]
      if (term === null && !hits[i]) remainingTimedPositions--
      let pos: RawPosition
      if (term !== null) {
        pos = {
          fen,
          cp: term,
          depth: 0,
          pv: [],
          lines: [{ multipv: 1, cp: term, pv: [] }],
        }
      } else {
        const cached = hits[i]
        if (cached) {
          pos = cached
          cachedPositions++
        } else {
          pos = await evalPosition(port, fen, control, goTimeoutMs)
          enginePositions++
          for (const l of pos.lines ?? []) {
            l.san = l.pv[0] ? uciToSan(pos.fen, l.pv[0]) : null
          }
          pendingPuts.push(pos)
          // Flush incremental: limita a perda num crash do processo a ~8
          // posições. Falha propaga (caminho crítico) e cai no catch abaixo,
          // cujo retry best-effort do buffer restante preserva a causa raiz.
          if (opts.cache && pendingPuts.length >= 8) {
            await opts.cache.putMany(
              pendingPuts,
              control.mode,
              keyValue,
              multipv,
            )
            pendingPuts.length = 0
          }
        }
      }
      raw.push(pos)
      const winPct = whiteWinPct(pos.cp, sideToMoveAtPly(game.moves, i))
      opts.onDetailedProgress?.(
        {
          stage: 'analyzing',
          completed: i + 1,
          total: positionFens.length,
          currentPly: i,
          phase: phases[i],
          cachedPositions,
          enginePositions,
          ...(control.mode === 'time'
            ? {
                remainingBudgetMs: remainingTimedPositions * control.movetimeMs,
              }
            : {}),
        },
        { index: i, winPct },
      )
    }
  } catch (err) {
    // Descarrega o buffer mesmo se a análise abortar no meio: posições já
    // avaliadas não se perdem — mas em caráter best-effort, para a causa
    // raiz do aborto vencer (a falha do flush vira warning).
    if (opts.cache && pendingPuts.length) {
      try {
        await opts.cache.putMany(pendingPuts, control.mode, keyValue, multipv)
      } catch (flushErr) {
        console.warn(
          'Falha ao descarregar o cache após aborto da análise:',
          flushErr,
        )
      }
    }
    throw err
  }
  if (opts.cache && pendingPuts.length) {
    // Caminho de sucesso: o cache é caminho crítico, não best-effort.
    await opts.cache.putMany(pendingPuts, control.mode, keyValue, multipv)
  }
  opts.onDetailedProgress?.({
    stage: 'finalizing',
    completed: positionFens.length,
    total: positionFens.length,
    currentPly: positionFens.length - 1,
    phase: phases[phases.length - 1] ?? 'opening',
    cachedPositions,
    enginePositions,
  })
  if (!opts.keepAlive) await port.send('quit')

  const opening = await lookupOpening(moves.map((m) => m.san))
  const book: BookInfo | undefined = opening
    ? { maxPly: opening.moves.length, eco: opening }
    : undefined

  return buildReview(game, raw, book)
}

/**
 * Revisão adaptativa em duas passagens. A triagem cobre todas as posições com
 * MultiPV > 1; apenas pares antes/depois de lances críticos recebem uma busca
 * maior. O resultado final mistura posições de profundidades diferentes.
 */
export async function analyzeGameAdaptive(
  pgn: string,
  profileId: AdaptiveProfileId,
  port: EnginePort,
  opts: {
    threads?: number
    hashMb?: number
    cache?: PositionCache
    keepAlive?: boolean
    goTimeoutMs?: number
    onDetailedProgress?: (
      progress: AnalysisProgress,
      update?: WinPctUpdate,
    ) => void
  } = {},
): Promise<ReviewResult> {
  const profile = ADAPTIVE_PROFILES[profileId]
  const { positionFens, moves } = extractGame(pgn)
  const game = { startFen: positionFens[0], moves }
  const triageControl: AnalyzeControl = {
    mode: 'time',
    movetimeMs: profile.triageMs,
  }

  await configureEngine(port, {
    threads: opts.threads,
    hashMb: opts.hashMb,
    multipv: profile.triageMultipv,
  })

  const triageHits = opts.cache
    ? await opts.cache.getBulk(
        positionFens,
        'time',
        profile.triageMs,
        profile.triageMultipv,
      )
    : positionFens.map(() => null)
  const raw: RawPosition[] = []
  const triagePuts: RawPosition[] = []
  const terminals = terminalCps(positionFens)
  const phases = computePhases(positionFens.map((fen) => ({ fen })))
  let cachedPositions = 0
  let enginePositions = 0
  let remainingTriagePositions = terminals.filter(
    (term, index) => term === null && !triageHits[index],
  ).length

  async function flushTriagePuts(): Promise<void> {
    if (!opts.cache || !triagePuts.length) return
    await opts.cache.putMany(
      triagePuts,
      'time',
      profile.triageMs,
      profile.triageMultipv,
    )
    triagePuts.length = 0
  }

  try {
    for (let index = 0; index < positionFens.length; index++) {
      const fen = positionFens[index]
      const term = terminals[index]
      const cached = triageHits[index]
      if (term === null && !cached) remainingTriagePositions--
      let pos: RawPosition
      if (term !== null) {
        pos = terminalPosition(fen, term)
      } else if (cached) {
        pos = cached
        cachedPositions++
      } else {
        pos = await evalPosition(
          port,
          fen,
          triageControl,
          opts.goTimeoutMs ?? defaultGoTimeout(triageControl),
        )
        addSanToLines(pos)
        enginePositions++
        triagePuts.push(pos)
        if (triagePuts.length >= 8) await flushTriagePuts()
      }
      raw.push(pos)
      const winPct = whiteWinPct(pos.cp, sideToMoveAtPly(moves, index))
      opts.onDetailedProgress?.(
        {
          stage: 'triage',
          completed: index + 1,
          total: positionFens.length,
          currentPly: index,
          phase: phases[index],
          cachedPositions,
          enginePositions,
          remainingBudgetMs: remainingTriagePositions * profile.triageMs,
        },
        { index, winPct },
      )
    }

    await flushTriagePuts()

    const opening = await lookupOpening(moves.map((move) => move.san))
    const book: BookInfo | undefined = opening
      ? { maxPly: opening.moves.length, eco: opening }
      : undefined
    const criticalMoves = rankCriticalMoves(moves, raw, book?.maxPly ?? 0)
    const targets = selectRefinementTargets(
      criticalMoves,
      positionFens.length,
      profile,
    )
    const refinementTargets = targets.filter(
      (target) => terminalCp(positionFens[target.positionIndex]) === null,
    )

    if (refinementTargets.length > 0) {
      if (profile.refinementMultipv !== profile.triageMultipv) {
        await port.send(
          `setoption name Multipv value ${profile.refinementMultipv}`,
        )
        await ask(port, 'isready', isReadyOk)
      }

      let refined = 0
      const refinementBudget = (target: (typeof refinementTargets)[number]) =>
        target.budget === 'high' ? profile.highMs : profile.mediumMs
      let remainingRefinementBudgetMs = refinementTargets.reduce(
        (sum, target) => sum + refinementBudget(target),
        0,
      )
      opts.onDetailedProgress?.({
        stage: 'refinement',
        completed: 0,
        total: refinementTargets.length,
        currentPly: refinementTargets[0].positionIndex,
        phase: phases[refinementTargets[0].positionIndex],
        cachedPositions,
        enginePositions,
        remainingBudgetMs: remainingRefinementBudgetMs,
      })

      for (const budget of ['high', 'medium'] as const) {
        const movetimeMs = budget === 'high' ? profile.highMs : profile.mediumMs
        const control: AnalyzeControl = { mode: 'time', movetimeMs }
        const group = refinementTargets.filter(
          (target) => target.budget === budget,
        )
        const refinementPuts: RawPosition[] = []
        const activeTargets = group.filter(
          (target) => terminals[target.positionIndex] === null,
        )
        const refinementHits =
          opts.cache && activeTargets.length > 0
            ? await opts.cache.getBulk(
                activeTargets.map(
                  (target) => positionFens[target.positionIndex],
                ),
                'time',
                movetimeMs,
                profile.refinementMultipv,
              )
            : activeTargets.map(() => null)
        const hitByPosition = new Map(
          activeTargets.map((target, index) => [
            target.positionIndex,
            refinementHits[index],
          ]),
        )

        for (const target of group) {
          const index = target.positionIndex
          const fen = positionFens[index]
          if (terminals[index] !== null) continue
          let pos = hitByPosition.get(index) ?? null
          if (pos) {
            cachedPositions++
          } else {
            pos = await evalPosition(
              port,
              fen,
              control,
              opts.goTimeoutMs ?? defaultGoTimeout(control),
            )
            addSanToLines(pos)
            enginePositions++
            refinementPuts.push(pos)
          }
          raw[index] = pos
          const winPct = whiteWinPct(pos.cp, sideToMoveAtPly(moves, index))
          refined++
          remainingRefinementBudgetMs -= movetimeMs
          opts.onDetailedProgress?.(
            {
              stage: 'refinement',
              completed: refined,
              total: refinementTargets.length,
              currentPly: index,
              phase: phases[index],
              cachedPositions,
              enginePositions,
              remainingBudgetMs: remainingRefinementBudgetMs,
            },
            { index, winPct },
          )
        }

        if (opts.cache && refinementPuts.length) {
          await opts.cache.putMany(
            refinementPuts,
            'time',
            movetimeMs,
            profile.refinementMultipv,
          )
        }
      }
    }

    opts.onDetailedProgress?.({
      stage: 'finalizing',
      completed: positionFens.length,
      total: positionFens.length,
      currentPly: positionFens.length - 1,
      phase: phases[phases.length - 1] ?? 'opening',
      cachedPositions,
      enginePositions,
    })
    if (!opts.keepAlive) await port.send('quit')
    return buildReview(game, raw, book)
  } catch (err) {
    if (triagePuts.length) {
      try {
        await flushTriagePuts()
      } catch (flushErr) {
        console.warn(
          'Falha ao descarregar a triagem adaptativa após aborto:',
          flushErr,
        )
      }
    }
    throw err
  }
}
