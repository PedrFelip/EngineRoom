import { useEffect, useRef, useState } from 'react'
import { useSettings } from '../lib/settings-context'

interface ReviewSettingsPanelProps {
  liveWideAvailable: boolean
  liveWideOn: boolean
  onToggleWide: (on: boolean) => void
  onApplyResources: (threads: number, hashMb: number) => void
}

const HASH_PRESETS_MB = [64, 128, 256, 512, 1024, 2048, 4096]

/**
 * Header dropdown for live refinement settings: Threads/Hash sliders apply to
 * the heavy engine, and a toggle controls the wide (light) engine.
 */
export default function ReviewSettingsPanel({
  liveWideAvailable,
  liveWideOn,
  onToggleWide,
  onApplyResources,
}: ReviewSettingsPanelProps) {
  const { settings } = useSettings()
  const [open, setOpen] = useState(false)
  const [threads, setThreads] = useState(settings.liveThreads)
  const [hashMb, setHashMb] = useState(settings.liveHashMb)
  const containerRef = useRef<HTMLDivElement | null>(null)

  // biome-ignore lint/correctness/useExhaustiveDependencies: sincroniza só quando o painel abre
  useEffect(() => {
    if (open) {
      setThreads(settings.liveThreads)
      setHashMb(settings.liveHashMb)
    }
  }, [open])

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

  const commit = () => {
    onApplyResources(threads, hashMb)
    setOpen(false)
  }

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
          <div className='mb-3'>
            <div className='mb-1 flex items-center justify-between'>
              <label
                htmlFor='live-threads'
                className='text-xs font-semibold uppercase tracking-wide text-ink-faint'
              >
                Threads (pesada)
              </label>
              <span className='font-mono text-xs text-ink'>{threads}</span>
            </div>
            <input
              id='live-threads'
              type='range'
              min={1}
              max={32}
              step={1}
              value={threads}
              onChange={(e) => setThreads(Number(e.target.value))}
              className='w-full accent-brand'
            />
          </div>
          <div className='mb-3'>
            <div className='mb-1 flex items-center justify-between'>
              <label
                htmlFor='live-hash'
                className='text-xs font-semibold uppercase tracking-wide text-ink-faint'
              >
                Hash MB (pesada)
              </label>
              <span className='font-mono text-xs text-ink'>{hashMb}</span>
            </div>
            <input
              id='live-hash'
              type='range'
              min={0}
              max={HASH_PRESETS_MB.length - 1}
              step={1}
              value={Math.max(0, HASH_PRESETS_MB.indexOf(hashMb))}
              onChange={(e) =>
                setHashMb(HASH_PRESETS_MB[Number(e.target.value)] ?? 256)
              }
              className='w-full accent-brand'
            />
            <div className='mt-0.5 flex justify-between text-[10px] text-ink-faint'>
              <span>64</span>
              <span>4096</span>
            </div>
          </div>
          <div className='flex justify-end gap-2'>
            <button
              type='button'
              onClick={() => setOpen(false)}
              className='rounded-lg px-3 py-1 text-xs text-ink-dim hover:bg-panel-3'
            >
              Cancelar
            </button>
            <button
              type='button'
              onClick={commit}
              className='rounded-lg bg-brand px-3 py-1 text-xs font-medium text-white hover:opacity-90'
            >
              Aplicar
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
