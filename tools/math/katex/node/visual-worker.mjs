import { parentPort, workerData } from 'node:worker_threads';
import { render, byteLength } from '../render.mjs';
import { parse } from '../parse.mjs';

const diagnostics = [];
let diagnosticBytes = 0;
let overflow = false;
for (const level of ['log', 'info', 'warn', 'error', 'debug']) {
  console[level] = (...args) => {
    if (overflow) return;
    for (const arg of args) {
      const text = typeof arg === 'string' ? arg : '[non-text renderer diagnostic]';
      const bytes = Buffer.byteLength(text, 'utf8') + 1;
      if (bytes > workerData.diagnosticBytes - diagnosticBytes) { overflow = true; return; }
      diagnosticBytes += bytes;
      diagnostics.push({ level, text });
    }
  };
}

async function invoke() {
  let renderer;
  try { renderer = await import(workerData.renderer); }
  catch { return { kind: 'unavailable', phase: 'renderer-import' }; }
  if (overflow) return { kind: 'stopped', reason: 'diagnostic-limit' };
  const output = render(renderer.default, workerData.request, workerData.limits);
  if (output.kind !== 'rendered-unchecked' || overflow) return output;
  let parser;
  try { parser = await import(workerData.parser); }
  catch { return { kind: 'unavailable', phase: 'parser-import' }; }
  if (overflow) return { kind: 'stopped', reason: 'diagnostic-limit' };
  if (typeof parser.parseFragment !== 'function') return { kind: 'provider-violation', reason: 'parser-export' };
  const visual = parse(parser.parseFragment, output.html, workerData.parseLimits);
  if (visual.kind !== 'parsed-unchecked') return visual;
  return { kind: 'visual-parsed-unchecked', version: output.version,
    inputBytes: output.inputBytes, outputBytes: output.outputBytes,
    html: output.html, visual };
}
let result = await invoke();
if (overflow) result = { kind: 'stopped', reason: 'diagnostic-limit' };
let encoded;
try { encoded = JSON.stringify({ result, diagnostics, diagnosticBytes }); }
catch {
  parentPort.postMessage({ result: { kind: 'provider-violation', reason: 'serialization' } });
  parentPort.close();
}
if (encoded !== undefined) {
  const bytes = byteLength(encoded, workerData.replyBytes);
  if (bytes === null || bytes > workerData.replyBytes) {
    parentPort.postMessage({ result: { kind: 'stopped', reason: 'reply-limit' } });
  } else { parentPort.postMessage(encoded); }
  parentPort.close();
}
