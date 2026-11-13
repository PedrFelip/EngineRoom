import { Moon, Palette, Sun, Volume2 } from 'lucide-react'
import type { CSSProperties } from 'react'
import type { Theme } from '../../lib/settings'
import { useSettings } from '../../lib/settings-context'
import { SwitchField } from '../ui/switch-field'

export default function SettingsPreferences() {
  const settings = useSettings(({ settings }) => ({
    theme: settings.theme,
    soundEnabled: settings.soundEnabled,
    soundVolume: settings.soundVolume,
  }))
  const { setTheme, setSoundEnabled, setSoundVolume } = useSettings(
    (state) => ({
      setTheme: state.setTheme,
      setSoundEnabled: state.setSoundEnabled,
      setSoundVolume: state.setSoundVolume,
    }),
  )

  return (
    <>
      {/* Appearance */}
      <section>
        <h3 className='mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-ink-dim'>
          <Palette size={13} strokeWidth={2.2} aria-hidden='true' />
          Aparência
        </h3>
        <div className='flex rounded-2xl border border-edge-soft bg-panel-2/40 p-1.5'>
          {(['dark', 'light'] as Theme[]).map((t) => {
            const active = settings.theme === t
            return (
              <button
                key={t}
                type='button'
                onClick={() => setTheme(t)}
                aria-pressed={active}
                className={`flex min-w-0 flex-1 items-center justify-center gap-2 rounded-xl px-2 py-3 text-sm font-medium transition sm:px-4 ${
                  active
                    ? 'bg-brand/15 text-brand'
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
        <div className='rounded-2xl border border-edge-soft bg-panel-2/40 p-4'>
          <SwitchField
            label='Som ao avançar lance'
            description='Toca ao navegar para a próxima jogada (não ao voltar).'
            checked={settings.soundEnabled}
            onCheckedChange={setSoundEnabled}
          />
          <label
            className={`mt-3 flex items-center gap-2 sm:gap-3 ${
              settings.soundEnabled ? '' : 'cursor-not-allowed opacity-50'
            }`}
          >
            <span className='w-14 shrink-0 text-xs text-ink-dim sm:w-16'>
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
              {Math.round(settings.soundVolume * 100)}%
            </span>
          </label>
        </div>
      </section>
    </>
  )
}
