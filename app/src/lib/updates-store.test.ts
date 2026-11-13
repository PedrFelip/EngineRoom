import { Deferred, Effect, Fiber } from 'effect'
import { describe, expect, it, vi } from 'vitest'
import { runUi } from './effect/ui-runtime'
import { UpdateError, type UpdateInfo } from './updates'
import { createUpdatesStore } from './updates-store'

const info: UpdateInfo = {
  currentVersion: '2.1.3',
  availableVersion: '2.2.0',
  notes: 'Novidades',
  supported: true,
}

describe('atualizações do app', () => {
  it('mostra o diagnóstico devolvido pelo atualizador', async () => {
    const store = createUpdatesStore({
      check: () => Effect.succeed(info),
      install: () =>
        Effect.fail(new UpdateError({ message: 'Arquivo não encontrado.' })),
    })
    await runUi(store.getState().check())
    await runUi(store.getState().install())
    expect(store.getState().error).toBe('Arquivo não encontrado.')
    expect(store.getState().errorKind).toBe('install')
  })

  it('permite dispensar o aviso de erro sem perder a atualização disponível', async () => {
    const store = createUpdatesStore({
      check: () => Effect.succeed(info),
      install: () =>
        Effect.fail(new UpdateError({ message: 'Arquivo não encontrado.' })),
    })
    await runUi(store.getState().check())
    await runUi(store.getState().install())
    store.getState().dismissError()
    expect(store.getState().error).toBeNull()
    expect(store.getState().errorKind).toBeNull()
    expect(store.getState().info?.availableVersion).toBe('2.2.0')
  })
  it('consulta somente uma vez, inclusive após uma falha', async () => {
    const check = vi.fn(() => Effect.fail('offline'))
    const store = createUpdatesStore({ check, install: () => Effect.void })
    await runUi(store.getState().check())
    await runUi(store.getState().check())
    expect(check).toHaveBeenCalledTimes(1)
    expect(store.getState().status).toBe('error')
    expect(store.getState().errorKind).toBe('check')
    expect(store.getState().busy).toBe(false)
  })

  it('informa versão atual e não instala sem uma atualização disponível', async () => {
    const install = vi.fn(() => Effect.void)
    const store = createUpdatesStore({
      check: () => Effect.succeed({ ...info, availableVersion: null }),
      install,
    })
    await runUi(store.getState().check())
    await runUi(store.getState().install())
    expect(store.getState().status).toBe('current')
    expect(store.getState().info?.currentVersion).toBe('2.1.3')
    expect(install).not.toHaveBeenCalled()
  })

  it('bloqueia instalações concorrentes e permite tentar novamente após erro', async () => {
    const gate = await runUi(Deferred.make<void, string>())
    const install = vi.fn((onProgress) => {
      onProgress({ status: 'downloading', downloaded: 50, total: 100 })
      return Deferred.await(gate)
    })
    const store = createUpdatesStore({
      check: () => Effect.succeed(info),
      install,
    })
    await runUi(store.getState().check())
    const fiber = Effect.runFork(store.getState().install())
    await vi.waitFor(() =>
      expect(store.getState().progress).toEqual({
        status: 'downloading',
        downloaded: 50,
        total: 100,
      }),
    )
    await runUi(store.getState().install())
    expect(install).toHaveBeenCalledTimes(1)
    await runUi(Deferred.fail(gate, 'signature mismatch'))
    await runUi(Fiber.join(fiber))
    expect(store.getState().busy).toBe(false)
    expect(store.getState().error).toContain('Não foi possível instalar')
    await runUi(store.getState().install())
    expect(install).toHaveBeenCalledTimes(2)
    expect(store.getState().info?.availableVersion).toBe('2.2.0')
  })
  it('preserva o pedido de reinício se o Windows recusar o instalador após a limpeza', async () => {
    const install = vi.fn((onProgress) => {
      onProgress({ status: 'restartRequired' })
      return Effect.fail('installer refused')
    })
    const store = createUpdatesStore({
      check: () => Effect.succeed(info),
      install,
    })
    await runUi(store.getState().check())
    await runUi(store.getState().install())
    expect(store.getState().progress).toEqual({ status: 'restartRequired' })
    expect(store.getState().error).toContain('Reinicie o app')
    await runUi(store.getState().install())
    expect(install).toHaveBeenCalledTimes(1)
  })
  it('remove a atualização disponível ao concluir a instalação', async () => {
    const install = vi.fn(() => Effect.void)
    const store = createUpdatesStore({
      check: () => Effect.succeed(info),
      install,
    })
    await runUi(store.getState().check())
    await runUi(store.getState().install())
    expect(store.getState().busy).toBe(false)
    expect(store.getState().progress).toBeNull()
    expect(store.getState().status).toBe('current')
    expect(store.getState().info?.availableVersion).toBeNull()
    await runUi(store.getState().install())
    expect(install).toHaveBeenCalledTimes(1)
  })
})
