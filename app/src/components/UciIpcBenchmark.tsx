import { invoke } from '@tauri-apps/api/core'
import { useEffect, useState } from 'react'
import {
  percentReduction,
  summarizeUciIpcSamples,
  UCI_IPC_BENCHMARK_SAMPLES,
  UCI_IPC_BENCHMARK_WARMUPS,
  uciBenchmarkPairs,
} from '../lib/uci-ipc-benchmark'

type Report = {
  workload: { positions: number; commandsPerPosition: number }
  samples: number
  warmups: number
  baseline: ReturnType<typeof summarizeUciIpcSamples> & { invokes: number }
  batch: ReturnType<typeof summarizeUciIpcSamples> & { invokes: number }
  medianReductionPercent: number
  gatePassed: boolean
  userAgent: string
}

let started = false

async function measure(run: () => Promise<void>) {
  const samples: number[] = []
  for (let index = 0; index < UCI_IPC_BENCHMARK_WARMUPS; index++) await run()
  for (let index = 0; index < UCI_IPC_BENCHMARK_SAMPLES; index++) {
    const start = performance.now()
    await run()
    samples.push(performance.now() - start)
  }
  return summarizeUciIpcSamples(samples)
}

export default function UciIpcBenchmark() {
  const [status, setStatus] = useState('Preparando benchmark IPC UCI…')

  useEffect(() => {
    if (started) return
    started = true
    const pairs = uciBenchmarkPairs()
    const baseline = async () => {
      for (const [position, go] of pairs) {
        await invoke('benchmark_uci_send', { line: position })
        await invoke('benchmark_uci_send', { line: go })
      }
    }
    const batch = async () => {
      for (const lines of pairs)
        await invoke('benchmark_uci_send_batch', { lines })
    }

    void (async () => {
      setStatus('Medindo 300 invokes por amostra…')
      const baselineSummary = await measure(baseline)
      setStatus('Medindo 150 batches por amostra…')
      const batchSummary = await measure(batch)
      const medianReductionPercent = percentReduction(
        baselineSummary.medianMs,
        batchSummary.medianMs,
      )
      const report: Report = {
        workload: { positions: pairs.length, commandsPerPosition: 2 },
        samples: UCI_IPC_BENCHMARK_SAMPLES,
        warmups: UCI_IPC_BENCHMARK_WARMUPS,
        baseline: { ...baselineSummary, invokes: pairs.length * 2 },
        batch: { ...batchSummary, invokes: pairs.length },
        medianReductionPercent,
        gatePassed: medianReductionPercent >= 40,
        userAgent: navigator.userAgent,
      }
      setStatus(
        `Concluído: redução mediana ${medianReductionPercent.toFixed(1)}%.`,
      )
      await invoke('benchmark_uci_report', { report })
    })().catch((error: unknown) => {
      setStatus(
        `Falhou: ${error instanceof Error ? error.message : String(error)}`,
      )
    })
  }, [])

  return (
    <main className='app-canvas grid min-h-full place-items-center p-8'>
      <p className='text-center text-lg text-ink'>{status}</p>
    </main>
  )
}
