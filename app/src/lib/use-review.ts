import { Chess } from 'chess.js'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type {
  ReviewConfig,
  ReviewResult,
  Variation,
  VariationMap,
  VariationMove,
} from '../types'
import { analyzeGame, configureEngine } from './analyze'
import { createTauriPositionCache } from './cache'
import { createTauriEnginePort, type TauriEnginePort } from './engine-port'
import { saveReview } from './games'
import { useSettings } from './settings-context'
import { getSystemResources, recommendedHashMb } from './system'
import {
  createVariationEvalSession,
  type VariationEvalSession,
} from './variation-eval'
import {
  applyLiveToVariation,
  decideUserMove,
  defaultBeforeCpResolver,
} from './variations'

export type ReviewStatus = 'running' | 'done' | 'error'

export interface UseReview {
  result: ReviewResult | null
  status: ReviewStatus
  error: string | null
  currentPly: number
  orientation: 'white' | 'black'
  goTo: (ply: number) => void
  next: () => void
  prev: () => void
  first: () => void
  last: () => void
  flip: () => void
  /** Linhas alternativas jogadas pelo usuário, indexadas pelo ply-pai. */
  variations: VariationMap
  /** Variação/ply atualmente em foco (null = linha principal). */
  currentVariation: {
    variationId: string
    parentPly: number
    ply: number
  } | null
  /** Destinos lícitos do lado a jogar na posição exibida (para o chessground). */
  dests: Map<string, string[]> | null
  /** Cor do lado a jogar na posição exibida. */
  turnColor: 'white' | 'black' | null
  /** FEN da posição exibida no tabuleiro (linha principal ou variação). */
  displayedFen: string | null
  /** Lance (de variação ou linha principal) que levou à posição exibida. */
  currentVariationMove: VariationMove | null
  /** Aplica um lance arrastado pelo usuário: avança a linha ou abre variação. */
  makeMove: (uci: string) => void
  /** Navega para um lance de uma variação. */
  goToVariation: (variationId: string, parentPly: number, ply: number) => void
  /** Abandona a variação e volta para a linha principal no ply-pai. */
  exitVariation: () => void
}

/** Decompõe um lance UCI ("e2e4" / "e7e8q") em from/to/promotion p/ chess.js. */
function parseUci(uci: string): {
  from: string
  to: string
  promotion: string | undefined
} {
  return {
    from: uci.slice(0, 2),
    to: uci.slice(2, 4),
    promotion: uci.length > 4 ? uci[4] : undefined,
  }
}

