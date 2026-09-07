// Strict file transport only; NEPL3 semantics execute in the firmware.
export function loadUF2(bytes, flash) {
  if (!bytes.length || bytes.length % 512 || bytes.length > 4 * 1024 * 1024) throw Error("UF2 size");
  const count = bytes.length / 512;
  const seen = new Set();
  for (let n = 0; n < count; n++) {
    const b = bytes.subarray(n * 512, (n + 1) * 512);
    const address = b.readUInt32LE(12), offset = address - 0x10000000;
    if (b.readUInt32LE(0) !== 0x0a324655 || b.readUInt32LE(4) !== 0x9e5d5157 ||
        b.readUInt32LE(508) !== 0x0ab16f30 || b.readUInt32LE(8) !== 0x2000 ||
        b.readUInt32LE(16) !== 256 || b.readUInt32LE(20) !== n ||
        b.readUInt32LE(24) !== count || b.readUInt32LE(28) !== 0xe48bff56 ||
        offset < 0 || offset % 256 || offset + 256 > Math.min(flash.length, 2 * 1024 * 1024) || seen.has(offset)) throw Error("UF2 block");
    seen.add(offset);
  }
  if (!seen.has(0) || !seen.has(256)) throw Error("UF2 missing boot/vector");
  for (let n = 0; n < count; n++) {
    const b = bytes.subarray(n * 512, (n + 1) * 512);
    flash.set(b.subarray(32, 288), b.readUInt32LE(12) - 0x10000000);
  }
  const data = Buffer.from(flash.buffer, flash.byteOffset, flash.byteLength);
  const sp = data.readUInt32LE(256), pc = data.readUInt32LE(260);
  if (sp <= 0x20000000 || sp > 0x20042000 || sp % 8 || !(pc & 1) ||
      pc < 0x10000100 || pc >= 0x10200000 || !seen.has((pc - 0x10000000) & ~255)) throw Error("UF2 invalid vectors");
  return { sp, pc };
}
