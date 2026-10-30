import { useCallback, useEffect, useRef, useState } from 'react'
import { runUi } from '../../lib/effect/ui-runtime'
import {
  deleteGame,
  getGame,
  getReviewConfig,
  listGames,
} from '../../lib/games'
import type { GameCursor, GameSummary, ReviewConfig } from '../../types'

const defaultHistoryIO = {
  list: (cursor: GameCursor | null, signal: AbortSignal) =>
    runUi(listGames(50, cursor), signal),
  open: (id: number, signal: AbortSignal) => runUi(getReviewConfig(id), signal),
  get: (id: number, signal: AbortSignal) => runUi(getGame(id), signal),
  delete: (id: number) => runUi(deleteGame(id)),
}
export type HistoryIO = typeof defaultHistoryIO
interface Options {
  io?: HistoryIO
  active: boolean
  onStart: (config: ReviewConfig) => void
  importPgn: (pgn: string) => void
  setView: (view: 'new' | 'history') => void
  setStep: (step: 'import' | 'configure') => void
}
export function useHistoryController({
  active,
  io = defaultHistoryIO,
  onStart,
  importPgn,
  setView,
  setStep,
}: Options) {
  const activeRef = useRef(active)
  activeRef.current = active
  const historyRequest = useRef<AbortController | null>(null)
  const historyAction = useRef(false)
  const deleting = useRef(false)
  const actionRequest = useRef<AbortController | null>(null)
  const [loadingHistory, setLoadingHistory] = useState(true)
  const [pendingGame, setPendingGame] = useState<number | null>(null)
  const [games, setGames] = useState<GameSummary[]>([])
  const [gamesTotal, setGamesTotal] = useState(0)
  const [nextGamesCursor, setNextGamesCursor] = useState<GameCursor | null>(
    null,
  )
  const [loadingMoreGames, setLoadingMoreGames] = useState(false)
  const [historyError, setHistoryError] = useState<string | null>(null)
  const invalidateHistory = useCallback(() => {
    historyRequest.current?.abort()
    historyRequest.current = null
    actionRequest.current?.abort()
    actionRequest.current = null
    historyAction.current = false
    setPendingGame(null)
    setLoadingMoreGames(false)
    setLoadingHistory(false)
  }, [])

  const loadHistory = useCallback(() => {
    invalidateHistory()
    const controller = new AbortController()
    historyRequest.current = controller
    setLoadingHistory(true)
    setHistoryError(null)
    void io
      .list(null, controller.signal)
      .then((page) => {
        if (controller.signal.aborted) return
        setGames(page.games)
        setGamesTotal(page.total)
        setNextGamesCursor(page.nextCursor)
      })
      .catch((error) => {
        if (controller.signal.aborted) return
        console.warn('Falha ao listar partidas analisadas:', error)
        setHistoryError('Não foi possível carregar o histórico.')
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoadingHistory(false)
      })
  }, [invalidateHistory, io])

  useEffect(() => {
    loadHistory()
    return () => {
      activeRef.current = false
      historyRequest.current?.abort()
      actionRequest.current?.abort()
    }
  }, [loadHistory])

  const clearGames = useCallback(() => {
    invalidateHistory()
    setGames([])
    setGamesTotal(0)
    setNextGamesCursor(null)
    setHistoryError(null)
  }, [invalidateHistory])

  const wasActive = useRef(active)
  useEffect(() => {
    if (!active) {
      invalidateHistory()
    } else if (!wasActive.current) {
      loadHistory()
    }
    wasActive.current = active
  }, [active, invalidateHistory, loadHistory])

  const openStored = useCallback(
    function openStored(id: number): void {
      if (historyAction.current || deleting.current) return
      historyAction.current = true
      const action = new AbortController()
      actionRequest.current = action
      setPendingGame(id)
      setHistoryError(null)
      void io
        .open(id, action.signal)
        .then((config) => {
          if (action.signal.aborted) return
          if (config) onStart(config)
          else setHistoryError('Esta partida não está mais disponível.')
        })
        .catch((error) => {
          if (action.signal.aborted) return
          console.warn('Falha ao abrir partida:', error)
          setHistoryError(
            'Não foi possível abrir a revisão salva. Você pode reanalisar o PGN.',
          )
        })
        .finally(() => {
          if (actionRequest.current !== action) return
          actionRequest.current = null
          historyAction.current = false
          setPendingGame(null)
        })
    },
    [onStart, io],
  )

  const removeStored = useCallback(
    async (id: number): Promise<boolean> => {
      if (historyAction.current || deleting.current) return false
      invalidateHistory()
      historyAction.current = true
      deleting.current = true
      const action = new AbortController()
      actionRequest.current = action
      setPendingGame(id)
      setHistoryError(null)
      try {
        await io.delete(id)
        if (action.signal.aborted) return true
        setGames((previous) => previous.filter((game) => game.id !== id))
        setGamesTotal((total) => Math.max(0, total - 1))
        return true
      } catch (error) {
        if (action.signal.aborted) return false
        console.warn('Falha ao excluir partida:', error)
        setHistoryError('Não foi possível excluir a partida. Tente novamente.')
        return false
      } finally {
        deleting.current = false
        if (actionRequest.current === action) {
          actionRequest.current = null
          historyAction.current = false
          setPendingGame(null)
        } else if (activeRef.current) {
          loadHistory()
        }
      }
    },
    [invalidateHistory, io, loadHistory],
  )

  const reanalyzeStored = useCallback(
    function reanalyzeStored(id: number): void {
      if (historyAction.current || deleting.current) return
      historyAction.current = true
      const action = new AbortController()
      actionRequest.current = action
      setPendingGame(id)
      setHistoryError(null)
      void io
        .get(id, action.signal)
        .then((game) => {
          if (action.signal.aborted) return
          if (!game) {
            setHistoryError('Esta partida não está mais disponível.')
            return
          }
          importPgn(game.pgn)
          setView('new')
          setStep('configure')
          window.scrollTo({ top: 0 })
        })
        .catch((error) => {
          if (action.signal.aborted) return
          console.warn('Falha ao carregar partida:', error)
          setHistoryError('Não foi possível carregar o PGN para reanálise.')
        })
        .finally(() => {
          if (actionRequest.current !== action) return
          actionRequest.current = null
          historyAction.current = false
          setPendingGame(null)
        })
    },
    [importPgn, setView, setStep, io],
  )

  const loadMoreGames = useCallback(
    function loadMoreGames(): void {
      if (
        !nextGamesCursor ||
        loadingMoreGames ||
        historyAction.current ||
        deleting.current
      )
        return
      const controller = new AbortController()
      historyRequest.current?.abort()
      historyRequest.current = controller
      setHistoryError(null)
      setLoadingMoreGames(true)
      void io
        .list(nextGamesCursor, controller.signal)
        .then((page) => {
          if (controller.signal.aborted) return
          setGames((previous) => [...previous, ...page.games])
          setGamesTotal(page.total)
          setNextGamesCursor(page.nextCursor)
        })
        .catch((error) => {
          if (controller.signal.aborted) return
          console.warn('Falha ao carregar mais partidas analisadas:', error)
          setHistoryError(
            'Não foi possível carregar mais partidas. Tente novamente.',
          )
        })
        .finally(() => {
          if (!controller.signal.aborted) setLoadingMoreGames(false)
        })
    },
    [loadingMoreGames, nextGamesCursor, io],
  )

  return {
    games,
    gamesTotal,
    nextGamesCursor,
    loadingHistory,
    loadingMoreGames,
    pendingGame,
    historyError,
    clearGames,
    setHistoryError,
    loadHistory,
    openStored,
    removeStored,
    reanalyzeStored,
    loadMoreGames,
  }
}