export function useReview(config: ReviewConfig): UseReview {
  const { settings } = useSettings()
  const [result, setResult] = useState<ReviewResult | null>(null)
  const [status, setStatus] = useState<ReviewStatus>('running')
  const [error, setError] = useState<string | null>(null)
  const [currentPly, setCurrentPly] = useState(0)
  const [orientation, setOrientation] = useState<'white' | 'black'>('white')
  const [variations, setVariations] = useState<VariationMap>({})
  const [currentVariation, setCurrentVariation] = useState<{
    variationId: string
    parentPly: number
    ply: number
  } | null>(null)
  const [dests, setDests] = useState<Map<string, string[]> | null>(null)
  const [turnColor, setTurnColor] = useState<'white' | 'black' | null>(null)

  const sessionRef = useRef<VariationEvalSession | null>(null)
  const portRef = useRef<TauriEnginePort | null>(null)
  // Instância persistente de chess.js para validar lances do usuário e computar
  // dests/turnColor na posição exibida. Mantida em sync pelo effect de displayedFen.
  const chessRef = useRef<Chess | null>(null)
  if (chessRef.current === null) chessRef.current = new Chess()
  // Alvo do refino da engine: posição da linha principal ou de um lance de
  // variação. Apenas posições de variação são refinadas — a linha principal
  // mostra só o resultado do analyzeGame.
  const analysisTargetRef = useRef<
    | { kind: 'mainline' }
    | { kind: 'variation'; variationId: string; moveId: string }
  >({ kind: 'mainline' })
  // Refs que espelham estado para closures estáveis (onMerge é criado uma vez).
  const resultRef = useRef<ReviewResult | null>(null)
  resultRef.current = result
  const variationsRef = useRef<VariationMap>(variations)
  variationsRef.current = variations
  const resolveBeforeCpRef = useRef<
    (v: Variation, m: VariationMove) => number | undefined
  >(() => undefined)
  resolveBeforeCpRef.current = (v, m) =>
    defaultBeforeCpResolver(
      v,
      m,
      (ply) => resultRef.current?.positions[ply]?.cp,
    )
  const idCounterRef = useRef(0)

  useEffect(() => {
    let cancelled = false

    // Reabertura instantânea: mostra o resultado imediatamente, antes mesmo
    // de spawnar a engine. Preserva a UX "sem loading" do store.
    if (config.initialResult) {
      setResult(config.initialResult)
      setCurrentPly(config.initialResult.moves.length)
      setStatus('done')
    }

    ;(async () => {
      try {
        const port = await createTauriEnginePort(
          settings.enginePath || undefined,
          () => cancelled,
        )
        if (cancelled || !port) return
        portRef.current = port

        // Dimensiona a engine pra usar o máximo de CPU/RAM (Threads + Hash).
        // Best-effort: se a detecção falhar, segue com os defaults do Stockfish.
        let sizing: { threads?: number; hashMb?: number } = {}
        try {
          const r = await getSystemResources()
          sizing = {
            threads: r.threads,
            hashMb: recommendedHashMb(r.memory_mb),
          }
        } catch {
          /* fallback: defaults */
        }

        // Ramo: análise nova (analyzeGame faz handshake E analisa) vs
        // reabertura do store (só handshake manual — o review já temos).
        let current: ReviewResult
        if (config.initialResult) {
          await configureEngine(port, { ...sizing, multipv: config.lines })
          if (cancelled) return
          current = config.initialResult
        } else {
          const control =
            config.mode === 'time'
              ? {
                  mode: 'time' as const,
                  movetimeMs: config.movetimeMs ?? 5000,
                }
              : { mode: 'depth' as const, depth: config.engine.depth }
          // keepAlive: true → a engine permanece viva para avaliar variações.
          const review = await analyzeGame(
            config.pgn,
            control,
            port,
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
          current = review
        }

        // === Refino de variações ===
        // Loop minimal sobre o SF18 já vivo. Só alimenta `applyLiveToVariation`
        // quando o alvo é uma variação; a linha principal mostra só o
        // `result.positions[i]` do analyzeGame.
        // Se o usuário já navegou pra um ply diferente do final antes da engine
        // subir, usamos o FEN exibido no lugar do "final" — evita a engine
        // avaliar posição que ninguém está vendo.
        const initialFen =
          displayedFenRef.current ??
          current.positions[current.moves.length]?.fen ??
          current.positions[0].fen
        const session = createVariationEvalSession(
          port,
          { fen: initialFen, multipv: config.lines },
          {
            onMerge: (pos) => {
              const target = analysisTargetRef.current
              if (target.kind !== 'variation') return
              setVariations((prev) =>
                applyLiveToVariation(
                  prev,
                  target,
                  pos,
                  resolveBeforeCpRef.current,
                ),
              )
            },
          },
        )
        sessionRef.current = session
        if (cancelled) return
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
      const port = portRef.current
      sessionRef.current = null
      portRef.current = null
      void (async () => {
        try {
          if (session) await session.stop()
        } catch {
          /* ignore */
        }
        await port?.dispose().catch(() => {})
      })()
    }
  }, [config, settings.enginePath])

  // Variação em foco e lance atual dentro dela (null = linha principal).
  const currentVariationData = useMemo(() => {
    if (!currentVariation) return null
    const list = variations[currentVariation.parentPly] ?? []
    return list.find((v) => v.id === currentVariation.variationId) ?? null
  }, [currentVariation, variations])

  const currentVariationMove = useMemo(() => {
    if (!currentVariation || !currentVariationData) return null
    return currentVariationData.moves[currentVariation.ply - 1] ?? null
  }, [currentVariation, currentVariationData])

  const displayedFen =
    currentVariationMove?.fenAfter ?? result?.positions[currentPly]?.fen ?? null

  // Espelho de `displayedFen` para o effect principal ler o FEN atual mesmo
  // depois dos awaits (quando ele criar a variation-eval session). Sem isso,
  // a engine poderia nascer apontando pra posição final enquanto o usuário
  // já navegou pra outra.
  const displayedFenRef = useRef<string | null>(displayedFen)
  displayedFenRef.current = displayedFen

  // Espelha o alvo do refino para o ref lido pelo onMerge (criado uma só vez).
  if (currentVariation && currentVariationMove) {
    analysisTargetRef.current = {
      kind: 'variation',
      variationId: currentVariation.variationId,
      moveId: currentVariationMove.id,
    }
  } else {
    analysisTargetRef.current = { kind: 'mainline' }
  }

  // Sincroniza o chess.js com a posição exibida (compute dests/turnColor) e
  // reponta a session de refino. Depende só do FEN exibido — assim o refino
  // progressivo de uma mesma posição não é reiniciado quando o lance recebe nota.
  useEffect(() => {
    if (!displayedFen) return
    const chess = chessRef.current
    if (chess) {
      try {
        chess.load(displayedFen)
        const map = new Map<string, string[]>()
        for (const m of chess.moves({ verbose: true })) {
          const arr = map.get(m.from)
          if (arr) arr.push(m.to)
          else map.set(m.from, [m.to])
        }
        setDests(map.size > 0 ? map : null)
        setTurnColor(chess.turn() === 'w' ? 'white' : 'black')
      } catch {
        setDests(null)
        setTurnColor(null)
      }
    }
    if (!sessionRef.current) return
    void sessionRef.current.setFen(displayedFen).catch(() => {})
  }, [displayedFen])

  const total = result?.moves.length ?? 0

  const goTo = useCallback(
    (ply: number) => {
      setCurrentVariation(null)
      setCurrentPly(Math.max(0, Math.min(total, ply)))
    },
    [total],
  )

  const next = useCallback(() => {
    if (currentVariation) {
      const v = variationsRef.current[currentVariation.parentPly]?.find(
        (x) => x.id === currentVariation.variationId,
      )
      const len = v?.moves.length ?? 0
      if (currentVariation.ply < len) {
        setCurrentVariation({
          ...currentVariation,
          ply: currentVariation.ply + 1,
        })
      }
      return
    }
    setCurrentPly((p) => Math.min(total, p + 1))
  }, [currentVariation, total])

  const prev = useCallback(() => {
    if (currentVariation) {
      if (currentVariation.ply > 1) {
        setCurrentVariation({
          ...currentVariation,
          ply: currentVariation.ply - 1,
        })
      } else {
        setCurrentVariation(null)
      }
      return
    }
    setCurrentPly((p) => Math.max(0, p - 1))
  }, [currentVariation])

  const first = useCallback(() => {
    setCurrentVariation(null)
    setCurrentPly(0)
  }, [])
  const last = useCallback(() => {
    setCurrentVariation(null)
    setCurrentPly(total)
  }, [total])
  const flip = useCallback(
    () => setOrientation((o) => (o === 'white' ? 'black' : 'white')),
    [],
  )

  const goToVariation = useCallback(
    (variationId: string, parentPly: number, ply: number) => {
      // Mantém currentPly sincronizado com o ply-pai (invariante: dentro de
      // variação, currentPly === parentPly → sair volta ao ponto de ramificação).
      setCurrentPly(parentPly)
      setCurrentVariation({ variationId, parentPly, ply })
    },
    [],
  )

  const exitVariation = useCallback(() => {
    setCurrentVariation(null)
  }, [])

  const nextId = useCallback((prefix: 'm' | 'v') => {
    idCounterRef.current += 1
    return `${prefix}${idCounterRef.current}`
  }, [])

  const makeMove = useCallback(
    (uci: string) => {
      const chess = chessRef.current
      if (!chess || !result) return
      const parsed = parseUci(uci)
      const fenBefore = chess.fen()
      let moveObj: ReturnType<Chess['move']> | null = null
      try {
        moveObj = chess.move({
          from: parsed.from,
          to: parsed.to,
          promotion: parsed.promotion,
        })
      } catch {
        return
      }
      if (!moveObj) return
      const fenAfter = chess.fen()
      const color = moveObj.color // 'w' | 'b'
      const san = moveObj.san

      // Dentro de variação: acrescenta à variação corrente.
      if (currentVariation) {
        const list = variationsRef.current[currentVariation.parentPly] ?? []
        const vIdx = list.findIndex(
          (v) => v.id === currentVariation.variationId,
        )
        if (vIdx === -1) return
        const v = list[vIdx]
        const newMove: VariationMove = {
          id: nextId('m'),
          ply: v.moves.length + 1,
          color,
          san,
          uci,
          fenBefore,
          fenAfter,
        }
        const newV: Variation = { ...v, moves: [...v.moves, newMove] }
        setVariations({
          ...variationsRef.current,
          [v.parentPly]: [
            ...list.slice(0, vIdx),
            newV,
            ...list.slice(vIdx + 1),
          ],
        })
        setCurrentVariation({
          variationId: v.id,
          parentPly: v.parentPly,
          ply: newMove.ply,
        })
        return
      }

      // Na linha principal: avança se o lance coincide com o próximo, senão abre variação.
      const nextUci = result.moves[currentPly]?.uci ?? null
      const decision = decideUserMove(uci, currentPly, nextUci)
      if (decision.kind === 'advance') {
        setCurrentVariation(null)
        setCurrentPly(currentPly + 1)
        return
      }
      const parentPly = decision.parentPly
      const firstMove: VariationMove = {
        id: nextId('m'),
        ply: 1,
        color,
        san,
        uci,
        fenBefore,
        fenAfter,
      }
      const newV: Variation = {
        id: nextId('v'),
        parentPly,
        moves: [firstMove],
      }
      const list = variationsRef.current[parentPly] ?? []
      setVariations({
        ...variationsRef.current,
        [parentPly]: [...list, newV],
      })
      setCurrentVariation({
        variationId: newV.id,
        parentPly,
        ply: 1,
      })
    },
    [currentVariation, currentPly, result, nextId],
  )

  return {
    result,
    status,
    error,
    currentPly,
    orientation,
    goTo,
    next,
    prev,
    first,
    last,
    flip,
    variations,
    currentVariation,
    dests,
    turnColor,
    displayedFen,
    currentVariationMove,
    makeMove,
    goToVariation,
    exitVariation,
  }
}
