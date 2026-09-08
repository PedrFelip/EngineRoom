export const UCI_IPC_BENCHMARK_POSITIONS = 150
export const UCI_IPC_BENCHMARK_WARMUPS = 5
export const UCI_IPC_BENCHMARK_SAMPLES = 31

export type UciIpcBenchmarkSummary = {
  medianMs: number
  p95Ms: number
  minMs: number
  maxMs: number
}

export function uciBenchmarkPairs(count = UCI_IPC_BENCHMARK_POSITIONS) {
  return Array.from({ length: count }, (_, index) => [
    `position fen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - ${index} 1`,
    'go depth 20',
  ]) as [string, string][]
}

export function summarizeUciIpcSamples(
  samples: readonly number[],
): UciIpcBenchmarkSummary {
  if (samples.length === 0) throw new Error('Benchmark sem amostras.')
  const sorted = [...samples].sort((a, b) => a - b)
  const quantile = (ratio: number) =>
    sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * ratio) - 1)]
  return {
    medianMs: quantile(0.5),
    p95Ms: quantile(0.95),
    minMs: sorted[0],
    maxMs: sorted[sorted.length - 1],
  }
}

export function percentReduction(baselineMs: number, candidateMs: number) {
  if (baselineMs <= 0) return 0
  return ((baselineMs - candidateMs) / baselineMs) * 100
}
