import assert from 'node:assert/strict'
import { test } from 'node:test'
import { assetFor, isExpectedVersion } from './fetch-stockfish.mjs'

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
