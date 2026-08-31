import { open } from '@tauri-apps/plugin-dialog'
import {
  Check,
  CircleAlert,
  Cpu,
  Database,
  Moon,
  Palette,
  Sun,
  Volume2,
  X,
} from 'lucide-react'
import { type CSSProperties, useEffect, useState } from 'react'
import { type ProbeResult, probeEngine } from '../lib/engine'
import { clearGames } from '../lib/games'
import type { Theme } from '../lib/settings'
import { useSettings } from '../lib/settings-context'
import {
  clearCache,
  formatBytes,
  getStorageStats,
  type StorageStats,
} from '../lib/storage'

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
  const { settings, setTheme, setEnginePath, setSoundEnabled, setSoundVolume } =
    useSettings()
  const [testing, setTesting] = useState(false)
  const [result, setResult] = useState<ProbeResult | null>(null)
  const [stats, setStats] = useState<StorageStats | null>(null)
  const [confirmTarget, setConfirmTarget] = useState<ConfirmTarget | null>(null)
  const [acting, setActing] = useState(false)
  const [storageError, setStorageError] = useState<string | null>(null)

  useEffect(() => {
    if (!isOpen) return
    let cancelled = false
    setStorageError(null)
    getStorageStats()
      .then((s) => !cancelled && setStats(s))
      .catch((e) => {
        console.warn('Falha ao ler armazenamento:', e)
        if (!cancelled) setStorageError('Não foi possível ler o tamanho.')
      })
    return () => {
      cancelled = true
    }
  }, [isOpen])

  if (!isOpen) return null

  const useCustom = settings.enginePath.trim().length > 0

  async function browse() {
    try {
      const selected = await open({ multiple: false, directory: false })
      if (typeof selected === 'string' && selected) setEnginePath(selected)
    } catch {
      /* not in tauri context */
    }
  }

  async function test() {
    setTesting(true)
    setResult(null)
    const res = await probeEngine(useCustom ? settings.enginePath : undefined)
    setResult(res)
    setTesting(false)
  }

  async function refreshStats() {
    try {
      setStats(await getStorageStats())
    } catch (e) {
      console.warn('Falha ao reler armazenamento:', e)
    }
  }

  async function runClearCache() {
    setActing(true)
    try {
      await clearCache()
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
      await clearGames()
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
    <div className='fixed inset-0 z-50 flex items-center justify-center p-4'>
      <button
        type='button'
        aria-label='Fechar'
        onClick={onClose}
        className='dialog-backdrop absolute inset-0 cursor-default'
      />
      <div
        role='dialog'
        aria-modal='true'
        aria-labelledby='settings-title'
        className='surface-glass elev-dialog relative z-10 w-full max-w-lg overflow-hidden rounded-2xl border border-edge'
      >
        {/* Header */}
        <div className='flex items-center justify-between border-b border-edge-soft px-5 py-4'>
          <h2 id='settings-title' className='text-base font-bold text-ink'>
            Configurações
          </h2>
          <button
            type='button'
            onClick={onClose}
            className='rounded-md p-1.5 text-ink-dim transition hover:bg-panel-3 hover:text-ink'
            aria-label='Fechar'
          >
            <X size={18} strokeWidth={2} aria-hidden='true' />
          </button>
        </div>

        <div className='max-h-[70vh] space-y-6 overflow-y-auto px-5 py-5'>
          {/* Appearance */}
          <section>
            <h3 className='mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-ink-dim'>
              <Palette size={13} strokeWidth={2.2} aria-hidden='true' />
              Aparência
            </h3>
            <div className='inline-flex rounded-lg border border-edge bg-panel-2 p-1'>
              {(['dark', 'light'] as Theme[]).map((t) => {
                const active = settings.theme === t
                return (
                  <button
                    key={t}
                    type='button'
                    onClick={() => setTheme(t)}
                    className={`flex items-center gap-2 rounded-md px-4 py-1.5 text-sm font-medium transition ${
                      active
                        ? 'bg-brand text-bg'
                        : 'text-ink-dim hover:text-ink'
                    }`}
                  >
                    {t === 'dark' ? (
                      <Moon size={15} strokeWidth={2} aria-hidden='true' />
                    ) : (
                      <Sun size={15} strokeWidth={2} aria-hidden='true' />
                    )}
                    {t === 'dark' ? 'Escuro' : 'Claro'}
                  </button>
                )
              })}
            </div>
          </section>

          {/* Som */}
          <section>
            <h3 className='mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-ink-dim'>
              <Volume2 size={13} strokeWidth={2.2} aria-hidden='true' />
              Som
            </h3>
            <div className='rounded-lg border border-edge-soft p-3'>
              <label className='flex cursor-pointer items-center justify-between gap-3'>
                <span className='min-w-0 flex-1'>
                  <span className='block text-sm font-medium text-ink'>
                    Som ao avançar lance
                  </span>
                  <span className='block text-xs text-ink-faint'>
                    Toca ao navegar para a próxima jogada (não ao voltar).
                  </span>
                </span>
                <input
                  type='checkbox'
                  checked={settings.soundEnabled}
                  onChange={(e) => setSoundEnabled(e.target.checked)}
                  className='h-4 w-4 accent-[var(--brand)]'
                  aria-label='Ativar som de movimentação'
                />
              </label>
              <label
                className={`mt-3 flex items-center gap-3 ${
                  settings.soundEnabled ? '' : 'cursor-not-allowed opacity-50'
                }`}
              >
                <span className='w-16 shrink-0 text-xs text-ink-dim'>
                  Volume
                </span>
                <input
                  type='range'
                  min={0}
                  max={100}
                  value={Math.round(settings.soundVolume * 100)}
                  onChange={(e) => setSoundVolume(Number(e.target.value) / 100)}
                  disabled={!settings.soundEnabled}
                  className='engine-range min-w-0 flex-1'
                  style={
                    {
                      '--fill': `${Math.round(settings.soundVolume * 100)}%`,
                    } as CSSProperties
                  }
                  aria-label='Volume do som'
                />
                <span className='w-10 shrink-0 text-right font-mono text-xs text-ink-dim'>
                  {Math.round(settings.soundVolume * 100)}
                </span>
              </label>
            </div>
          </section>

          {/* Engine */}
          <section>
            <h3 className='mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-ink-dim'>
              <Cpu size={13} strokeWidth={2.2} aria-hidden='true' />
              Engine Stockfish
            </h3>

            <label className='flex cursor-pointer items-start gap-3 rounded-lg border border-edge-soft p-3 transition hover:bg-panel-2/60'>
              <input
                type='radio'
                name='engine-src'
                checked={!useCustom}
                onChange={() => setEnginePath('')}
                className='mt-0.5 accent-[var(--brand)]'
              />
              <span>
                <span className='block text-sm font-medium text-ink'>
                  Stockfish embarcado
                </span>
                <span className='block text-xs text-ink-faint'>
                  Usa a engine que vem junto com o app (recomendado).
                </span>
              </span>
            </label>

            <label className='mt-2 flex cursor-pointer items-start gap-3 rounded-lg border border-edge-soft p-3 transition hover:bg-panel-2/60'>
              <input
                type='radio'
                name='engine-src'
                checked={useCustom}
                onChange={() =>
                  setEnginePath(settings.enginePath || '/usr/bin/stockfish')
                }
                className='mt-0.5 accent-[var(--brand)]'
              />
              <span className='min-w-0 flex-1'>
                <span className='block text-sm font-medium text-ink'>
                  Caminho customizado
                </span>
                <span className='mb-2 block text-xs text-ink-faint'>
                  Use um Stockfish instalado no seu sistema.
                </span>
                <div className='flex gap-2'>
                  <input
                    type='text'
                    value={settings.enginePath}
                    onChange={(e) => setEnginePath(e.target.value)}
                    placeholder='/usr/bin/stockfish'
                    spellCheck={false}
                    className='min-w-0 flex-1 rounded-md border border-edge bg-panel-2 px-2.5 py-1.5 font-mono text-xs text-ink outline-none focus:border-brand'
                  />
                  <button
                    type='button'
                    onClick={browse}
                    className='shrink-0 rounded-md border border-edge bg-panel-2 px-3 py-1.5 text-xs font-medium text-ink-dim transition hover:bg-panel-3 hover:text-ink'
                  >
                    Procurar…
                  </button>
                </div>
              </span>
            </label>

            <div className='mt-3 flex items-center gap-3'>
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
          <section>
            <h3 className='mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-ink-dim'>
              <Database size={13} strokeWidth={2.2} aria-hidden='true' />
              Armazenamento
            </h3>
            <div className='rounded-lg border border-edge-soft p-3'>
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
                      : 'Apagar TODO o histórico de partidas? Esta ação não pode ser desfeita.'}
                  </p>
                  <div className='mt-2 flex justify-end gap-2'>
                    <button
                      type='button'
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
                    onClick={() => setConfirmTarget('cache')}
                    disabled={acting}
                    className='rounded-md border border-edge bg-panel-2 px-3 py-1.5 text-xs font-medium text-ink-dim transition hover:bg-blunder/15 hover:text-blunder disabled:opacity-60'
                  >
                    Limpar cache
                  </button>
                  <button
                    type='button'
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

        <div className='flex justify-end border-t border-edge-soft px-5 py-3'>
          <button
            type='button'
            onClick={onClose}
            className='rounded-lg bg-brand px-5 py-2 text-sm font-semibold text-bg transition hover:bg-brand-strong'
          >
            Concluído
          </button>
        </div>
      </div>
    </div>
  )
}
