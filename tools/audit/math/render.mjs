// Test adapter: executes production TeX against pinned KaTeX. This is not the
// product's host, output validator, or cancellation/asset/identity protocol.
import katex from 'katex';
import { readFileSync } from 'node:fs';
import { render } from '../../math/katex/render.mjs';

const requests = JSON.parse(readFileSync(0, 'utf8'));
const results = requests.map(({ tex, displayMode }) => {
  const result = render(katex, { tex, displayMode, output: 'htmlAndMathml' },
    { inputBytes: 1000000, outputBytes: 10000000 });
  if (result.kind !== 'rendered-unchecked') throw new Error(JSON.stringify(result));
  return { version: result.version, html: result.html };
});
process.stdout.write(JSON.stringify(results));
