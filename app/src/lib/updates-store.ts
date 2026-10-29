import { Effect } from 'effect'
import { createStore } from 'zustand/vanilla'
import {
  checkAppUpdate,
  installAppUpdate,
  restartAfterFailedUpdate,
  type UpdateInfo,
  type UpdateProgress,
} from './updates'

type Status = 'checking' | 'current' | 'available' | 'unsupported' | 'error'

interface UpdateState {
  status: Status
  info: UpdateInfo | null
  progress: UpdateProgress | null
  busy: boolean
  error: string | null
  check: () => Effect.Effect<void>
  install: () => Effect.Effect<void>
  restart: () => Effect.Effect<void>
}

interface UpdatePort {
  restart?: () => Effect.Effect<void, unknown>
  check: () => Effect.Effect<UpdateInfo, unknown>
  install: (
    onProgress: (event: UpdateProgress) => void,
  ) => Effect.Effect<void, unknown>
}

export function createUpdatesStore(
  port: UpdatePort = { check: checkAppUpdate, install: installAppUpdate },
) {
  let checked = false
  return createStore<UpdateState>()((set, get) => ({
    status: 'checking',
    info: null,
    progress: null,
    busy: false,
    error: null,
    check: () =>
      Effect.suspend(() => {
        if (checked) return Effect.void
        checked = true
        return port.check().pipe(
          Effect.tap((info) =>
            Effect.sync(() => {
              let status: Status = 'current'
              if (!info.supported) status = 'unsupported'
              else if (info.availableVersion) status = 'available'
              set({ info, status })
            }),
          ),
          Effect.asVoid,
          Effect.catchCause(() =>
            Effect.sync(() => {
              set({
                status: 'error',
                error:
                  'Não foi possível verificar as atualizações. A próxima verificação será ao abrir o app novamente.',
              })
            }),
          ),
        )
      }),
    install: () =>
      Effect.suspend(() => {
        if (
          get().busy ||
          get().progress?.status === 'restartRequired' ||
          !get().info?.availableVersion
        )
          return Effect.void
        set({
          busy: true,
          error: null,
          progress: { status: 'downloading', downloaded: 0, total: null },
        })
        // Installation must finish even if the UI unmounts after download starts.
        return port
          .install((progress) => set({ progress }))
          .pipe(
            Effect.uninterruptible,
            Effect.tap(() =>
              Effect.sync(() =>
                set((state) => ({
                  busy: false,
                  progress: null,
                  status: 'current',
                  info: state.info
                    ? { ...state.info, availableVersion: null }
                    : null,
                })),
              ),
            ),
            Effect.catchCause(() =>
              Effect.sync(() => {
                set({
                  busy: false,
                  progress:
                    get().progress?.status === 'restartRequired'
                      ? get().progress
                      : null,
                  error:
                    get().progress?.status === 'restartRequired'
                      ? 'Não foi possível iniciar o instalador. Reinicie o app para continuar.'
                      : 'Não foi possível instalar a atualização. Tente novamente.',
                })
              }),
            ),
          )
      }),
    restart: () =>
      (port.restart ?? restartAfterFailedUpdate)().pipe(
        Effect.catchCause(() =>
          Effect.sync(() =>
            set({ error: 'Feche e abra o EngineRoom para continuar.' }),
          ),
        ),
      ),
  }))
}

export const updatesStore = createUpdatesStore()
