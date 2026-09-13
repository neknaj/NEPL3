import test from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, sep } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { loadAssets } from '../../math/katex/node/assets.mjs';
import pins from '../../math/katex/assets.json' with { type: 'json' };
const root = new URL('./node_modules/katex/', import.meta.url);
const total = pins.files.reduce((sum, file) => sum + file.bytes, 0);

test('fixed CSS has every relative font, original bytes and license', async () => {
  const result = await loadAssets(root, total);
  assert.equal(result.kind, 'loaded');
  assert.equal(result.totalBytes, total);
  const files = new Map(result.files.map(file => [file.path, file]));
  const css = files.get('katex.min.css').bytes.toString('utf8');
  // This is an inventory assertion for the pinned CSS, not a CSS parser.
  const references = [...css.matchAll(/url\(([^)]+)\)/g)].map(match => match[1]);
  assert.equal(references.length, 60);
  assert.equal(files.size, 62);
  for (const path of references) assert.ok(files.has(path), path);
  assert.ok(files.get('LICENSE').bytes.includes(Buffer.from('Khan Academy')));
  for (const file of result.files) {
    assert.equal(createHash('sha256').update(file.bytes).digest('hex'), file.sha256);
    assert.match(file.mime, /^(text\/(css|plain)|font\/(ttf|woff|woff2))$/);
  }
});
test('asset byte limit is checked before filesystem reads', async () => {
  const missing = new URL('./nonexistent/', root);
  assert.deepEqual(await loadAssets(missing, total - 1), { kind: 'stopped', reason: 'asset-limit' });
  assert.equal((await loadAssets(root, Infinity)).kind, 'invalid-request');
});
test('changed CSS, font and missing license never yield partial success', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'nepl3-katex-assets-'));
  const target = pathToFileURL(dir + sep);
  try {
    cpSync(new URL('dist', root), join(dir, 'dist'), { recursive: true });
    cpSync(new URL('LICENSE', root), join(dir, 'LICENSE'));
    for (const source of ['dist/katex.min.css', 'dist/fonts/KaTeX_Main-Regular.woff2']) {
      const path = new URL(source, target);
      const bytes = readFileSync(path);
      const changed = Buffer.from(bytes);
      changed[0] ^= 1;
      writeFileSync(path, changed);
      assert.equal((await loadAssets(target, total)).kind, 'provider-violation');
      writeFileSync(path, Buffer.concat([bytes, Buffer.from('extra')]));
      assert.equal((await loadAssets(target, total)).reason, 'asset-size');
      writeFileSync(path, bytes);
    }
    rmSync(new URL('LICENSE', target));
    const missing = await loadAssets(target, total);
    assert.equal(missing.kind, 'unavailable');
    assert.equal(missing.files, undefined);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
