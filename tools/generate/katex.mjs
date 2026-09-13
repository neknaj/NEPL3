// Fixed-package asset inventory. Run after npm ci --prefix tools/audit/math.
// This inventories the reviewed distribution CSS; it is not a CSS sanitizer.
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { KATEX_VERSION } from '../math/katex/render.mjs';
const root = new URL('../audit/math/node_modules/katex/', import.meta.url);
const output = new URL('../math/katex/assets.json', import.meta.url);
const packageInfo = JSON.parse(readFileSync(new URL('package.json', root), 'utf8'));
if (packageInfo.name !== 'katex' || packageInfo.version !== KATEX_VERSION) throw Error('unexpected KaTeX package');
const lock = JSON.parse(readFileSync(new URL('../audit/math/package-lock.json', import.meta.url), 'utf8'));
const css = readFileSync(new URL('dist/katex.min.css', root), 'utf8');
const fonts = [...css.matchAll(/url\(([^)]+)\)/g)].map(match => match[1]);
if (!fonts.length || fonts.some(path => !/^fonts\/KaTeX_[A-Za-z0-9-]+\.(woff2?|ttf)$/.test(path))) throw Error('review CSS font references');
const entries = [
  ['dist/katex.min.css', 'katex.min.css', 'text/css'],
  ['LICENSE', 'LICENSE', 'text/plain'],
  ...[...new Set(fonts)].sort().map(path => [`dist/${path}`, path,
    path.endsWith('.woff2') ? 'font/woff2' : path.endsWith('.woff') ? 'font/woff' : 'font/ttf']),
].map(([source, path, mime]) => {
  const bytes = readFileSync(new URL(source, root));
  return { source, path, mime, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
});
const header = {
  version: KATEX_VERSION,
  packageIntegrity: lock.packages['node_modules/katex'].integrity,
};
const text = JSON.stringify(header, null, 2).slice(0, -2) + ',\n  "files": [\n' +
  entries.map(entry => `    ${JSON.stringify(entry)}`).join(',\n') + '\n  ]\n}\n';
if (process.argv.length === 3 && process.argv[2] === '--write') writeFileSync(output, text, 'utf8');
else if (process.argv.length === 2) {
  if (readFileSync(output, 'utf8') !== text) throw Error('KaTeX inventory changed; review the fixed package before --write');
} else throw Error('usage: node tools/generate/katex.mjs [--write]');
