// Explicit supervised visual parsing. Returned data is still unchecked content,
// never a renderer authenticity, resource usage or document admission proof.
import { render as preflight, byteLength } from '../render.mjs';
import { run } from './realm.mjs';

export async function renderVisual(renderer, parser, request, limits, parseLimits,
    { timeoutMillis, diagnosticBytes, replyBytes, signal } = {}) {
  if (signal?.aborted) return { result: { kind: 'stopped', reason: 'cancelled' } };
  if (![renderer, parser].every(url => url instanceof URL && url.protocol === 'file:') ||
      request?.output !== 'html' || !parseLimits ||
      ![parseLimits.inputBytes, parseLimits.nodes, parseLimits.depth, diagnosticBytes, replyBytes]
        .every(n => Number.isSafeInteger(n) && n >= 0) ||
      !Number.isSafeInteger(timeoutMillis) || timeoutMillis <= 0 || timeoutMillis > 2147483647) {
    return { result: { kind: 'invalid-request' } };
  }
  const valid = preflight(null, request, limits);
  if (valid.kind !== 'unavailable') return { result: valid };
  return run(new URL('./visual-worker.mjs', import.meta.url), {
    renderer: renderer.href, parser: parser.href,
    request: { tex: request.tex, displayMode: request.displayMode, output: 'html' },
    limits: { inputBytes: limits.inputBytes, outputBytes: limits.outputBytes },
    parseLimits: { inputBytes: parseLimits.inputBytes, nodes: parseLimits.nodes, depth: parseLimits.depth },
    diagnosticBytes, replyBytes,
  }, { timeoutMillis, signal }, value => decodeVisualReply(value, replyBytes));
}

export function decodeVisualReply(value, replyBytes) {

    // Fixed-size overflow/failure controls is reserved even when replyBytes is zero.
    // It omits unknown diagnostics rather than claiming a zero observation.
    if (typeof value !== 'string') {
      if (value && Object.keys(value).length === 1 && value.result &&
          Object.keys(value.result).length === 2 &&
          ((value.result.kind === 'stopped' && value.result.reason === 'reply-limit') ||
           (value.result.kind === 'provider-violation' && value.result.reason === 'serialization'))) return value;
      throw Error('invalid visual reply');
    }
    const size = byteLength(value, replyBytes);
    if (size === null || size > replyBytes) throw Error('oversized visual reply');
    const decoded = JSON.parse(value);
    if (!decoded || typeof decoded !== 'object' || !decoded.result ||
        typeof decoded.result.kind !== 'string' || !Array.isArray(decoded.diagnostics) ||
        !Number.isSafeInteger(decoded.diagnosticBytes)) throw Error('invalid visual envelope');
    return { ...decoded, replyBytes: size };
}
