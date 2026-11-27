import assert from 'node:assert/strict'
import {
  chmodSync,
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'
import {
  assetFor,
  extractTar,
  extractZip,
  isExpectedVersion,
  markExecutable,
} from './fetch-stockfish.mjs'

const FIXTURES = fileURLToPath(new URL('./fixtures/', import.meta.url))
const FIXTURE_FILE = 'stockfish-linux-x86-64-universal'
const FIXTURE_CONTENT = 'fixture stockfish binary\n'
// Spaces, quotes, command substitution, backticks and separators must all be
// treated as literal path characters, never as shell syntax.
const SENSITIVE_DIR = "fixture dir's $(echo injected) `echo x` & ; (test)"

test('selects official universal packages for supported Rust targets', () => {
  const targets = {
    'x86_64-unknown-linux-gnu': 'stockfish-linux-x86-64-universal.tar.gz',
    'aarch64-unknown-linux-gnu': 'stockfish-linux-arm64-universal.tar.gz',
    'x86_64-pc-windows-msvc': 'stockfish-windows-x86-64-universal.zip',
    'aarch64-pc-windows-msvc': 'stockfish-windows-arm64-universal.zip',
    'x86_64-apple-darwin': 'stockfish-macos-universal.tar.gz',
    'aarch64-apple-darwin': 'stockfish-macos-universal.tar.gz',
  }
  for (const [target, asset] of Object.entries(targets)) {
    assert.equal(assetFor(target), asset)
  }
})

test('rejects targets without a compatible official package', () => {
  for (const target of [
    'i686-unknown-linux-gnu',
    'armv7-unknown-linux-gnueabihf',
    'x86_64-unknown-linux-musl',
    'x86_64-unknown-freebsd',
  ]) {
    assert.throws(() => assetFor(target))
  }
})

test('requires the exact released version from the UCI identity', () => {
  assert.equal(isExpectedVersion('id name Stockfish 19\r\nuciok\r\n'), true)
  for (const output of [
    'id name Stockfish 18\nuciok\n',
    'id name Stockfish 190\nuciok\n',
    'id name Stockfish 19-dev\nuciok\n',
    'Stockfish 19 by the Stockfish developers\n',
  ]) {
    assert.equal(isExpectedVersion(output), false)
  }
})

test('extracts archives whose paths contain shell-sensitive characters', async () => {
  const base = mkdtempSync(join(tmpdir(), 'stockfish-paths-'))
  try {
    const dir = join(base, SENSITIVE_DIR)
    mkdirSync(dir, { recursive: true })
    for (const [fixture, extract] of [
      ['stockfish-linux-x86-64-universal.tar.gz', extractTar],
      ['stockfish-linux-x86-64-universal.zip', extractZip],
    ]) {
      const archive = join(dir, fixture)
      copyFileSync(join(FIXTURES, fixture), archive)
      const outDir = join(dir, `${fixture}.out`)
      mkdirSync(outDir)
      await extract(archive, outDir)
      assert.equal(
        readFileSync(join(outDir, 'stockfish', FIXTURE_FILE), 'utf8'),
        FIXTURE_CONTENT,
      )
    }
  } finally {
    rmSync(base, { recursive: true, force: true })
  }
})

test('marks binaries executable through fs instead of a shell command', () => {
  if (process.platform === 'win32') return
  const dir = mkdtempSync(join(tmpdir(), 'stockfish-chmod-'))
  try {
    const binary = join(dir, "bin 'quoted' $(echo injected)")
    writeFileSync(binary, FIXTURE_CONTENT, { mode: 0o600 })
    chmodSync(binary, 0o600)
    markExecutable(binary)
    assert.equal(statSync(binary).mode & 0o111, 0o111)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})
