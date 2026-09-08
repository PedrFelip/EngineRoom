import { describe, expect, it } from 'vitest'
import {
  percentReduction,
  summarizeUciIpcSamples,
  uciBenchmarkPairs,
} from './uci-ipc-benchmark'

describe('benchmark IPC UCI', () => {
  it('cria pares position/go ordenados para cada posição', () => {
    const pairs = uciBenchmarkPairs(2)

    expect(pairs).toHaveLength(2)
    expect(pairs[0][0]).toMatch(/^position fen /)
    expect(pairs[0][1]).toBe('go depth 20')
  })

  it('calcula mediana, p95 e redução', () => {
    expect(summarizeUciIpcSamples([5, 1, 3, 2, 4])).toEqual({
      medianMs: 3,
      p95Ms: 5,
      minMs: 1,
      maxMs: 5,
    })
    expect(percentReduction(10, 6)).toBe(40)
  })
})
