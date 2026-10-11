// Inventory of the fixed, reviewed generation-side executable closure. This is
// not a general JavaScript dependency resolver or a CSS sanitizer.
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
const root = new URL('../audit/math/node_modules/', import.meta.url);
const output = new URL('../math/katex/execution.json', import.meta.url);
const lock = JSON.parse(readFileSync(new URL('../audit/math/package-lock.json', import.meta.url)));
const packages = [['katex', '0.18.7'], ['parse5', '8.0.0'], ['entities', '6.0.1']];
const paths = ['katex/dist/katex.mjs', 'parse5/package.json', 'entities/package.json'];
function collect(path) {
  for (const entry of readdirSync(new URL(path, root), { withFileTypes: true })) {
    const next = `${path}${entry.name}`;
    if (entry.isDirectory()) collect(`${next}/`);
    else if (entry.isFile() && /\.(js|json)$/.test(entry.name)) paths.push(next);
    else if (!entry.isFile()) throw Error('unexpected executable entry');
  }
}
collect('parse5/dist/');
collect('entities/dist/esm/');
const identities = packages.map(([name, version]) => {
  const info = JSON.parse(readFileSync(new URL(`${name}/package.json`, root)));
  const pin = lock.packages[`node_modules/${name}`];
  if (info.name !== name || info.version !== version || pin.version !== version) throw Error('package version changed');
  return { name, version, integrity: pin.integrity };
});
const files = paths.sort().map(path => {
  const bytes = readFileSync(new URL(path, root));
  return { path, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
});
const css = readFileSync(new URL('katex/dist/katex.min.css', root), 'utf8');
// CSS-derived subset: selectors of this exact reviewed fixed stylesheet.
const stylesheetClasses = [...new Set([...css.matchAll(/([^{}]+)\{/g)]
  .flatMap(rule => [...rule[1].matchAll(/\.([A-Za-z_][A-Za-z0-9_-]*)/g)].map(match => match[1])))].sort();
if (!stylesheetClasses.includes('katex') || !stylesheetClasses.includes('mfrac')) throw Error('CSS inventory incomplete');
// Fixed atom classes in KaTeX src/buildHTML.ts and tight-layout marker from
// src/buildCommon.ts; text wrapper from src/functions/text.ts. They carry structural meaning even without CSS rules.
const structuralClasses = ['mbin', 'mclose', 'minner', 'mop', 'mopen', 'mord', 'mpunct', 'mrel', 'mtight', 'text',
  // domTree.ts script marker names from the fixed unicodeScripts.ts inventory.
  'latin_fallback', 'cyrillic_fallback', 'armenian_fallback', 'brahmic_fallback',
  'georgian_fallback', 'cjk_fallback', 'hangul_fallback'];
const classes = [...new Set([...stylesheetClasses, ...structuralClasses])].sort();
const value = { format: 'nepl3.katex-execution/1', packages: identities,
  stylesheetSha256: createHash('sha256').update(css).digest('hex'), stylesheetClasses, structuralClasses, classes, files };
const text = JSON.stringify(value, null, 2) + '\n';
if (process.argv.length === 3 && process.argv[2] === '--write') writeFileSync(output, text);
else if (process.argv.length === 2) {
  if (readFileSync(output, 'utf8') !== text) throw Error('execution inventory changed; review package before --write');
} else throw Error('usage: node tools/generate/katex-host.mjs [--write]');
