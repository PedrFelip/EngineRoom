import { Moon, Palette, Sun, Volume2 } from 'lucide-react'
import type { CSSProperties } from 'react'
import type { Theme } from '../../lib/settings'
import { useSettings } from '../../lib/settings-context'

export default function SettingsPreferences() {
  const { settings, setTheme, setSoundEnabled, setSoundVolume } = useSettings()

  return (
    <>
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
                  active ? 'bg-brand text-bg' : 'text-ink-dim hover:text-ink'
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
            <span className='w-16 shrink-0 text-xs text-ink-dim'>Volume</span>
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
    </>
  )
}
