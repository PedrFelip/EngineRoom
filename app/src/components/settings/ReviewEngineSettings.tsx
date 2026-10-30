import { Cpu, MemoryStick, Split, Timer } from 'lucide-react'
import type { CSSProperties, ReactNode } from 'react'
import { useEffect, useState } from 'react'
import { runUi } from '../../lib/effect/ui-runtime'
import { reviewThreadSettings } from '../../lib/settings'
import { useSettings } from '../../lib/settings-context'
import { selectReviewEngineSettings } from '../../lib/settings-store'
import { getSystemResources } from '../../lib/system'
import { SwitchField } from '../ui/switch-field'

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
    const controller = new AbortController()
    runUi(getSystemResources(), controller.signal)
      .then(({ threads }) => {
        if (controller.signal.aborted) return
        const available = Math.max(1, threads)
        setMaxThreads(available)
        if (settings.reviewThreadsAuto) {
          updateSettings(reviewThreadSettings(true, available))
        }
      })
      .catch(() => {})
    return () => {
      controller.abort()
    }
  }, [settings.reviewThreadsAuto, updateSettings])

  const threads = Math.min(settings.reviewThreads, maxThreads)
  const disabled = !settings.reviewEngineEnabled

  return (
    <div className='rounded-2xl border border-edge-soft bg-panel-2/40 p-4'>
      <div
        className={`grid gap-4 sm:grid-cols-2 sm:gap-x-6 ${disabled ? 'opacity-45' : ''}`}
      >
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
        <div>
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
          <SwitchField
            className='mt-2'
            size='sm'
            label='Adaptar ao computador'
            checked={settings.reviewThreadsAuto}
            disabled={disabled}
            onCheckedChange={(reviewThreadsAuto) => {
              updateSettings(
                reviewThreadSettings(reviewThreadsAuto, maxThreads),
              )
            }}
          />
        </div>
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
        <SwitchField
          className='border-t border-edge-soft pt-3 sm:col-span-2'
          size='sm'
          label='Feedback dos movimentos'
          description='Classifica lances jogados em novas linhas'
          checked={settings.reviewMoveFeedbackEnabled}
          disabled={disabled}
          onCheckedChange={(reviewMoveFeedbackEnabled) =>
            updateSettings({ reviewMoveFeedbackEnabled })
          }
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
      <span className='mb-1.5 flex items-center justify-between gap-3 text-xs text-ink-dim'>
        <span className='flex min-w-0 items-center gap-1.5'>
          {props.icon}
          {props.label}
        </span>
        <span className='shrink-0 font-mono tabular-nums text-ink'>
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
