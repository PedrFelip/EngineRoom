import { useState } from 'react'
import HomePage from './components/HomePage'
import ReviewScreen from './components/ReviewScreen'
import type { ReviewConfig } from './types'

type View = 'home' | 'review'

export default function App() {
  const [view, setView] = useState<View>('home')
  const [config, setConfig] = useState<ReviewConfig | null>(null)

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
