import { invoke } from '@tauri-apps/api/core'
import { useEffect, useState } from 'react'
import HomePage from './components/HomePage'
import ReviewScreen from './components/ReviewScreen'
import UciIpcBenchmark from './components/UciIpcBenchmark'
import type { ReviewConfig } from './types'

type View = 'home' | 'review'

export default function App() {
  const [benchmarkEnabled, setBenchmarkEnabled] = useState<boolean | null>(null)
  const [view, setView] = useState<View>('home')
  const [config, setConfig] = useState<ReviewConfig | null>(null)

  useEffect(() => {
    void invoke<boolean>('benchmark_uci_enabled')
      .then(setBenchmarkEnabled)
      .catch(() => setBenchmarkEnabled(false))
  }, [])

  if (benchmarkEnabled === null) return null
  if (benchmarkEnabled) return <UciIpcBenchmark />

  if (view === 'home' || !config) {
    return (
      <main className='app-canvas min-h-full'>
        <HomePage
          onStart={(cfg) => {
            setConfig(cfg)
            setView('review')
          }}
        />
      </main>
    )
  }

  return (
    <main className='app-canvas min-h-full'>
      <ReviewScreen
        key={config.pgn + config.engine.id}
        config={config}
        onExit={() => setView('home')}
      />
    </main>
  )
}
