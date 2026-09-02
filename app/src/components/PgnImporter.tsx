import { Check, ClipboardPaste, FileUp, Upload } from 'lucide-react'
import { useCallback, useRef, useState } from 'react'
import { Badge } from './ui/badge'
import { Button } from './ui/button'

interface Props {
  value: string
  onChange: (pgn: string) => void
  /** Arquivos são validados imediatamente, sem aguardar o debounce da digitação. */
  onImport?: (pgn: string) => void
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

export default function PgnImporter({ value, onChange, onImport }: Props) {
  const [mode, setMode] = useState<Mode>('file')
  const [dragActive, setDragActive] = useState(false)
  const [fileName, setFileName] = useState<string | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const handleFile = useCallback(
    async (file: File) => {
      if (!file) return
      const text = await readFile(file)
      setFileName(file.name)
      const change = onImport ?? onChange
      change(text)
    },
    [onChange, onImport],
  )

  return (
    <div>
      <div className='mb-3 flex w-fit rounded-[calc(var(--control-radius)+2px)] border border-edge bg-panel-2/60 p-1'>
        {(['file', 'paste'] as Mode[]).map((m) => (
          <Button
            key={m}
            onClick={() => setMode(m)}
            size='sm'
            variant={mode === m ? 'default' : 'ghost'}
            className={`h-8 px-3 ${mode === m ? '' : 'text-ink-faint hover:text-ink'}`}
          >
            {m === 'file' ? (
              <>
                <FileUp size={14} strokeWidth={2} aria-hidden='true' />
                Arquivo PGN
              </>
            ) : (
              <>
                <ClipboardPaste size={14} strokeWidth={2} aria-hidden='true' />
                Colar PGN
              </>
            )}
          </Button>
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
            className={`flex w-full cursor-pointer flex-col items-center justify-center rounded-[var(--control-radius)] border border-dashed px-6 py-8 text-center outline-none transition focus-visible:ring-2 focus-visible:ring-brand focus-visible:ring-offset-2 focus-visible:ring-offset-bg ${
              dragActive
                ? 'border-brand bg-brand/10'
                : 'border-edge bg-panel-2/40 hover:border-ink-faint hover:bg-panel-2/70'
            }`}
          >
            <Upload
              size={36}
              strokeWidth={1.6}
              className='mb-3 text-ink-faint'
              aria-hidden='true'
            />

            <p className='text-sm text-ink'>
              Arraste um arquivo{' '}
              <span className='font-semibold text-brand'>.pgn</span>
            </p>
            <p className='mt-0.5 text-xs text-ink-faint'>
              ou clique para procurar
            </p>

            {fileName && (
              <Badge variant='outline' className='mt-3'>
                <Check size={12} strokeWidth={2.5} aria-hidden='true' />
                {fileName}
              </Badge>
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
            className='h-44 w-full resize-none rounded-[var(--control-radius)] border border-edge bg-panel-2/40 p-3 font-mono text-[13px] leading-relaxed text-ink outline-none transition-[border-color,box-shadow] placeholder:text-ink-faint focus:border-brand focus:ring-2 focus:ring-brand/25'
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
