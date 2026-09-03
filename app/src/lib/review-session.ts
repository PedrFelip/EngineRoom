/**
 * Sessão de revisão: orquestração de I/O de uma partida — boot da engine
 * (com sizing best-effort), análise nova ou reabertura do store e persistência.
 * Estado de navegação mora no `ReviewStore` injetado; aqui é só sequenciamento
 * sobre o seam `Backend`, testável com fakes (mesma disciplina do `EnginePort`).
 */

import type {
  Classification,
  Phase,
  PositionAnalysis,
  ReviewConfig,
  ReviewResult,
} from '../types'
import { adaptiveProfileForKind } from './adaptive-analysis'
import type { RawPosition } from './analysis/analysis-types'
import {
  addSanToLines,
  configureEngine,
  evalPosition,
  terminalCp,
} from './analysis/engine-analysis'
import {
  type AnalysisProgress,
  type AnalyzeControl,
  analyzeGame,
  analyzeGameAdaptive,
  type WinPctUpdate,
} from './analyze'
import type { Backend } from './backend'
import { phaseOfPosition } from './phase'
import type { ReviewStore } from './review-store'
import { classifyMove, cpToWinPct, whiteCp, whiteWinPct } from './scoring'
import { recommendedReviewThreads } from './settings'
import { recommendedHashMb } from './system'

export interface ReviewSessionState {
  status: 'running' | 'done' | 'error'
  error: string | null
}

export interface ReviewProgress {
  stage: 'preparing' | AnalysisProgress['stage']
  completed: number
  total: number
  currentPly: number
  phase: Phase | null
  /** Buffer privado da sessão; a view deve criar seu snapshot antes de renderizar. */
  winPcts: readonly number[]
  cachedPositions: number
  enginePositions: number
  remainingBudgetMs?: number
}

export interface ReviewSessionOpts {
  config: ReviewConfig
  backend: Backend
  store: ReviewStore
  onStateChange(state: ReviewSessionState): void
  /** Progresso rico da engine — cru, antes do coalescing por rAF da view. */
  onProgress(progress: ReviewProgress): void
}

export interface ReviewSession {
  start(): Promise<void>
  analyzePosition(
    request: LiveAnalysisRequest,
    settings: LiveAnalysisSettings,
  ): void
  cancelLiveAnalysis(): void
  /** Aborta tudo: engine, listeners. Assíncrono por dentro. */
  dispose(): void
}

export interface LiveAnalysisSettings {
  searchSeconds: number
  lines: number
  threadsAuto: boolean
  threads: number
  memoryMb: number
  moveFeedbackEnabled: boolean
  /** Durante a reprodução da PV, privilegia resposta imediata por lance. */
  fastPass?: boolean
}

interface ResolvedLiveAnalysisSettings
  extends Omit<LiveAnalysisSettings, 'threadsAuto'> {}

interface LiveSearchPlan {
  movetimeMs: number
  multipv: number
}

const PLAYBACK_SEARCH_MS = 500

export interface LiveAnalysisRequest {
  fen: string
  variationNodeId?: string
  sourceFen?: string
  sourceAnalysis?: PositionAnalysis
}

function manualAnalysisControl(config: ReviewConfig): AnalyzeControl {
  if (config.mode === 'time') {
    return { mode: 'time', movetimeMs: config.movetimeMs ?? 5000 }
  }
  return { mode: 'depth', depth: config.engine.depth }
}

