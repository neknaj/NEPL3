import { parentPort, workerData } from 'node:worker_threads';
import { render, byteLength } from '../render.mjs';
import { parse } from '../parse.mjs';

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
if (result.kind === 'rendered-unchecked' && workerData.parser) {
  let parser;
  try { parser = await import(workerData.parser); }
  catch { result = { kind: 'unavailable' }; }
  if (parser) {
    const rendered = result;
    result = parse(parser.parseFragment, rendered.html, {
      inputBytes: workerData.limits.outputBytes,
      nodes: workerData.limits.nodes,
      depth: workerData.limits.depth,
    });
    if (result.kind === 'parsed-unchecked') {
      result = { ...result, version: rendered.version,
        inputBytes: rendered.inputBytes, outputBytes: rendered.outputBytes };
      // Parsing, finite-tree conversion and JSON encoding are all terminated
      // by the same outer deadline. The cap bounds transport, not parser RSS.
      const encoded = JSON.stringify(result);
      const transportBytes = byteLength(encoded, workerData.limits.transportBytes);
      if (transportBytes === null) {
        result = { kind: 'provider-violation', reason: 'transport' };
      } else if (transportBytes > workerData.limits.transportBytes) {
        result = { kind: 'stopped', reason: 'transport-limit' };
      }
    }
  }
}
if (overflow) result = { kind: 'stopped', reason: 'diagnostic-limit' };
parentPort.postMessage({ result, diagnostics, diagnosticBytes });
parentPort.close();
