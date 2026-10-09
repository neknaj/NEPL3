import { run } from './realm.mjs';
import { render as preflight } from '../render.mjs';

/** Run the shared call in a disposable Node realm. `renderer` is a trusted
 * host-configured file URL, never a document-supplied module. This is not an OS
 * sandbox or an artifact admission API. Resolve only after the realm exits. */
export async function render(renderer, request, limits, { timeoutMillis, diagnosticBytes, signal } = {}) {
  if (signal?.aborted) return { result: { kind: 'stopped', reason: 'cancelled' } };
  if (!(renderer instanceof URL) || renderer.protocol !== 'file:' ||
      !Number.isSafeInteger(timeoutMillis) || timeoutMillis <= 0 || timeoutMillis > 2147483647 ||
      !Number.isSafeInteger(diagnosticBytes) || diagnosticBytes < 0) {
    return { result: { kind: 'invalid-request' } };
  }
  const valid = preflight(null, request, limits);
  if (valid.kind !== 'unavailable') return { result: valid };
  return run(new URL('./worker.mjs', import.meta.url), {
    renderer: renderer.href,
    request: { tex: request.tex, displayMode: request.displayMode, output: request.output },
    limits: { inputBytes: limits.inputBytes, outputBytes: limits.outputBytes },
    diagnosticBytes,
  }, { timeoutMillis, signal });
}
