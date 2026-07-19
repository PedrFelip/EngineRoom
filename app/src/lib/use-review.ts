import { useCallback, useEffect, useRef, useState } from 'react'
import type { ReviewConfig, ReviewResult } from '../types'
import { analyzeGame, type EnginePort, type RawPosition } from './analyze'
import { createTauriPositionCache } from './cache'
import { createTauriEnginePort, type TauriEnginePort } from './engine-port'
import { saveReview } from './games'
import { createLiveEvalSession, type LiveEvalSession } from './live-eval'
import { useSettings } from './settings-context'
import {
  computePresets,
  getSystemResources,
  type LivePreset,
} from './system'
import { isReadyOk, isUciOk } from './uci'

export type ReviewStatus = 'running' | 'done' | 'error'

/** ID de registro das engines no Rust. */
const DEEP_ID = 'primary'
const WIDE_ID = 'live-wide'

/** Quantas variações a wide explora por posição. */
const WIDE_MULTIPV = 6

export interface UseReview {
  result: ReviewResult | null
  status: ReviewStatus
  error: string | null
  currentPly: number
  orientation: 'white' | 'black'
  /** Posição ao vivo refinada (null enquanto não há dados do refino). */
  livePosition: RawPosition | null
  /** True quando a engine leve está disponível nesta plataforma. */
  liveWideAvailable: boolean
  /** Estado corrente do toggle da engine leve. */
  liveWideOn: boolean
  setLiveWideOn: (on: boolean) => void
  /** Presets computados a partir de getSystemResources (null enquanto carrega). */
  presets: LivePreset[] | null
  applyPreset: (preset: LivePreset) => void
  goTo: (ply: number) => void
  next: () => void
  prev: () => void
  first: () => void
  last: () => void
  flip: () => void
}

/**
 * Faz o handshake UCI mínimo necessário antes de uma enginewide começar a
 * receber `position`/`go`: aguarda `uciok` e `readyok`, e seta Threads, Hash e
 * Multipv. Falhas propagam (o caller decide se ignora).
 *
 * Cada `ask` tem timeout — sem isso, uma engine que não responde deixaria o
 * hook pendurado para sempre (caso real: sidecar não encontrado em dev).
 */
async function handshakeWide(
  port: EnginePort,
  opts: { threads: number; hashMb: number; multipv: number },
): Promise<void> {
  await askWithTimeout(port, 'uci', (l) => isUciOk(l), HANDSHAKE_TIMEOUT_MS)
  await port.send(`setoption name Threads value ${Math.max(1, opts.threads)}`)
  await port.send(`setoption name Hash value ${Math.max(1, opts.hashMb)}`)
  await port.send(`setoption name Multipv value ${Math.max(1, opts.multipv)}`)
  await askWithTimeout(
    port,
    'isready',
    (l) => isReadyOk(l),
    HANDSHAKE_TIMEOUT_MS,
  )
}

const HANDSHAKE_TIMEOUT_MS = 8000

function askWithTimeout(
  port: EnginePort,
  cmd: string,
  done: (line: string) => boolean,
  ms: number,
): Promise<void> {
  return new Promise((resolve, reject) => {
    let off = () => {}
    const timer = setTimeout(() => {
      off()
      reject(new Error(`timeout aguardando resposta de '${cmd}' (${ms}ms)`))
    }, ms)
    off = port.onLine((line) => {
      if (done(line)) {
        off()
        clearTimeout(timer)
        resolve()
      }
    })
    void port.send(cmd)
  })
}

/** Log de diagnóstico, ativo só em dev. */
function debug(...args: unknown[]) {
  if (import.meta.env.DEV) console.info('[live]', ...args)
}

