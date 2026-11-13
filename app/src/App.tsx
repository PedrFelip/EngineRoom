import { invoke } from '@tauri-apps/api/core'
import { useEffect, useState } from 'react'
import HomePage from './components/home/HomePage'
import ReviewScreen from './components/review/ReviewScreen'
import UciIpcBenchmark from './components/UciIpcBenchmark'
import { runUi } from './lib/effect/ui-runtime'
import { updatesStore } from './lib/updates-store'
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

  useEffect(() => {
    if (benchmarkEnabled === false) void runUi(updatesStore.getState().check())
  }, [benchmarkEnabled])

  if (benchmarkEnabled === null) return null
  if (benchmarkEnabled) return <UciIpcBenchmark />

  const homeActive = view === 'home' || !config

  return (
    <main className='app-canvas min-h-full'>
      <div hidden={!homeActive}>
        <HomePage
          active={homeActive}
          onStart={(cfg) => {
            setConfig(cfg)
            setView('review')
          }}
        />
      </div>
      {!homeActive && config && (
        <ReviewScreen
          key={config.pgn + config.engine.id}
          config={config}
          onExit={() => setView('home')}
        />
      )}
    </main>
  )
}