export function createReviewSession(opts: ReviewSessionOpts): ReviewSession {
  const { config, backend, store } = opts
  let cancelled = false
  let port: Awaited<ReturnType<Backend['createEnginePort']>> = null
  let livePort: Awaited<ReturnType<Backend['createEnginePort']>> = null
  let appliedLiveSettings: ResolvedLiveAnalysisSettings | null = null
  let detectedLiveResources: {
    threads: number
    memoryMb: number
  } | null = null
  let liveGeneration = 0
  let liveTask: Promise<void> = Promise.resolve()
  const partialWinPcts: number[] = []
  const cache = backend.createPositionCache()

  const notify = (state: ReviewSessionState) => {
    if (!cancelled) opts.onStateChange(state)
  }

  async function start(): Promise<void> {
    try {
      // Reabertura instantânea: instala o resultado antes de qualquer await —
      // a tela não espera a engine subir. Sem variações exploratórias, não há
      // refino ao vivo — engine não precisa subir para initialResult.
      if (config.initialResult) {
        store.setResult(config.initialResult)
        notify({ status: 'done', error: null })
        return
      }

      partialWinPcts.length = 0
      opts.onProgress({
        stage: 'preparing',
        completed: 0,
        total: config.meta.plies + 1,
        currentPly: 0,
        phase: null,
        winPcts: partialWinPcts,
        cachedPositions: 0,
        enginePositions: 0,
      })

      port = await backend.createEnginePort(() => cancelled)
      if (!port) return
      if (cancelled) {
        await port.dispose().catch(() => {})
        port = null
        return
      }

      // Dimensiona Threads/Hash (best-effort: falha → defaults do Stockfish).
      let sizing: { threads?: number; hashMb?: number } = {}
      try {
        const r = await backend.getSystemResources()
        sizing = { threads: r.threads, hashMb: recommendedHashMb(r.memory_mb) }
      } catch {
        /* fallback: defaults */
      }

      const analysisOpts = {
        ...sizing,
        cache,
        keepAlive: false,
        onDetailedProgress: (
          progress: AnalysisProgress,
          update: WinPctUpdate | undefined,
        ) => {
          if (update) partialWinPcts[update.index] = update.winPct
          if (!cancelled) {
            opts.onProgress({ ...progress, winPcts: partialWinPcts })
          }
        },
      }
      const profile = adaptiveProfileForKind(config.analysisKind)
      let review: ReviewResult
      if (profile) {
        review = await analyzeGameAdaptive(
          config.pgn,
          profile.id,
          port,
          analysisOpts,
        )
      } else {
        review = await analyzeGame(
          config.pgn,
          manualAnalysisControl(config),
          port,
          config.lines,
          analysisOpts,
        )
      }
      if (cancelled) return
      store.setResult(review)
      notify({ status: 'done', error: null })
      void backend
        .saveReview(config, review)
        .catch((e) => console.warn('Falha ao salvar a partida no store:', e))
    } catch (e) {
      notify({
        status: 'error',
        error: e instanceof Error ? e.message : String(e),
      })
    } finally {
      // Engine já recebeu `quit` via analyzeGame (keepAlive=false). Dispõe o
      // handle para não vazar processo/file descriptor até o unmount.
      if (port) {
        const p = port
        port = null
        void p.dispose().catch(() => {})
      }
    }
  }

  async function ensureLivePort(
    settings: ResolvedLiveAnalysisSettings,
    plan: LiveSearchPlan,
    generation: number,
  ): Promise<NonNullable<typeof livePort>> {
    if (!livePort) {
      livePort = await backend.createEnginePort(
        () => cancelled || generation !== liveGeneration,
      )
      if (!livePort) throw new Error('A inicialização da engine foi cancelada.')
      await configureEngine(livePort, {
        threads: settings.threads,
        hashMb: settings.memoryMb,
        multipv: plan.multipv,
      })
      appliedLiveSettings = settings
      return livePort
    }
    if (
      appliedLiveSettings?.threads === settings.threads &&
      appliedLiveSettings.memoryMb === settings.memoryMb &&
      appliedLiveSettings.lines === settings.lines &&
      appliedLiveSettings.fastPass === settings.fastPass
    ) {
      return livePort
    }
    await livePort.send(`setoption name Threads value ${settings.threads}`)
    await livePort.send(`setoption name Hash value ${settings.memoryMb}`)
    await livePort.send(`setoption name MultiPV value ${plan.multipv}`)
    appliedLiveSettings = settings
    return livePort
  }

  async function analyzeFen(
    fen: string,
    settings: ResolvedLiveAnalysisSettings,
    generation: number,
  ): Promise<PositionAnalysis> {
    const plan = liveSearchPlan(settings)
    const cached = await cache.get(fen, 'time', plan.movetimeMs, plan.multipv)
    if (cancelled || generation !== liveGeneration) {
      throw new Error('A análise da posição foi cancelada.')
    }
    let raw = cached
    if (!raw) {
      const terminal = terminalCp(fen)
      if (terminal !== null) {
        raw = {
          fen,
          cp: terminal,
          depth: 0,
          pv: [],
          lines: [{ multipv: 1, cp: terminal, pv: [] }],
        }
      } else {
        const engine = await ensureLivePort(settings, plan, generation)
        if (cancelled || generation !== liveGeneration) {
          throw new Error('A análise da posição foi cancelada.')
        }
        raw = await evalPosition(
          engine,
          fen,
          { mode: 'time', movetimeMs: plan.movetimeMs },
          plan.movetimeMs + 10_000,
          plan.movetimeMs,
        )
        addSanToLines(raw)
        await cache.put(raw, 'time', plan.movetimeMs, plan.multipv)
      }
    }
    return positionAnalysis(raw)
  }

  function runLiveAnalysis(
    generation: number,
    request: LiveAnalysisRequest,
    settings: LiveAnalysisSettings,
  ): Promise<void> {
    return (async () => {
      if (cancelled || generation !== liveGeneration) return
      store.startLiveAnalysis(request.fen)
      try {
        const resolvedSettings = await resolveLiveSettings(settings)
        if (cancelled || generation !== liveGeneration) return
        const analysis = await analyzeFen(
          request.fen,
          resolvedSettings,
          generation,
        )
        if (cancelled || generation !== liveGeneration) return
        store.setLiveAnalysis(request.fen, analysis)
        if (
          !settings.moveFeedbackEnabled ||
          !request.variationNodeId ||
          !request.sourceFen
        ) {
          return
        }
        let source = request.sourceAnalysis
        if (!source && request.sourceFen === request.fen) {
          source = analysis
        }
        if (!source && !settings.fastPass) {
          source = await analyzeFen(
            request.sourceFen,
            resolvedSettings,
            generation,
          )
        }
        if (!source || cancelled || generation !== liveGeneration) return
        store.setVariationClassification(
          request.variationNodeId,
          classifyLiveMove(source, analysis),
        )
      } catch (error) {
        if (cancelled || generation !== liveGeneration) return
        store.failLiveAnalysis(
          request.fen,
          error instanceof Error ? error.message : String(error),
        )
      }
    })()
  }

  async function resolveLiveSettings(
    settings: LiveAnalysisSettings,
  ): Promise<ResolvedLiveAnalysisSettings> {
    if (!settings.threadsAuto) return manualLiveSettings(settings)
    if (!detectedLiveResources) {
      try {
        const resources = await backend.getSystemResources()
        detectedLiveResources = {
          threads: recommendedReviewThreads(resources.threads),
          memoryMb: recommendedHashMb(resources.memory_mb),
        }
      } catch {
        return manualLiveSettings(settings)
      }
    }
    return {
      ...manualLiveSettings(settings),
      threads: detectedLiveResources.threads,
      memoryMb: detectedLiveResources.memoryMb,
    }
  }

  return {
    start,
    analyzePosition(request, settings) {
      const generation = ++liveGeneration
      void livePort?.send('stop')
      liveTask = liveTask
        .catch(() => {})
        .then(() => runLiveAnalysis(generation, request, settings))
    },
    cancelLiveAnalysis() {
      liveGeneration++
      void livePort?.send('stop')
    },
    dispose() {
      cancelled = true
      liveGeneration++
      const p = port
      port = null
      void p?.dispose().catch(() => {})
      const live = livePort
      livePort = null
      appliedLiveSettings = null
      void live?.dispose().catch(() => {})
    },
  }
}

