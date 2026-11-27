const { test } = require('node:test');
const assert = require('node:assert/strict');
const { PRODUCTS, resolveProduct, validateAssets, versionFromCargoToml } = require('./product.cjs');

const asset = name => ({ name, size: 100, state: 'uploaded' });

test('the desktop product is selected by default', () => {
  assert.equal(resolveProduct({}).name, 'rune');
  assert.equal(resolveProduct({}, { ENGINEROOM_PRODUCT: 'rune' }).name, 'rune');
  assert.throws(() => resolveProduct({ product: 'other' }), /Produto de release desconhecido/);
});

test('root workspace version comes only from [package].version', () => {
  const source = [
    '[workspace]',
    'members = ["crates/engineroom-rune"]',
    '',
    '[workspace.package]',
    'version = "9.9.9"',
    '',
    '[package]',
    'name = "engineroom"',
    'version = "3.0.0"',
    'edition = "2024"',
  ].join('\n');
  assert.equal(versionFromCargoToml(source), '3.0.0');
  assert.equal(PRODUCTS.rune.readVersion(() => source), '3.0.0');
  assert.equal(versionFromCargoToml('[package]\nversion = "3.0.0-alpha.1"\n'), '3.0.0-alpha.1');
  assert.throws(() => versionFromCargoToml('[workspace]\nversion = "3.0.0"\n'), /estável/);
});

test('Rune publication accepts AppImage + NSIS only and never an updater feed', () => {
  const names = ['EngineRoom.AppImage', 'EngineRoom.exe'];
  validateAssets(names.map(asset), PRODUCTS.rune);
  assert.throws(() => validateAssets(['EngineRoom.AppImage'].map(asset), PRODUCTS.rune), /NSIS|\.exe/);
  assert.throws(() => validateAssets([...names, 'latest.json'].map(asset), PRODUCTS.rune), /updater/);
  assert.throws(() => validateAssets([...names, 'EngineRoom.AppImage.sig', 'latest.json'].map(asset), PRODUCTS.rune), /updater/);
});
