import { open } from '@tauri-apps/plugin-dialog'
import { useState } from 'react'
import { type ProbeResult, probeEngine } from '../lib/engine'
import type { Theme } from '../lib/settings'
import { useSettings } from '../lib/settings-context'

interface Props {
  open: boolean
  onClose: () => void
}

export default function SettingsModal({ open: isOpen, onClose }: Props) {
  const { settings, setTheme, setEnginePath, setSoundEnabled, setSoundVolume } =
    useSettings()
  const [testing, setTesting] = useState(false)
  const [result, setResult] = useState<ProbeResult | null>(null)

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

  return (
    <div className='fixed inset-0 z-50 flex items-center justify-center p-4'>
      <button
        type='button'
        aria-label='Fechar'
        onClick={onClose}
        className='absolute inset-0 cursor-default bg-black/60'
      />
      <div
        role='dialog'
        aria-modal='true'
        aria-labelledby='settings-title'
        className='relative z-10 w-full max-w-lg overflow-hidden rounded-2xl border border-edge bg-panel shadow-2xl'
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
            <svg
              width='18'
              height='18'
              viewBox='0 0 24 24'
              fill='none'
              stroke='currentColor'
              strokeWidth='2'
              strokeLinecap='round'
              strokeLinejoin='round'
              aria-hidden='true'
            >
              <line x1='18' y1='6' x2='6' y2='18' />
              <line x1='6' y1='6' x2='18' y2='18' />
            </svg>
          </button>
        </div>

        <div className='max-h-[70vh] space-y-6 overflow-y-auto px-5 py-5'>
          {/* Appearance */}
          <section>
            <h3 className='mb-2 text-xs font-semibold uppercase tracking-wide text-ink-dim'>
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
                      <svg
                        width='15'
                        height='15'
                        viewBox='0 0 24 24'
                        fill='none'
                        stroke='currentColor'
                        strokeWidth='2'
                        strokeLinecap='round'
                        strokeLinejoin='round'
                        aria-hidden='true'
                      >
                        <path d='M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z' />
                      </svg>
                    ) : (
                      <svg
                        width='15'
                        height='15'
                        viewBox='0 0 24 24'
                        fill='none'
                        stroke='currentColor'
                        strokeWidth='2'
                        strokeLinecap='round'
                        strokeLinejoin='round'
                        aria-hidden='true'
                      >
                        <circle cx='12' cy='12' r='5' />
                        <line x1='12' y1='1' x2='12' y2='3' />
                        <line x1='12' y1='21' x2='12' y2='23' />
                        <line x1='4.22' y1='4.22' x2='5.64' y2='5.64' />
                        <line x1='18.36' y1='18.36' x2='19.78' y2='19.78' />
                        <line x1='1' y1='12' x2='3' y2='12' />
                        <line x1='21' y1='12' x2='23' y2='12' />
                        <line x1='4.22' y1='19.78' x2='5.64' y2='18.36' />
                        <line x1='18.36' y1='5.64' x2='19.78' y2='4.22' />
                      </svg>
                    )}
                    {t === 'dark' ? 'Escuro' : 'Claro'}
                  </button>
                )
              })}
            </div>
          </section>

          {/* Som */}
          <section>
            <h3 className='mb-2 text-xs font-semibold uppercase tracking-wide text-ink-dim'>
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
                  className='min-w-0 flex-1 accent-[var(--brand)]'
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
            <h3 className='mb-2 text-xs font-semibold uppercase tracking-wide text-ink-dim'>
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
                    <svg
                      width='15'
                      height='15'
                      viewBox='0 0 24 24'
                      fill='none'
                      stroke='currentColor'
                      strokeWidth='2.5'
                      strokeLinecap='round'
                      strokeLinejoin='round'
                      aria-hidden='true'
                    >
                      <polyline points='20 6 9 17 4 12' />
                    </svg>
                  ) : (
                    <svg
                      width='15'
                      height='15'
                      viewBox='0 0 24 24'
                      fill='none'
                      stroke='currentColor'
                      strokeWidth='2'
                      strokeLinecap='round'
                      strokeLinejoin='round'
                      aria-hidden='true'
                    >
                      <circle cx='12' cy='12' r='10' />
                      <line x1='12' y1='8' x2='12' y2='12' />
                      <line x1='12' y1='16' x2='12.01' y2='16' />
                    </svg>
                  )}
                  {result.ok
                    ? (result.name ?? 'Engine respondeu (uciok)')
                    : (result.error ?? 'Falhou')}
                </span>
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
