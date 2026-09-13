// Test adapter: executes production TeX against pinned KaTeX. This is not the
// product's host, output validator, or cancellation/asset/identity protocol.
import { readFileSync } from 'node:fs';
import { render } from '../../math/katex/node/render.mjs';
import { parse } from '../../math/katex/parse.mjs';
import { parseFragment } from 'parse5';

const requests = JSON.parse(readFileSync(0, 'utf8'));
const results = [];
for (const { tex, displayMode } of requests) {
  const { result } = await render(new URL('./node_modules/katex/dist/katex.mjs', import.meta.url),
    { tex, displayMode, output: 'htmlAndMathml' },
    { inputBytes: 1000000, outputBytes: 10000000 },
    { timeoutMillis: 10000, diagnosticBytes: 10000 });
  if (result.kind !== 'rendered-unchecked') throw new Error(JSON.stringify(result));
  const visual = await render(new URL('./node_modules/katex/dist/katex.mjs', import.meta.url),
    { tex, displayMode, output: 'html' },
    { inputBytes: 1000000, outputBytes: 10000000 },
    { timeoutMillis: 10000, diagnosticBytes: 10000 });
  if (visual.result.kind !== 'rendered-unchecked') throw new Error(JSON.stringify(visual.result));
  const parsed = parse(parseFragment, visual.result.html, { inputBytes: 10000000, nodes: 100000, depth: 1000 });
  if (parsed.kind !== 'parsed-unchecked') throw new Error(JSON.stringify(parsed));
  // Test inventory only. Production policy must bind names to the fixed CSS.
  const classes = [...new Set(parsed.nodes.filter(n => n.kind === 'span')
    .flatMap(n => n.classes.split(' ')).filter(Boolean))].sort();
  results.push({ version: result.version, html: result.html, visualHtml: visual.result.html, visual: parsed, classes });
}
process.stdout.write(JSON.stringify(results));
