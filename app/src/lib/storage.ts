import { invoke } from '@tauri-apps/api/core'

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
export function getStorageStats(): Promise<StorageStats> {
  return invoke<StorageStats>('storage_stats')
}

/** Esvazia a tabela de posições avaliadas (não toca no histórico). */
export function clearCache(): Promise<void> {
  return invoke('cache_clear')
}
