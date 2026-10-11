import test from 'node:test';
import assert from 'node:assert/strict';
import { access, cp, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { withExecution } from '../../math/katex/node/execution.mjs';
import { renderParsed } from '../../math/katex/node/render.mjs';
const root = new URL('./node_modules/', import.meta.url);
const limits = { inputBytes: 10000, outputBytes: 100000, nodes: 10000, depth: 256, transportBytes: 1000000 };
test('verified executable closure works from owned snapshot and is cleaned', async () => {
  let retained;
  const output = await withExecution(root, 10000000, async ({ renderer, parser }) => {
    retained = renderer;
    return renderParsed(renderer, parser, { tex: String.raw`\sqrt{x}`, displayMode: false, output: 'html' },
      limits, { timeoutMillis: 10000, diagnosticBytes: 1000 });
  });
  assert.equal(output.result.kind, 'parsed-unchecked');
  await assert.rejects(access(retained));
});
test('byte admission precedes disk access or callback', async () => {
  assert.deepEqual(await withExecution(new URL('./missing/', import.meta.url), 0, () => assert.fail()),
    { kind: 'stopped', reason: 'execution-limit' });
});
test('tampered executable never runs', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'nepl3-katex-tamper-'));
  try {
    await cp(root, directory, { recursive: true });
    const file = join(directory, 'katex/dist/katex.mjs');
    const original = await readFile(file);
    original[0] ^= 1;
    await writeFile(file, original);
    const result = await withExecution(pathToFileURL(`${directory}/`), 10000000, () => assert.fail());
    assert.equal(result.kind, 'provider-violation');
    assert.equal(result.reason, 'execution-identity');
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('reviewed CSS and structural inventory cover representative pinned output', async () => {
  const pins = JSON.parse(await readFile(new URL('../../math/katex/execution.json', import.meta.url)));
  const classes = new Set(pins.classes);
  await withExecution(root, 10000000, async ({ renderer, parser }) => {
    for (const tex of ['x', String.raw`\frac{1}{2}`, String.raw`\sqrt{x}`, String.raw`\sum_{i=1}^{n} i`,
      String.raw`\begin{pmatrix}a&b\\c&d\end{pmatrix}`, String.raw`\int_0^1 x\,dx`,
      String.raw`\binom{n}{k}`, String.raw`\hat{x}`, String.raw`a\to b`, String.raw`\mathbb{R}`]) {
      const output = await renderParsed(renderer, parser, { tex, displayMode: true, output: 'html' }, limits,
        { timeoutMillis: 10000, diagnosticBytes: 1000 });
      assert.equal(output.result.kind, 'parsed-unchecked', tex);
      for (const node of output.result.nodes) {
        if (node.kind === 'span') for (const name of node.classes.split(' ').filter(Boolean)) {
          assert.ok(classes.has(name), `${tex}: missing reviewed class ${name}`);
        }
      }
    }
  });
});
