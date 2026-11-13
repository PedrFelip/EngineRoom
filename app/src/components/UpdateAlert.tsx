import { ExternalLink, TriangleAlert, X } from 'lucide-react'
import { useState } from 'react'
import { useStore } from 'zustand'
import { runUi } from '../lib/effect/ui-runtime'
import { openUpdateReleasePage } from '../lib/updates'
import { updatesStore } from '../lib/updates-store'
import { Button } from './ui/button'

export default function UpdateAlert() {
  const state = useStore(updatesStore)
  const [linkError, setLinkError] = useState<{
    source: string
    message: string
  } | null>(null)

  if (!state.error) return null

  const restartRequired = state.progress?.status === 'restartRequired'
  const canRetryInstall =
    state.errorKind === 'install' && Boolean(state.info?.availableVersion)
  const canRetryRestart = restartRequired || state.errorKind === 'restart'

  let title = 'Não foi possível atualizar'
  if (state.errorKind === 'check') {
    title = 'Não foi possível verificar atualizações'
  } else if (state.errorKind === 'restart') {
    title = 'Não foi possível reiniciar o app'
  } else if (restartRequired) {
    title = 'Reinicie o app para concluir a atualização'
  } else if (state.errorKind === 'install') {
    title = 'Não foi possível instalar a atualização'
  }

  const actionLabel = restartRequired
    ? 'Reiniciar o app'
    : canRetryRestart
      ? 'Tentar reiniciar'
      : canRetryInstall
        ? 'Tentar novamente'
        : null

  return (
    <div
      role='alert'
      className='mt-4 flex w-full max-w-6xl items-start gap-3 rounded-2xl border border-blunder/30 bg-blunder/5 p-4 shadow-sm'
    >
      <span className='mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-xl bg-blunder/10 text-blunder'>
        <TriangleAlert size={18} strokeWidth={2} aria-hidden='true' />
      </span>
      <div className='min-w-0 flex-1'>
        <p className='pr-8 text-sm font-semibold text-ink'>{title}</p>
        <p className='mt-1 text-sm leading-relaxed text-ink-dim'>
          {state.error}
        </p>
        <div className='mt-3 flex flex-wrap items-center gap-2'>
          {actionLabel && (
            <Button
              type='button'
              size='sm'
              variant='outline'
              className='border-blunder/40 text-blunder hover:bg-blunder/10 hover:text-blunder'
              disabled={state.busy}
              onClick={() => {
                void runUi(
                  restartRequired || state.errorKind === 'restart'
                    ? state.restart()
                    : state.install(),
                )
              }}
            >
              {actionLabel}
            </Button>
          )}
          <Button
            type='button'
            size='sm'
            variant='ghost'
            className='text-ink-dim hover:text-ink'
            title='Abrir a página de releases no GitHub'
            onClick={() => {
              setLinkError(null)
              void runUi(
                openUpdateReleasePage(state.info?.availableVersion ?? null),
              ).catch(() => {
                setLinkError({
                  source: state.error ?? '',
                  message:
                    'Não foi possível abrir o navegador. Acesse github.com/PedrFelip/EngineRoom/releases.',
                })
              })
            }}
          >
            <ExternalLink size={14} strokeWidth={2} aria-hidden='true' />
            Baixar no GitHub
          </Button>
        </div>
        {linkError?.source === state.error && (
          <p className='mt-2 text-xs leading-relaxed text-blunder'>
            {linkError.message}
          </p>
        )}
      </div>
      <Button
        type='button'
        variant='ghost'
        size='icon'
        className='-mr-1 -mt-1 shrink-0 text-ink-dim'
        aria-label='Dispensar aviso de atualização'
        onClick={state.dismissError}
      >
        <X size={16} strokeWidth={2} aria-hidden='true' />
      </Button>
    </div>
  )
}
