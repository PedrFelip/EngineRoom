import { useCallback, useRef, useState } from 'react'

interface Props {
  value: string
  onChange: (pgn: string) => void
}

type Mode = 'file' | 'paste'

function readFile(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result ?? ''))
    reader.onerror = () => reject(reader.error)
    reader.readAsText(file)
  })
}

export default function PgnImporter({ value, onChange }: Props) {
  const [mode, setMode] = useState<Mode>('file')
  const [dragActive, setDragActive] = useState(false)
  const [fileName, setFileName] = useState<string | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const handleFile = useCallback(
    async (file: File) => {
      if (!file) return
      const text = await readFile(file)
      setFileName(file.name)
      onChange(text)
    },
    [onChange],
  )

  return (
    <div>
      <div className='mb-3 mx-auto flex w-fit rounded-lg border border-edge bg-panel-2/60 p-1'>
        {(['file', 'paste'] as Mode[]).map((m) => (
          <button
            key={m}
            type='button'
            onClick={() => setMode(m)}
            className={`rounded-md px-4 py-1.5 text-sm font-medium transition ${
              mode === m
                ? 'bg-brand text-bg shadow'
                : 'text-ink-dim hover:text-ink'
            }`}
          >
            {m === 'file' ? 'Arquivo PGN' : 'Colar PGN'}
          </button>
        ))}
      </div>

      {mode === 'file' ? (
        <div className='relative'>
          <button
            type='button'
            onDragOver={(e) => {
              e.preventDefault()
              setDragActive(true)
            }}
            onDragLeave={() => setDragActive(false)}
            onDrop={(e) => {
              e.preventDefault()
              setDragActive(false)
              const file = e.dataTransfer.files?.[0]
              if (file) void handleFile(file)
            }}
            onClick={() => inputRef.current?.click()}
            className={`flex w-full cursor-pointer flex-col items-center justify-center rounded-xl border-2 border-dashed px-6 py-10 text-center transition ${
              dragActive
                ? 'border-brand bg-brand/10'
                : 'border-edge bg-panel-2/40 hover:border-ink-faint hover:bg-panel-2/70'
            }`}
          >
            <svg
              width='36'
              height='36'
              viewBox='0 0 24 24'
              fill='none'
              stroke='currentColor'
              strokeWidth='1.6'
              strokeLinecap='round'
              strokeLinejoin='round'
              className='mb-3 text-ink-faint'
              aria-hidden='true'
            >
              <path d='M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4' />
              <polyline points='17 8 12 3 7 8' />
              <line x1='12' y1='3' x2='12' y2='15' />
            </svg>

            <p className='text-sm text-ink'>
              Arraste um arquivo{' '}
              <span className='font-semibold text-brand'>.pgn</span>
            </p>
            <p className='mt-0.5 text-xs text-ink-faint'>
              ou clique para procurar
            </p>

            {fileName && (
              <span className='mt-3 inline-flex items-center gap-1.5 rounded-md bg-panel-3 px-2.5 py-1 text-xs text-ink-dim'>
                <svg
                  width='12'
                  height='12'
                  viewBox='0 0 24 24'
                  fill='none'
                  stroke='currentColor'
                  strokeWidth='2.5'
                  strokeLinecap='round'
                  strokeLinejoin='round'
                  aria-hidden='true'
                >
                  <polyline points='20 6 9 17 4 12' />
                </svg>
                {fileName}
              </span>
            )}
          </button>

          <input
            ref={inputRef}
            type='file'
            accept='.pgn,.txt,text/plain'
            className='hidden'
            onChange={(e) => {
              const file = e.target.files?.[0]
              if (file) void handleFile(file)
              e.currentTarget.value = ''
            }}
          />
        </div>
      ) : (
        <div className='relative'>
          <textarea
            value={value}
            onChange={(e) => onChange(e.target.value)}
            spellCheck={false}
            placeholder={
              '[Event "Partida amistosa"]\n[White "Magnus Carlsen"]\n[Black "Hikaru Nakamura"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 *'
            }
            className='h-44 w-full resize-none rounded-xl border border-edge bg-panel-2/40 p-4 font-mono text-[13px] leading-relaxed text-ink outline-none transition placeholder:text-ink-faint focus:border-brand focus:bg-panel-2/70'
          />
          {value && (
            <span className='absolute bottom-2.5 right-3 rounded bg-panel-3/80 px-1.5 py-0.5 font-mono text-[10px] text-ink-faint'>
              {value.length} chars
            </span>
          )}
        </div>
      )}
    </div>
  )
}
