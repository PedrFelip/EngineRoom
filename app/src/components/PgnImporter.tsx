import { Check, ClipboardPaste, FileUp, Upload } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Badge } from './ui/badge'
import { SegmentedControl } from './ui/segmented-control'

interface Props {
  value: string
  active?: boolean
  valid?: boolean
  onReadingChange?: (reading: boolean) => void
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

export default function PgnImporter({
  value,
  active = true,
  valid = false,
  onReadingChange,
  onChange,
  onImport,
}: Props) {
  const [mode, setMode] = useState<Mode>(value.trim() ? 'paste' : 'file')
  const [dragActive, setDragActive] = useState(false)
  const [fileName, setFileName] = useState<string | null>(null)
  const [reading, setReading] = useState(false)
  const [readError, setReadError] = useState<string | null>(null)
  const readRequest = useRef(0)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (!active) {
      readRequest.current++
      setReading(false)
      onReadingChange?.(false)
    }
    return () => {
      readRequest.current++
      onReadingChange?.(false)
    }
  }, [active, onReadingChange])

  const handleFile = useCallback(
    async (file: File) => {
      const request = ++readRequest.current
      setReading(true)
      onReadingChange?.(true)
      setFileName(null)
      setReadError(null)
      try {
        const text = await readFile(file)
        if (request !== readRequest.current) return
        if (!text.trim()) {
          setReadError('O arquivo está vazio. Escolha um PGN com uma partida.')
          return
        }
        setFileName(file.name)
        const change = onImport ?? onChange
        change(text)
      } catch {
        if (request === readRequest.current) {
          setReadError(
            'Não foi possível ler o arquivo. Tente selecioná-lo novamente.',
          )
        }
      } finally {
        if (request === readRequest.current) {
          setReading(false)
          onReadingChange?.(false)
        }
      }
    },
    [onChange, onImport, onReadingChange],
  )
  let importStatus = ''
  if (reading) importStatus = 'Lendo arquivo PGN…'
  else if (fileName && valid) importStatus = `Arquivo carregado: ${fileName}`

  return (
    <div>
      <SegmentedControl
        className='mb-3 max-w-sm'
        label='Forma de importar a partida'
        value={mode}
        options={[
          { id: 'file', label: 'Arquivo PGN', icon: FileUp },
          { id: 'paste', label: 'Colar PGN', icon: ClipboardPaste },
        ]}
        onChange={(nextMode) => {
          setMode(nextMode)
          setReadError(null)
          if (nextMode === 'paste') {
            readRequest.current++
            setReading(false)
            onReadingChange?.(false)
          }
        }}
      />

      <div>
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
              className={`home-dropzone flex w-full cursor-pointer flex-col items-center justify-center rounded-[var(--control-radius)] border border-dashed px-4 py-6 text-center outline-none transition focus-visible:ring-2 focus-visible:ring-brand focus-visible:ring-offset-2 focus-visible:ring-offset-bg sm:px-6 sm:py-6 ${
                dragActive
                  ? 'border-brand bg-brand/10'
                  : 'border-edge bg-panel-2/40 hover:border-ink-faint hover:bg-panel-2/70'
              }`}
            >
              <Upload
                size={28}
                strokeWidth={1.6}
                className='mb-3 text-brand'
                aria-hidden='true'
              />

              <p className='text-sm text-ink'>
                Arraste um arquivo{' '}
                <span className='font-semibold text-brand'>.pgn</span>
              </p>
              <p className='mt-0.5 text-xs text-ink-faint'>
                {reading ? 'Lendo arquivo…' : 'ou clique para procurar'}
              </p>

              {fileName && valid && (
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
              onChange={(e) => {
                setFileName(null)
                setReadError(null)
                onChange(e.target.value)
              }}
              aria-label='Notação PGN da partida'
              spellCheck={false}
              placeholder={
                '[Event "Partida amistosa"]\n[White "Magnus Carlsen"]\n[Black "Hikaru Nakamura"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 *'
              }
              className='h-36 w-full resize-y rounded-[var(--control-radius)] border border-edge bg-panel-2/40 p-3 font-mono text-[13px] leading-relaxed text-ink outline-none transition-[border-color,box-shadow] placeholder:text-ink-faint focus:border-brand focus:ring-2 focus:ring-brand/25'
            />
            {value && (
              <span className='absolute bottom-2.5 right-3 rounded bg-panel-3/80 px-1.5 py-0.5 font-mono text-[10px] text-ink-faint'>
                {value.length} chars
              </span>
            )}
          </div>
        )}
      </div>
      <p className='sr-only' role='status'>
        {importStatus}
      </p>
      {readError && (
        <p role='alert' className='mt-3 text-sm text-blunder'>
          {readError}
        </p>
      )}
    </div>
  )
}