export function useReview(config: ReviewConfig): UseReview {
  const {
    settings,
    setLivePreset,
    setLiveWideOn: persistLiveWideOn,
  } = useSettings()
  const [result, setResult] = useState<ReviewResult | null>(null)
  const [status, setStatus] = useState<ReviewStatus>('running')
  const [error, setError] = useState<string | null>(null)
  const [currentPly, setCurrentPly] = useState(0)
  const [orientation, setOrientation] = useState<'white' | 'black'>('white')
  const [livePosition, setLivePosition] = useState<RawPosition | null>(null)
  const [liveWideAvailable, setLiveWideAvailable] = useState(false)
  const [liveWideOn, setLiveWideOnState] = useState(settings.liveWideOn)
  const [presets, setPresets] = useState<LivePreset[] | null>(null)

  const sessionRef = useRef<LiveEvalSession | null>(null)
  const deepPortRef = useRef<TauriEnginePort | null>(null)
  const widePortRef = useRef<TauriEnginePort | null>(null)
  const presetsRef = useRef<LivePreset[] | null>(null)

  // Carrega os recursos do sistema uma vez por sessão do hook — presets
  // dependem de cores/RAM. Best-effort: se falhar, presets fica null e a UI
  // pode continuar com defaults conservadores hardcoded.
  useEffect(() => {
    let cancelled = false
    getSystemResources()
      .then((sys) => {
        if (cancelled) return
        const list = computePresets(sys)
        presetsRef.current = list
        setPresets(list)
      })
      .catch(() => {
        /* presets fica null — UI segue com fallback */
      })
    return () => {
      cancelled = true
    }
  }, [])

  // biome-ignore lint/correctness/useExhaustiveDependencies: dependências intencionais — settings.liveWideOn só é lido no setup inicial; mudanças no toggle em runtime vão por setLiveWideOn→session.setWideEnabled
  useEffect(() => {
    // Partida vinda do store: reabertura instantânea, sem engine e sem regravar.
    if (config.initialResult) {
      setResult(config.initialResult)
      setCurrentPly(config.initialResult.moves.length)
      setStatus('done')
      return
    }

    let cancelled = false

    ;(async () => {
      try {
        const deep = await createTauriEnginePort(
          DEEP_ID,
          undefined, // sidecar default ("stockfish")
          settings.enginePath || undefined,
          () => cancelled,
        )
        if (cancelled || !deep) return
        deepPortRef.current = deep

        // Sizing do preset selecionado (default Equilibrado). Se os presets
        // ainda não carregaram, usa fallback conservador.
        const preset =
          presetsRef.current?.find((p) => p.id === settings.livePreset) ?? {
            deep: { threads: 2, hashMb: 256 },
            wide: { threads: 1, hashMb: 64 },
          }
        const sizing = preset.deep

        const control =
          config.mode === 'time'
            ? {
                mode: 'time' as const,
                movetimeMs: config.movetimeMs ?? 5000,
              }
            : { mode: 'depth' as const, depth: config.engine.depth }

        // keepAlive: true → a engine pesada permanece viva para o refino ao vivo.
        const review = await analyzeGame(
          config.pgn,
          control,
          deep,
          config.lines,
          {
            ...sizing,
            cache: createTauriPositionCache(),
            keepAlive: true,
          },
        )
        if (cancelled) return
        setResult(review)
        setCurrentPly(review.moves.length)
        setStatus('done')
        void saveReview(config, review).catch((e) =>
          console.warn('Falha ao salvar a partida no store:', e),
        )

        // === Refino ao vivo ===
        // Engine leve (SF17) — opcional. Sidecar "stockfish-lite" (definido em
        // `bundle.externalBin`). Se indisponível, segue só com a pesada.
        let widePort: TauriEnginePort | null = null
        try {
          debug('spawnando engine leve (sidecar "stockfish-lite")…')
          const w = await createTauriEnginePort(
            WIDE_ID,
            'stockfish-lite',
            undefined,
            () => cancelled,
          )
          if (w) {
            debug('wide spawnada, iniciando handshake')
            await handshakeWide(w, {
              threads: preset.wide.threads,
              hashMb: preset.wide.hashMb,
              multipv: WIDE_MULTIPV,
            })
            debug('handshake wide ok')
            widePort = w
            widePortRef.current = w
            setLiveWideAvailable(true)
          } else {
            debug('createTauriEnginePort devolveu null (cancelado?)')
          }
        } catch (e) {
          console.warn('Engine leve indisponível, seguindo só com a pesada:', e)
        }

        if (cancelled) return

        const initialFen =
          review.positions[review.moves.length]?.fen ?? review.positions[0].fen

        const session = createLiveEvalSession(
          { deep, wide: widePort },
          { fen: initialFen },
          { onMerge: (pos) => setLivePosition(pos) },
          { cache: createTauriPositionCache() },
        )
        sessionRef.current = session

        if (widePort && !settings.liveWideOn) {
          await session.setWideEnabled(false)
        }
        await session.start()
        debug('session iniciada (deep%s)', widePort ? ' + wide' : ' apenas')
      } catch (e) {
        if (cancelled) return
        setError(e instanceof Error ? e.message : String(e))
        setStatus('error')
      }
    })()

    return () => {
      cancelled = true
      const session = sessionRef.current
      const deep = deepPortRef.current
      const wide = widePortRef.current
      sessionRef.current = null
      deepPortRef.current = null
      widePortRef.current = null
      setLivePosition(null)
      setLiveWideAvailable(false)
      void (async () => {
        try {
          if (session) await session.stop()
        } catch {
          /* ignore */
        }
        await deep?.dispose().catch(() => {})
        await wide?.dispose().catch(() => {})
      })()
    }
  }, [config, settings.enginePath])

  // Quando o usuário troca de ply, a session refina a nova posição.
  useEffect(() => {
    if (!result || !sessionRef.current) return
    const fen = result.positions[currentPly]?.fen
    if (!fen) return
    setLivePosition(null)
    void sessionRef.current.setFen(fen).catch(() => {})
  }, [currentPly, result])

  const total = result?.moves.length ?? 0

  const goTo = useCallback(
    (ply: number) => setCurrentPly(Math.max(0, Math.min(total, ply))),
    [total],
  )
  const next = useCallback(
    () => setCurrentPly((p) => Math.min(total, p + 1)),
    [total],
  )
  const prev = useCallback(() => setCurrentPly((p) => Math.max(0, p - 1)), [])
  const first = useCallback(() => setCurrentPly(0), [])
  const last = useCallback(() => setCurrentPly(total), [total])
  const flip = useCallback(
    () => setOrientation((o) => (o === 'white' ? 'black' : 'white')),
    [],
  )

  const setLiveWideOn = useCallback(
    (on: boolean) => {
      setLiveWideOnState(on)
      persistLiveWideOn(on)
      void sessionRef.current?.setWideEnabled(on).catch(() => {})
    },
    [persistLiveWideOn],
  )

  const applyPreset = useCallback(
    (preset: LivePreset) => {
      setLivePreset(preset.id)
      const session = sessionRef.current
      if (!session) return
      void session
        .applyHeavyResources(preset.deep.threads, preset.deep.hashMb)
        .catch(() => {})
      void session
        .applyWideResources(preset.wide.threads, preset.wide.hashMb)
        .catch(() => {})
    },
    [setLivePreset],
  )

  return {
    result,
    status,
    error,
    currentPly,
    orientation,
    livePosition,
    liveWideAvailable,
    liveWideOn,
    setLiveWideOn,
    presets,
    applyPreset,
    goTo,
    next,
    prev,
    first,
    last,
    flip,
  }
}
