// Test adapter: executes production TeX against pinned KaTeX. This is not the
// product's host, output validator, or cancellation/asset/identity protocol.
import katex from 'katex';
import { readFileSync } from 'node:fs';

const requests = JSON.parse(readFileSync(0, 'utf8'));
const results = requests.map(({ tex, displayMode }) => ({
  version: katex.version,
  html: katex.renderToString(tex, {
    displayMode,
    output: 'htmlAndMathml',
    throwOnError: true,
    strict: 'error',
    trust: false,
    macros: {},
    maxExpand: 1000,
    maxSize: 100,
  }),
}));
process.stdout.write(JSON.stringify(results));
