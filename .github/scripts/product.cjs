// Release version and installer validation for EngineRoom.
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '../..');
const STABLE = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const PRERELEASE = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)-[0-9A-Za-z.-]+$/;
const defaultRead = file => fs.readFileSync(path.join(ROOT, file), 'utf8');

function tomlSection(source, name) {
  const header = new RegExp(`^\\[${name}\\][ \\t]*\\r?\\n`, 'm').exec(source);
  if (!header) return '';
  const rest = source.slice(header.index + header[0].length);
  const next = rest.search(/^\[/m);
  return next === -1 ? rest : rest.slice(0, next);
}

function versionFromCargoToml(source) {
  const section = tomlSection(source, 'package');
  const match = /^version[ \t]*=[ \t]*"([^"\r\n]+)"/m.exec(section);
  if (!match || !(STABLE.test(match[1]) || PRERELEASE.test(match[1]))) {
    throw new Error('Cargo.toml do desktop precisa de [package].version estável (1.2.3).');
  }
  return match[1];
}

const PRODUCTS = {
  rune: {
    name: 'rune',
    manifestKey: '.',
    versionFile: 'Cargo.toml',
    changelog: 'CHANGELOG.md',
    readVersion(read = defaultRead) {
      return versionFromCargoToml(read('Cargo.toml'));
    },
    installers: ['.AppImage', '.exe'],
  },
};

function resolveProduct(context = {}, env = process.env) {
  const name = context.product || env.ENGINEROOM_PRODUCT || 'rune';
  if (!Object.hasOwn(PRODUCTS, name)) {
    throw new Error(`Produto de release desconhecido: ${name}. Use rune.`);
  }
  return PRODUCTS[name];
}

function validateAssets(assets, product = PRODUCTS.rune) {
  for (const extension of product.installers) {
    if (!assets.some(asset => asset.name.endsWith(extension) && asset.size > 0 && asset.state === 'uploaded')) {
      throw new Error(`Instalador ausente ou incompleto: ${extension}`);
    }
  }
  if (assets.some(asset => asset.name === 'latest.json')) {
    throw new Error('A release não publica feed de updater (latest.json).');
  }
}

module.exports = {
  PRODUCTS, STABLE, defaultRead, resolveProduct, validateAssets,
  versionFromCargoToml, tomlSection, PRERELEASE,
};
