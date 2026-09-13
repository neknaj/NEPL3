import test from 'node:test';
import assert from 'node:assert/strict';
import katex from 'katex';
import { render } from '../../math/katex/render.mjs';

const limits = { inputBytes: 4096, outputBytes: 100000 };
const request = (tex, output = 'html') => ({ tex, displayMode: false, output });

test('real renderer returns unchecked generation output in either mode', () => {
  for (const output of ['html', 'htmlAndMathml']) {
    const result = render(katex, request(String.raw`\frac{1}{0}`, output), limits);
    assert.equal(result.kind, 'rendered-unchecked');
    assert.ok(result.html.includes('mfrac'));
    assert.equal(result.outputBytes, Buffer.byteLength(result.html));
    assert.equal(result.html.includes('<math'), output === 'htmlAndMathml');
  }
});
test('UTF-8 limits and malformed scalar input are explicit', () => {
  assert.equal(render(katex, request('é'), { ...limits, inputBytes: 1 }).reason, 'input-limit');
  assert.equal(render(katex, request('\ud800'), limits).kind, 'invalid-request');
  assert.equal(render(katex, request('x'), { ...limits, outputBytes: 0 }).reason, 'output-limit');
  assert.equal(render(katex, request('x'), { ...limits, inputBytes: Infinity }).kind, 'invalid-request');
});
test('real expansion exhaustion differs from a parse error with hostile text', () => {
  assert.deepEqual(render(katex, request(String.raw`\def\loop{\loop}\loop`), limits),
    { kind: 'stopped', reason: 'expansion-limit' });
  const error = render(katex, request(String.raw`\bad{<script>Too many expansions}`), limits);
  assert.deepEqual(error, { kind: 'render-error', reason: 'parse' });
});
test('macro mutations do not survive another generation', () => {
  assert.equal(render(katex, request(String.raw`\gdef\saved{1}\saved`), limits).kind, 'rendered-unchecked');
  assert.equal(render(katex, request(String.raw`\saved`), limits).kind, 'render-error');
});
test('unavailable or mismatched renderer and internal exceptions are not success', () => {
  assert.equal(render(null, request('x'), limits).kind, 'unavailable');
  assert.equal(render({ ...katex, version: 'other' }, request('x'), limits).kind, 'provider-violation');
  assert.equal(render({ ...katex, renderToString() { throw Error('internal'); } }, request('x'), limits).reason, 'exception');
});
test('supplementary scalars use four bytes at input and output boundaries', () => {
  // A controlled renderer isolates byte accounting from font/TeX support.
  const renderer = { ...katex, renderToString: () => '𠮷' };
  assert.equal(render(renderer, request('𠮷'), { inputBytes: 3, outputBytes: 4 }).reason, 'input-limit');
  assert.equal(render(renderer, request('𠮷'), { inputBytes: 4, outputBytes: 3 }).reason, 'output-limit');
  const result = render(renderer, request('𠮷'), { inputBytes: 4, outputBytes: 4 });
  assert.equal(result.kind, 'rendered-unchecked');
  assert.equal(result.inputBytes, 4);
  assert.equal(result.outputBytes, 4);
});
test('nonstring and malformed scalar renderer output is rejected', () => {
  for (const html of [null, {}, '\ud800', '\udc00']) {
    const renderer = { ...katex, renderToString: () => html };
    assert.deepEqual(render(renderer, request('x'), limits),
      { kind: 'provider-violation', reason: 'output' });
  }
});
