import { useEffect, useRef, useState } from 'react'
import { useSettings } from '../lib/settings-context'
import type { LivePreset } from '../lib/system'

interface ReviewSettingsPanelProps {
  liveWideAvailable: boolean
  liveWideOn: boolean
  onToggleWide: (on: boolean) => void
  presets: LivePreset[] | null
  onApplyPreset: (preset: LivePreset) => void
}

function describeSizing(s: LivePreset['deep']): string {
  return `${s.threads}T · ${s.hashMb >= 1024 ? `${(s.hashMb / 1024).toFixed(1)}G` : `${s.hashMb}MB`}`
}

/**
 * Header dropdown for live refinement settings: a preset (Leve / Equilibrado /
 * Pesado) sizes both engines, and a toggle controls the wide (light) engine.
 */
export default function ReviewSettingsPanel({
  liveWideAvailable,
  liveWideOn,
  onToggleWide,
  presets,
  onApplyPreset,
}: ReviewSettingsPanelProps) {
  const { settings } = useSettings()
  const [open, setOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement | null>(null)

  // Click-outside closes the panel without committing pending slider values.
  useEffect(() => {
    if (!open) return
    const onClick = (e: MouseEvent) => {
      if (!containerRef.current) return
      if (!containerRef.current.contains(e.target as Node)) setOpen(false)
    }
    window.addEventListener('mousedown', onClick)
    return () => window.removeEventListener('mousedown', onClick)
  }, [open])

  return (
    <div className='relative flex items-center gap-1' ref={containerRef}>
      {liveWideAvailable && (
        <button
          type='button'
          title={liveWideOn ? 'Desligar engine leve' : 'Ligar engine leve'}
          onClick={() => onToggleWide(!liveWideOn)}
          className={`rounded-lg px-2 py-1.5 text-sm transition ${
            liveWideOn
              ? 'bg-brand/20 text-brand ring-1 ring-brand/40'
              : 'bg-panel-3 text-ink-dim hover:bg-edge'
          }`}
          aria-pressed={liveWideOn}
        >
          {liveWideOn ? '●' : '○'} AO VIVO
        </button>
      )}
      <button
        type='button'
        title='Configurações da engine'
        onClick={() => setOpen((v) => !v)}
        className='rounded-lg bg-panel-3 px-2 py-1.5 text-sm text-ink-dim transition hover:bg-edge'
        aria-expanded={open}
      >
        ⚙
      </button>
      {open && (
        <div className='absolute right-0 top-full z-20 mt-1 w-72 rounded-xl border border-edge bg-panel-2 p-3 shadow-xl'>
          <div className='mb-2 text-[11px] font-semibold uppercase tracking-wide text-ink-faint'>
            Recursos (engine pesada + leve)
          </div>
          <div className='flex flex-col gap-1'>
            {(presets ?? []).map((p) => {
              const active = p.id === settings.livePreset
              return (
                <button
                  key={p.id}
                  type='button'
                  onClick={() => {
                    onApplyPreset(p)
                    setOpen(false)
                  }}
                  className={`flex items-center justify-between rounded-lg px-3 py-2 text-left text-sm transition ${
                    active
                      ? 'bg-brand/15 ring-1 ring-brand/40 text-ink'
                      : 'bg-panel-3/40 text-ink-dim hover:bg-panel-3/60'
                  }`}
                >
                  <span className='font-medium'>{p.label}</span>
                  <span className='font-mono text-xs text-ink-faint'>
                    {describeSizing(p.deep)}
                  </span>
                </button>
              )
            })}
            {!presets && (
              <div className='rounded-lg bg-panel-3/40 px-3 py-2 text-xs text-ink-faint'>
                Detectando recursos da máquina…
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
