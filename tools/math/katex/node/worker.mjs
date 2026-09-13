import { parentPort, workerData } from 'node:worker_threads';
import { render } from '../render.mjs';

// This realm is dedicated to one invocation of a trusted, pinned renderer.
// Capture before module loading; never patch the application's console.
const diagnostics = [];
let diagnosticBytes = 0;
let overflow = false;
for (const level of ['log', 'info', 'warn', 'error', 'debug']) {
  console[level] = (...args) => {
    if (overflow) return;
    for (const arg of args) {
      const text = typeof arg === 'string' ? arg : '[non-text renderer diagnostic]';
      // Charge an empty message too, bounding the number of records.
      const bytes = Buffer.byteLength(text, 'utf8') + 1;
      if (bytes > workerData.diagnosticBytes - diagnosticBytes) {
        overflow = true;
        return;
      }
      diagnosticBytes += bytes;
      diagnostics.push({ level, text });
    }
  };
}

let result;
try {
  const module = await import(workerData.renderer);
  result = render(module.default, workerData.request, workerData.limits);
} catch {
  // Loading the configured optional capability failed. Do not leak exception
  // text or let absence prevent the separately prepared MathML path.
  result = { kind: 'unavailable' };
}
if (overflow) result = { kind: 'stopped', reason: 'diagnostic-limit' };
parentPort.postMessage({ result, diagnostics, diagnosticBytes });
parentPort.close();
