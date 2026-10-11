import { open } from 'node:fs/promises';
import { createHash } from 'node:crypto';

export async function readPinned(root, pin, category = 'asset') {
  const file = await open(new URL(pin.source, root), 'r');
  try {
    const stat = await file.stat();
    if (!stat.isFile() || stat.size !== pin.bytes) return { kind: 'provider-violation', reason: `${category}-size`, path: pin.path };
    // Growth cannot increase the admitted read/allocation. Probe one extra byte.
    const bytes = Buffer.alloc(pin.bytes);
    let offset = 0;
    while (offset < bytes.length) {
      const part = await file.read(bytes, offset, bytes.length - offset, offset);
      if (part.bytesRead === 0) return { kind: 'provider-violation', reason: `${category}-size`, path: pin.path };
      offset += part.bytesRead;
    }
    const tail = await file.read(Buffer.alloc(1), 0, 1, offset);
    if (tail.bytesRead || createHash('sha256').update(bytes).digest('hex') !== pin.sha256) {
      return { kind: 'provider-violation', reason: `${category}-identity`, path: pin.path };
    }
    return { kind: 'file', file: { path: pin.path, mime: pin.mime, sha256: pin.sha256, bytes } };
  } finally { await file.close(); }
}

