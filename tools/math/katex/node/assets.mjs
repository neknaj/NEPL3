import pins from '../assets.json' with { type: 'json' };
import { readPinned } from './pinned.mjs';

/** Load the fixed package's complete CSS/font/license dependency set into owned
 * bytes. `root` is a trusted installed package directory URL. No output file is
 * written and no HTML is admitted here. Returned bytes, not later filesystem
 * reads, must be used by the artifact owner. */
export async function loadAssets(root, byteLimit) {
  if (!(root instanceof URL) || root.protocol !== 'file:' || !root.pathname.endsWith('/') ||
      !Number.isSafeInteger(byteLimit) || byteLimit < 0) return { kind: 'invalid-request' };
  const totalBytes = pins.files.reduce((total, file) => total + file.bytes, 0);
  if (totalBytes > byteLimit) return { kind: 'stopped', reason: 'asset-limit' };
  const files = [];
  for (const pin of pins.files) {
    try {
      const result = await readPinned(root, pin);
      if (result.kind !== 'file') return result;
      files.push(result.file);
    } catch {
      return { kind: 'unavailable', path: pin.path };
    }
  }
  return { kind: 'loaded', version: pins.version, totalBytes, files };
}
