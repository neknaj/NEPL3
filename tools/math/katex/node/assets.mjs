import { open } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import pins from '../assets.json' with { type: 'json' };

async function readPinned(root, pin) {
  const file = await open(new URL(pin.source, root), 'r');
  try {
    const stat = await file.stat();
    if (!stat.isFile() || stat.size !== pin.bytes) return { kind: 'provider-violation', reason: 'asset-size', path: pin.path };
    // Growth cannot increase the admitted read/allocation. Probe one extra byte.
    const bytes = Buffer.alloc(pin.bytes);
    let offset = 0;
    while (offset < bytes.length) {
      const part = await file.read(bytes, offset, bytes.length - offset, offset);
      if (part.bytesRead === 0) return { kind: 'provider-violation', reason: 'asset-size', path: pin.path };
      offset += part.bytesRead;
    }
    const tail = await file.read(Buffer.alloc(1), 0, 1, offset);
    if (tail.bytesRead || createHash('sha256').update(bytes).digest('hex') !== pin.sha256) {
      return { kind: 'provider-violation', reason: 'asset-identity', path: pin.path };
    }
    return { kind: 'file', file: { path: pin.path, mime: pin.mime, sha256: pin.sha256, bytes } };
  } finally { await file.close(); }
}

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
