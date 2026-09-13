// Test adapter: executes production TeX against pinned KaTeX. This is not the
// product's host, output validator, or cancellation/asset/identity protocol.
import { readFileSync } from 'node:fs';
import { render } from '../../math/katex/node/render.mjs';

const requests = JSON.parse(readFileSync(0, 'utf8'));
const results = [];
for (const { tex, displayMode } of requests) {
  const { result } = await render(new URL('./node_modules/katex/dist/katex.mjs', import.meta.url),
    { tex, displayMode, output: 'htmlAndMathml' },
    { inputBytes: 1000000, outputBytes: 10000000 },
    { timeoutMillis: 10000, diagnosticBytes: 10000 });
  if (result.kind !== 'rendered-unchecked') throw new Error(JSON.stringify(result));
  results.push({ version: result.version, html: result.html });
}
process.stdout.write(JSON.stringify(results));
