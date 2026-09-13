import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Worker } from 'node:worker_threads';
import { render } from '../../math/katex/node/render.mjs';
const katex = new URL('./node_modules/katex/dist/katex.mjs', import.meta.url);
const fixture = new URL('./fixtures/renderer.mjs', import.meta.url);
const request = tex => ({ tex, displayMode: false, output: 'html' });
const limits = { inputBytes: 10000, outputBytes: 100000 };
const options = { timeoutMillis: 10000, diagnosticBytes: 1000 };

test('real pinned KaTeX runs in disposable realm', async () => {
  const output = await render(katex, request(String.raw`\frac{1}{0}`), limits, options);
  assert.equal(output.result.kind, 'rendered-unchecked');
  assert.ok(output.result.html.includes('mfrac'));
  assert.deepEqual(output.diagnostics, []);
});
test('console diagnostics are bounded structured text', async () => {
  const output = await render(fixture, request('warning'), limits, options);
  assert.deepEqual(output.diagnostics, [{ level: 'warn', text: 'renderer warning' }]);
  const exhausted = await render(fixture, request('warning'), limits, { ...options, diagnosticBytes: 0 });
  assert.deepEqual(exhausted.result, { kind: 'stopped', reason: 'diagnostic-limit' });
});
test('missing capability, unsolicited stdio and premature exit stay distinct', async () => {
  assert.equal((await render(new URL('./missing.mjs', import.meta.url), request('x'), limits, options)).result.kind, 'unavailable');
  for (const tex of ['stdio', 'exit']) {
    assert.equal((await render(fixture, request(tex), limits, options)).result.kind, 'provider-violation');
  }
});
test('deadline terminates an entered synchronous loop before resolving', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'nepl3-katex-worker-'));
  try {
    const path = join(dir, 'entered');
    const output = await render(fixture, request(`busy:${path}`), limits, { ...options, timeoutMillis: 1500 });
    assert.deepEqual(output.result, { kind: 'stopped', reason: 'deadline' });
    assert.equal(readFileSync(path, 'utf8'), 'entered synchronous render');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
test('cancelled and invalid requests never become rendering successes', async () => {
  const controller = new AbortController();
  controller.abort();
  assert.deepEqual((await render(katex, request('x'), limits, { ...options, signal: controller.signal })).result,
    { kind: 'stopped', reason: 'cancelled' });
  const running = new AbortController();
  const promise = render(katex, request('x'), limits, { ...options, signal: running.signal });
  running.abort();
  assert.deepEqual((await promise).result, { kind: 'stopped', reason: 'cancelled' });
  assert.equal((await render(katex, request('x'), limits, { ...options, timeoutMillis: Infinity })).result.kind, 'invalid-request');
  assert.equal((await render(katex, request('\ud800'), limits, options)).result.kind, 'invalid-request');
});
test('cancellation during actual worker termination overrides received output', async () => {
  const controller = new AbortController();
  const terminate = Worker.prototype.terminate;
  // Force the precise race after message receipt, retaining actual termination.
  Worker.prototype.terminate = function () {
    controller.abort();
    return terminate.call(this);
  };
  try {
    const output = await render(katex, request('x'), limits, { ...options, signal: controller.signal });
    assert.deepEqual(output.result, { kind: 'stopped', reason: 'cancelled' });
  } finally { Worker.prototype.terminate = terminate; }
});