function manualLiveSettings(
  settings: LiveAnalysisSettings,
): ResolvedLiveAnalysisSettings {
  return {
    searchSeconds: settings.searchSeconds,
    lines: settings.lines,
    threads: settings.threads,
    memoryMb: settings.memoryMb,
    moveFeedbackEnabled: settings.moveFeedbackEnabled,
    fastPass: settings.fastPass,
  }
}

function liveSearchPlan(
  settings: ResolvedLiveAnalysisSettings,
): LiveSearchPlan {
  if (settings.fastPass) {
    return { movetimeMs: PLAYBACK_SEARCH_MS, multipv: 1 }
  }
  return {
    movetimeMs: settings.searchSeconds * 1000,
    multipv: settings.lines,
  }
}

function positionAnalysis(raw: RawPosition): PositionAnalysis {
  const stm = raw.fen.split(' ')[1] === 'b' ? 'b' : 'w'
  const lines = (raw.lines ?? []).map((line) => ({
    multipv: line.multipv,
    san: line.san ?? null,
    cp: whiteCp(line.cp, stm),
    winPct: whiteWinPct(line.cp, stm),
    pv: line.pv,
  }))
  return {
    ply: 0,
    fen: raw.fen,
    phase: phaseOfPosition(raw.fen),
    depth: raw.depth,
    cp: raw.cp,
    winPct: whiteWinPct(raw.cp, stm),
    pv: raw.pv,
    lines,
  }
}

function classifyLiveMove(
  before: PositionAnalysis,
  after: PositionAnalysis,
): Classification {
  const winPctBefore = cpToWinPct(before.cp)
  const winPctAfter = 100 - cpToWinPct(after.cp)
  const loss = Math.max(0, winPctBefore - winPctAfter)
  return classifyMove(loss)
}
