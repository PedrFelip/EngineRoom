import { describe, expect, it } from 'vitest'
import { formatBytes } from './storage'

describe('formatBytes', () => {
  it('formata zero bytes', () => {
    expect(formatBytes(0)).toBe('0 B')
  })

  it('formata bytes puros abaixo de 1 KB', () => {
    expect(formatBytes(512)).toBe('512 B')
  })

  it('formata KB com uma casa decimal', () => {
    expect(formatBytes(1024)).toBe('1.0 KB')
    expect(formatBytes(1536)).toBe('1.5 KB')
    expect(formatBytes(50 * 1024)).toBe('50.0 KB')
  })

  it('formata MB com duas casas decimais', () => {
    expect(formatBytes(1024 * 1024)).toBe('1.00 MB')
    expect(formatBytes(4.5 * 1024 * 1024)).toBe('4.50 MB')
    expect(formatBytes(250 * 1024 * 1024)).toBe('250.00 MB')
  })
})
