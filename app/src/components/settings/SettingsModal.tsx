import { Check, CircleAlert, Cpu, Database, Palette, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { runUi } from '../../lib/effect/ui-runtime'
import { type ProbeResult, probeEngine } from '../../lib/engine'
import { clearGames } from '../../lib/games'
import {
  clearCache,
  formatBytes,
  getStorageStats,
  type StorageStats,
} from '../../lib/storage'
import { useModalFocus } from '../../lib/use-modal-focus'
import { SegmentedControl } from '../ui/segmented-control'
import ReviewEngineSettings from './ReviewEngineSettings'
import SettingsPreferences from './SettingsPreferences'

interface Props {
  open: boolean
  onClose: () => void
  /** Notifica o parent (HomePage) que o histórico foi zerado para esvaziar a lista. */
  onGamesCleared?: () => void
}

type ConfirmTarget = 'cache' | 'games'

export default function SettingsModal({
  open: isOpen,
  onClose,
  onGamesCleared,
}: Props) {
  const [section, setSection] = useState('preferences')
  const dialogRef = useRef<HTMLDivElement>(null)
  const contentRef = useRef<HTMLDivElement>(null)
  const [testing, setTesting] = useState(false)
  const [result, setResult] = useState<ProbeResult | null>(null)
  const [stats, setStats] = useState<StorageStats | null>(null)
  const [confirmTarget, setConfirmTarget] = useState<ConfirmTarget | null>(null)
  const [acting, setActing] = useState(false)
  const [storageError, setStorageError] = useState<string | null>(null)
  const probeController = useRef<AbortController | null>(null)

  useEffect(() => {
    if (!isOpen) return
    const controller = new AbortController()
    setStorageError(null)
    runUi(getStorageStats(), controller.signal)
      .then((s) => !controller.signal.aborted && setStats(s))
      .catch((e) => {
        if (controller.signal.aborted) return
        console.warn('Falha ao ler armazenamento:', e)
        setStorageError('Não foi possível ler o tamanho.')
      })
    return () => {
      controller.abort()
      probeController.current?.abort()
    }
  }, [isOpen])

  const cancelRef = useRef<HTMLButtonElement>(null)
  const clearCacheRef = useRef<HTMLButtonElement>(null)
  const clearGamesRef = useRef<HTMLButtonElement>(null)
  const previousConfirmation = useRef<ConfirmTarget | null>(null)

  function requestClose() {
    if (acting) return
    if (confirmTarget) setConfirmTarget(null)
    else onClose()
  }

  useModalFocus(isOpen, dialogRef, requestClose)

  useEffect(() => {
    if (!isOpen) return
    setConfirmTarget(null)
    setResult(null)
    setStats(null)
  }, [isOpen])

  useEffect(() => {
    if (!isOpen) return
    if (confirmTarget) cancelRef.current?.focus()
    else if (previousConfirmation.current) {
      const trigger =
        previousConfirmation.current === 'cache' ? clearCacheRef : clearGamesRef
      trigger.current?.focus()
    }
    previousConfirmation.current = confirmTarget
  }, [confirmTarget, isOpen])

  if (!isOpen) return null

  async function test() {
    const controller = new AbortController()
    probeController.current?.abort()
    probeController.current = controller
    setTesting(true)
    setResult(null)
    try {
      const res = await runUi(probeEngine(), controller.signal)
      if (!controller.signal.aborted) setResult(res)
    } catch (error) {
      if (!controller.signal.aborted) {
        setResult({ ok: false, name: null, error: String(error) })
      }
    } finally {
      if (probeController.current === controller) {
        probeController.current = null
        setTesting(false)
      }
    }
  }

  async function refreshStats() {
    try {
      setStats(await runUi(getStorageStats()))
    } catch (e) {
      console.warn('Falha ao reler armazenamento:', e)
    }
  }

  async function runClearCache() {
    setActing(true)
    try {
      await runUi(clearCache())
      await refreshStats()
      setStorageError(null)
    } catch (e) {
      console.warn('Falha ao limpar cache:', e)
      setStorageError('Falha ao limpar o cache.')
    } finally {
      setActing(false)
      setConfirmTarget(null)
    }
  }

  async function runClearGames() {
    setActing(true)
    try {
      await runUi(clearGames())
      await refreshStats()
      onGamesCleared?.()
      setStorageError(null)
    } catch (e) {
      console.warn('Falha ao limpar histórico:', e)
      setStorageError('Falha ao limpar o histórico.')
    } finally {
      setActing(false)
      setConfirmTarget(null)
    }
  }

  return (
    <div className='fixed inset-0 z-50 flex items-center justify-center p-2 sm:p-4'>
      <button
        type='button'
        aria-label='Fechar'
        onClick={requestClose}
        className='dialog-backdrop absolute inset-0 cursor-default'
      />
      <div
        ref={dialogRef}
        tabIndex={-1}
        role='dialog'
        aria-modal='true'
        aria-labelledby='settings-title'
        className='surface-glass elev-dialog relative z-10 flex max-h-[calc(100dvh-1rem)] w-full max-w-2xl flex-col overflow-hidden rounded-2xl border border-edge outline-none sm:max-h-[calc(100dvh-2rem)] sm:rounded-3xl'
      >
        <div
          className='sr-only'
          role='status'
          aria-live='polite'
          aria-atomic='true'
        >
          {storageError}
          {result &&
            (result.ok
              ? (result.name ?? 'Engine respondeu (uciok)')
              : (result.error ?? 'Falhou'))}
        </div>
        {/* Header */}
        <div className='flex shrink-0 items-center justify-between gap-3 border-b border-edge-soft px-4 py-3 sm:px-5 sm:py-4'>
          <div>
            <h2
              id='settings-title'
              className='text-lg font-semibold tracking-tight text-ink sm:text-xl'
            >
              Configurações
            </h2>
            <p className='mt-1 text-xs text-ink-dim'>
              Ajustes salvos automaticamente neste computador.
            </p>
          </div>
          <button
            type='button'
            onClick={requestClose}
            className='rounded-md p-1.5 text-ink-dim transition hover:bg-panel-3 hover:text-ink'
            aria-label='Fechar'
          >
            <X size={18} strokeWidth={2} aria-hidden='true' />
          </button>
        </div>

        <div className='shrink-0 border-b border-edge-soft px-3 py-2 sm:px-5'>
          <SegmentedControl
            label='Seções das configurações'
            value={section}
            disabled={acting || confirmTarget !== null}
            stackedOnMobile
            options={[
              {
                id: 'preferences',
                label: 'Preferências',
                icon: Palette,
                controls: 'settings-preferences',
              },
              {
                id: 'analysis',
                label: 'Análise',
                icon: Cpu,
                controls: 'settings-analysis settings-engine',
              },
              {
                id: 'storage',
                label: 'Armazenamento',
                icon: Database,
                controls: 'settings-storage',
              },
            ]}
            onChange={(nextSection) => {
              setSection(nextSection)
              contentRef.current?.scrollTo({ top: 0 })
            }}
          />
        </div>
        <div
          ref={contentRef}
          className='grid min-h-0 gap-4 overflow-y-auto overscroll-contain px-3 py-4 sm:px-5'
        >
          <div id='settings-preferences' hidden={section !== 'preferences'}>
            <div className='grid gap-4 sm:grid-cols-2'>
              <SettingsPreferences />
            </div>
          </div>

          <section id='settings-analysis' hidden={section !== 'analysis'}>
            <h3 className='mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-ink-dim'>
              <Cpu size={13} strokeWidth={2.2} aria-hidden='true' />
              Análise na revisão
            </h3>
            <ReviewEngineSettings />
          </section>

          {/* Engine */}
          <section id='settings-engine' hidden={section !== 'analysis'}>
            <h3 className='mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-ink-dim'>
              <Cpu size={13} strokeWidth={2.2} aria-hidden='true' />
              Engine Stockfish
            </h3>

            <div className='rounded-2xl border border-edge-soft bg-panel-2/40 p-4'>
              <span>
                <span className='block text-sm font-medium text-ink'>
                  Stockfish embarcado
                </span>
                <span className='block text-xs text-ink-faint'>
                  Usa exclusivamente a engine que vem junto com o app.
                </span>
              </span>
            </div>

            <div className='mt-3 flex flex-wrap items-center gap-3'>
              <button
                type='button'
                onClick={test}
                disabled={testing}
                className='rounded-lg border border-edge bg-panel-2 px-4 py-2 text-sm font-medium text-ink transition hover:bg-panel-3 disabled:opacity-60'
              >
                {testing ? 'Testando…' : 'Testar engine'}
              </button>

              {result && (
                <span
                  className={`flex items-center gap-1.5 text-sm ${
                    result.ok ? 'text-good' : 'text-blunder'
                  }`}
                >
                  {result.ok ? (
                    <Check size={15} strokeWidth={2.5} aria-hidden='true' />
                  ) : (
                    <CircleAlert
                      size={15}
                      strokeWidth={2.5}
                      aria-hidden='true'
                    />
                  )}
                  {result.ok
                    ? (result.name ?? 'Engine respondeu (uciok)')
                    : (result.error ?? 'Falhou')}
                </span>
              )}
            </div>
          </section>

          {/* Armazenamento */}
          <section id='settings-storage' hidden={section !== 'storage'}>
            <h3 className='mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-ink-dim'>
              <Database size={13} strokeWidth={2.2} aria-hidden='true' />
              Armazenamento
            </h3>
            <div className='rounded-2xl border border-edge-soft bg-panel-2/40 p-4'>
              <dl className='space-y-1.5 text-sm'>
                <div className='flex items-center justify-between gap-3'>
                  <dt className='text-ink-dim'>Cache de posições</dt>
                  <dd className='font-mono text-ink'>
                    {stats ? formatBytes(stats.cacheBytes) : '—'}
                  </dd>
                </div>
                <div className='flex items-center justify-between gap-3'>
                  <dt className='text-ink-dim'>Histórico de partidas</dt>
                  <dd className='font-mono text-ink'>
                    {stats ? formatBytes(stats.gamesBytes) : '—'}
                  </dd>
                </div>
                <div className='mt-1.5 border-t border-edge-soft pt-1.5' />
                <div className='flex items-center justify-between gap-3'>
                  <dt className='text-ink-dim'>Total do banco</dt>
                  <dd className='font-mono text-ink'>
                    {stats ? formatBytes(stats.dbBytes) : '—'}
                  </dd>
                </div>
              </dl>

              {storageError && (
                <p className='mt-2 text-xs text-blunder'>{storageError}</p>
              )}

              {confirmTarget && (
                <div className='mt-3 rounded-md border border-blunder/30 bg-blunder/10 p-2.5 text-xs'>
                  <p className='text-blunder'>
                    {confirmTarget === 'cache'
                      ? 'Apagar todo o cache de posições? As próximas análises vão precisar recalcular.'
                      : 'Apagar todo o histórico de partidas? Esta ação não pode ser desfeita.'}
                  </p>
                  <div className='mt-2 flex justify-end gap-2'>
                    <button
                      type='button'
                      ref={cancelRef}
                      onClick={() => setConfirmTarget(null)}
                      disabled={acting}
                      className='rounded-md border border-edge bg-panel-2 px-3 py-1 font-medium text-ink-dim transition hover:bg-panel-3 hover:text-ink disabled:opacity-60'
                    >
                      Cancelar
                    </button>
                    <button
                      type='button'
                      onClick={
                        confirmTarget === 'cache'
                          ? runClearCache
                          : runClearGames
                      }
                      disabled={acting}
                      className='rounded-md bg-blunder px-3 py-1 font-semibold text-bg transition hover:bg-blunder-strong disabled:opacity-60'
                    >
                      {acting ? 'Limpando…' : 'Confirmar'}
                    </button>
                  </div>
                </div>
              )}

              {!confirmTarget && (
                <div className='mt-3 flex flex-wrap gap-2'>
                  <button
                    type='button'
                    ref={clearCacheRef}
                    onClick={() => setConfirmTarget('cache')}
                    disabled={acting}
                    className='rounded-md border border-edge bg-panel-2 px-3 py-1.5 text-xs font-medium text-ink-dim transition hover:bg-blunder/15 hover:text-blunder disabled:opacity-60'
                  >
                    Limpar cache
                  </button>
                  <button
                    type='button'
                    ref={clearGamesRef}
                    onClick={() => setConfirmTarget('games')}
                    disabled={acting}
                    className='rounded-md border border-edge bg-panel-2 px-3 py-1.5 text-xs font-medium text-ink-dim transition hover:bg-blunder/15 hover:text-blunder disabled:opacity-60'
                  >
                    Limpar histórico
                  </button>
                </div>
              )}
            </div>
          </section>
        </div>

        <div className='flex shrink-0 justify-end border-t border-edge-soft px-5 py-3'>
          <button
            type='button'
            onClick={requestClose}
            className='rounded-lg bg-brand px-5 py-2 text-sm font-semibold text-bg transition hover:bg-brand-strong'
          >
            Concluído
          </button>
        </div>
      </div>
    </div>
  )
}
