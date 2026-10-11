// Execute only bytes checked against the repository's reviewed executable pins.
// A private copy closes the installed-file check/use race. This does not isolate
// a hostile OS user, authenticate arbitrary paths or supply an OS sandbox.
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { pathToFileURL } from 'node:url';
import pins from '../execution.json' with { type: 'json' };
import { readPinned } from './pinned.mjs';

/** Host-configured node_modules directory, never document input. The callback
 * must await all workers before returning; private copies are removed afterward.
 * Retained byte caps are logical limits, not total heap or physical memory. */
export async function withExecution(root, byteLimit, use) {
  if (!(root instanceof URL) || root.protocol !== 'file:' || !root.pathname.endsWith('/') ||
      !Number.isSafeInteger(byteLimit) || byteLimit < 0 || typeof use !== 'function') {
    return { kind: 'invalid-request' };
  }
  const total = pins.files.reduce((sum, pin) => sum + pin.bytes, 0);
  if (total > byteLimit) return { kind: 'stopped', reason: 'execution-limit' };
  const owned = [];
  for (const pin of pins.files) {
    let result;
    try { result = await readPinned(root, { ...pin, source: pin.path }, 'execution'); }
    catch { return { kind: 'unavailable', path: pin.path }; }
    if (result.kind !== 'file') return result;
    owned.push(result.file);
  }
  const directory = await mkdtemp(join(tmpdir(), 'nepl3-katex-execution-'));
  try {
    for (const file of owned) {
      const destination = join(directory, 'node_modules', file.path);
      await mkdir(dirname(destination), { recursive: true });
      await writeFile(destination, file.bytes, { flag: 'wx' });
    }
    return await use({
      renderer: pathToFileURL(join(directory, 'node_modules/katex/dist/katex.mjs')),
      parser: pathToFileURL(join(directory, 'node_modules/parse5/dist/index.js')),
      totalBytes: total,
    });
  } finally { await rm(directory, { recursive: true, force: true }); }
}
