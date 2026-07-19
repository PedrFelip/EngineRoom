import { useCallback, useEffect, useRef, useState } from 'react'
import type { ReviewConfig, ReviewResult } from '../types'
import { analyzeGame, type EnginePort, type RawPosition } from './analyze'
import { createTauriPositionCache } from './cache'
import { engineLitePath } from './engine'
import { createTauriEnginePort, type TauriEnginePort } from './engine-port'
import { saveReview } from './games'
import { createLiveEvalSession, type LiveEvalSession } from './live-eval'
import { useSettings } from './settings-context'
import { isReadyOk, isUciOk } from './uci'

export type ReviewStatus = 'running' | 'done' | 'error'

/** ID de registro das engines no Rust. */
const DEEP_ID = 'primary'
const WIDE_ID = 'live-wide'

/** Sizing fixo da engine leve —控制ado por não expor UI pra ela. */
const WIDE_THREADS = 1
const WIDE_HASH_MB = 64
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
  applyHeavyResources: (threads: number, hashMb: number) => void
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
 */
async function handshakeWide(
  port: EnginePort,
  opts: { threads: number; hashMb: number; multipv: number },
): Promise<void> {
  await ask(port, 'uci', (l) => isUciOk(l))
  await port.send(`setoption name Threads value ${Math.max(1, opts.threads)}`)
  await port.send(`setoption name Hash value ${Math.max(1, opts.hashMb)}`)
  await port.send(`setoption name Multipv value ${Math.max(1, opts.multipv)}`)
  await ask(port, 'isready', (l) => isReadyOk(l))
}

function ask(
  port: EnginePort,
  cmd: string,
  done: (line: string) => boolean,
): Promise<void> {
  return new Promise((resolve) => {
    const off = port.onLine((line) => {
      if (done(line)) {
        off()
        resolve()
      }
    })
    void port.send(cmd)
  })
}

export function useReview(config: ReviewConfig): UseReview {
  const {
    settings,
    setLiveThreads,
    setLiveHashMb,
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

  const sessionRef = useRef<LiveEvalSession | null>(null)
  const deepPortRef = useRef<TauriEnginePort | null>(null)
  const widePortRef = useRef<TauriEnginePort | null>(null)

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
          settings.enginePath || undefined,
          () => cancelled,
        )
        if (cancelled || !deep) return
        deepPortRef.current = deep

        // Sizing da engine pesada é controlado manualmente pelo usuário
        // (painel de configurações do review). Defaults conservadores já
        // estão em `settings.liveThreads/liveHashMb`.
        const sizing: { threads: number; hashMb: number } = {
          threads: settings.liveThreads,
          hashMb: settings.liveHashMb,
        }

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
        // Engine leve (SF17) — opcional. Se disponível, explora mais variações.
        let widePort: TauriEnginePort | null = null
        try {
          const litePath = await engineLitePath()
          if (litePath) {
            const w = await createTauriEnginePort(
              WIDE_ID,
              litePath,
              () => cancelled,
            )
            if (w) {
              await handshakeWide(w, {
                threads: WIDE_THREADS,
                hashMb: WIDE_HASH_MB,
                multipv: WIDE_MULTIPV,
              })
              widePort = w
              widePortRef.current = w
              setLiveWideAvailable(true)
            }
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

  const applyHeavyResources = useCallback(
    (threads: number, hashMb: number) => {
      setLiveThreads(threads)
      setLiveHashMb(hashMb)
      void sessionRef.current
        ?.applyHeavyResources(threads, hashMb)
        .catch(() => {})
    },
    [setLiveThreads, setLiveHashMb],
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
    applyHeavyResources,
    goTo,
    next,
    prev,
    first,
    last,
    flip,
  }
}
