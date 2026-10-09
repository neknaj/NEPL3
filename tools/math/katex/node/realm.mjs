// Shared trusted one-shot Worker lifecycle. Callers perform request validation.
import { Worker } from 'node:worker_threads';
import { performance } from 'node:perf_hooks';
import { finished } from 'node:stream/promises';
export async function run(workerUrl, workerData, { timeoutMillis, signal }, decode = value => value, execArgv = []) {
  const deadline = performance.now() + timeoutMillis;
  let worker;
  try {
    worker = new Worker(workerUrl, {
      workerData,
      // Do not inherit CLI preload hooks or write renderer output to the host.
      execArgv, stdout: true, stderr: true,
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
      else {
        let decoded;
        try { decoded = decode(value); } catch { violation('message'); return; }
        if (signal?.aborted) cancel();
        else if (performance.now() >= deadline) stop('deadline');
        else finish(decoded);
      }
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
