import { Effect } from 'effect'
import { errorMessage, PersistenceError } from './effect/errors'
import { decode, ipc } from './effect/ipc'
import { StorageStatsSchema } from './effect/schemas'

export interface StorageStats {
  cacheBytes: number
  gamesBytes: number
  dbBytes: number
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`
}

/** Tamanho ocupado pelas tabelas de cache e partidas + arquivo do banco. */
export function getStorageStats() {
  return storageIpc('storage_stats').pipe(
    Effect.flatMap((data) => decode(StorageStatsSchema, data, 'storage_stats')),
  )
}

/** Esvazia a tabela de posições avaliadas (não toca no histórico). */
export function clearCache() {
  return storageIpc('cache_clear').pipe(Effect.asVoid)
}

function storageIpc(operation: string) {
  return ipc(
    operation,
    undefined,
    (cause) =>
      new PersistenceError({ operation, cause, message: errorMessage(cause) }),
  )
}
