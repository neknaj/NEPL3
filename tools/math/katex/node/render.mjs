import { Worker } from 'node:worker_threads';
import { performance } from 'node:perf_hooks';
import { finished } from 'node:stream/promises';
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
  const deadline = performance.now() + timeoutMillis;
  let worker;
  try {
    worker = new Worker(new URL('./worker.mjs', import.meta.url), {
      workerData: {
        renderer: renderer.href,
        request: { tex: request.tex, displayMode: request.displayMode, output: request.output },
        limits: { inputBytes: limits.inputBytes, outputBytes: limits.outputBytes },
        diagnosticBytes,
      },
      // Do not inherit CLI preload hooks or write renderer output to the host.
      execArgv: [], stdout: true, stderr: true,
    });
  } catch {
    return { result: { kind: 'provider-violation', reason: 'worker-start' } };
  }
  return new Promise(resolve => {
    let selected;
    let timer;
    const stop = reason => {
      const value = { result: { kind: 'stopped', reason } };
      if (!selected) finish(value);
      else if (selected.result.kind !== 'stopped') selected = value;
    };
    const cancel = () => stop('cancelled');
    const violation = reason => {
      const value = { result: { kind: 'provider-violation', reason } };
      if (!selected) finish(value);
      else if (selected.result.kind !== 'stopped') selected = value;
    };
    const finish = value => {
      if (selected) return;
      selected = value;
      // `exit` resolves the operation, including an interrupted synchronous call.
      // A termination rejection must not claim that the realm has exited.
      void worker.terminate().catch(() => {
        selected = { ...selected, terminationFailure: true };
      });
    };
    worker.on('message', value => {
      if (signal?.aborted) cancel();
      else if (performance.now() >= deadline) stop('deadline');
      else finish(value);
    });
    worker.on('error', () => violation('worker'));
    // Console is captured structurally in worker.mjs. Unexpected direct stdio
    // is a violation, never renderer HTML or an unbounded raw log buffer.
    for (const stream of [worker.stdout, worker.stderr]) {
      stream.on('data', () => violation('stdio'));
    }
    const drained = Promise.all([worker.stdout, worker.stderr].map(stream =>
      finished(stream).catch(() => violation('stdio'))));
    worker.once('exit', async () => {
      await drained;
      if (signal?.aborted) cancel();
      else if (performance.now() >= deadline) stop('deadline');
      clearTimeout(timer);
      signal?.removeEventListener('abort', cancel);
      resolve(selected ?? { result: { kind: 'provider-violation', reason: 'worker-exit' } });
    });
    timer = setTimeout(() => stop('deadline'), Math.max(1, deadline - performance.now()));
    signal?.addEventListener('abort', cancel, { once: true });
    if (signal?.aborted) cancel();
  });
}
