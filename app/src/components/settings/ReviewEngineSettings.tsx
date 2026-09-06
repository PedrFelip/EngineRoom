import { Cpu, MemoryStick, Split, Timer } from 'lucide-react'
import type { CSSProperties, ReactNode } from 'react'
import { useEffect, useState } from 'react'
import { recommendedReviewThreads } from '../../lib/settings'
import { useSettings } from '../../lib/settings-context'
import { selectReviewEngineSettings } from '../../lib/settings-store'
import { getSystemResources } from '../../lib/system'
import { Switch } from '../ui/switch'

function fill(value: number, min: number, max: number): CSSProperties {
  if (min === max) return { '--fill': '100%' } as CSSProperties
  return {
    '--fill': `${((value - min) / (max - min)) * 100}%`,
  } as CSSProperties
}

export default function ReviewEngineSettings() {
  const settings = useSettings(selectReviewEngineSettings)
  const updateSettings = useSettings((state) => state.updateSettings)
  const [maxThreads, setMaxThreads] = useState(
    Math.max(1, navigator.hardwareConcurrency || 1),
  )

  useEffect(() => {
    let cancelled = false
    getSystemResources()
      .then(({ threads }) => {
        if (cancelled) return
        const available = Math.max(1, threads)
        setMaxThreads(available)
        if (settings.reviewThreadsAuto) {
          updateSettings({ reviewThreads: recommendedReviewThreads(available) })
        }
      })
      .catch(() => {})
    return () => {
      cancelled = true
    }
  }, [settings.reviewThreadsAuto, updateSettings])

  const threads = Math.min(settings.reviewThreads, maxThreads)
  const disabled = !settings.reviewEngineEnabled

  return (
    <div className='rounded-lg border border-edge-soft p-3'>
      <div className={`space-y-4 ${disabled ? 'opacity-45' : ''}`}>
        <SettingRange
          icon={<Timer size={13} aria-hidden='true' />}
          label='Tempo de busca'
          value={settings.reviewSearchSeconds}
          suffix='s'
          min={1}
          max={30}
          disabled={disabled}
          onChange={(reviewSearchSeconds) =>
            updateSettings({ reviewSearchSeconds })
          }
        />
        <label
          htmlFor='review-move-feedback-switch'
          className='flex items-center justify-between gap-3 text-xs text-ink-dim'
        >
          <span>
            <span className='block font-medium text-ink'>
              Feedback dos movimentos
            </span>
            <span className='mt-0.5 block text-ink-faint'>
              Classifica lances jogados em novas linhas
            </span>
          </span>
          <Switch
            id='review-move-feedback-switch'
            size='sm'
            checked={settings.reviewMoveFeedbackEnabled}
            disabled={disabled}
            onCheckedChange={(reviewMoveFeedbackEnabled) =>
              updateSettings({ reviewMoveFeedbackEnabled })
            }
            aria-label='Feedback dos movimentos'
          />
        </label>
        <SettingRange
          icon={<Split size={13} aria-hidden='true' />}
          label='Linhas de análise'
          value={settings.reviewAnalysisLines}
          suffix=' / 5'
          min={1}
          max={5}
          disabled={disabled}
          onChange={(reviewAnalysisLines) =>
            updateSettings({ reviewAnalysisLines })
          }
        />
        <SettingRange
          icon={<Cpu size={13} aria-hidden='true' />}
          label='Threads'
          value={threads}
          suffix={` / ${maxThreads}`}
          min={1}
          max={maxThreads}
          disabled={disabled || settings.reviewThreadsAuto}
          onChange={(reviewThreads) => updateSettings({ reviewThreads })}
        />
        <label className='flex items-center justify-end gap-2 text-xs text-ink-dim'>
          <input
            type='checkbox'
            checked={settings.reviewThreadsAuto}
            disabled={disabled}
            onChange={(event) => {
              const reviewThreadsAuto = event.target.checked
              updateSettings({
                reviewThreadsAuto,
                ...(reviewThreadsAuto
                  ? { reviewThreads: recommendedReviewThreads(maxThreads) }
                  : {}),
              })
            }}
            className='accent-[var(--brand)]'
          />
          Adaptar ao computador
        </label>
        <SettingRange
          icon={<MemoryStick size={13} aria-hidden='true' />}
          label='Memória'
          value={settings.reviewMemoryMb}
          suffix=' MB'
          min={16}
          max={1024}
          step={16}
          disabled={disabled}
          onChange={(reviewMemoryMb) => updateSettings({ reviewMemoryMb })}
        />
      </div>
    </div>
  )
}

interface SettingRangeProps {
  icon: ReactNode
  label: string
  value: number
  suffix: string
  min: number
  max: number
  step?: number
  disabled: boolean
  onChange: (value: number) => void
}

function SettingRange(props: SettingRangeProps) {
  return (
    <label className='block'>
      <span className='mb-1.5 flex items-center justify-between text-xs text-ink-dim'>
        <span className='flex items-center gap-1.5'>
          {props.icon}
          {props.label}
        </span>
        <span className='font-mono text-ink'>
          {props.value}
          {props.suffix}
        </span>
      </span>
      <input
        type='range'
        min={props.min}
        max={props.max}
        step={props.step ?? 1}
        value={props.value}
        disabled={props.disabled}
        onChange={(event) => props.onChange(Number(event.target.value))}
        className='engine-range w-full disabled:cursor-not-allowed'
        style={fill(props.value, props.min, props.max)}
        aria-label={props.label}
      />
    </label>
  )
}
