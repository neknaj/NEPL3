import test from "node:test";
import assert from "node:assert/strict";
import { Protocol, expectedTests } from "./protocol.mjs";
import { loadUF2 } from "./uf2.mjs";

const complete = "@NEPL3/1 BEGIN\n" + expectedTests.map(id => `@NEPL3/1 PASS ${id}\n`).join("") + "@NEPL3/1 END\n";
function feed(text) { const p = new Protocol(); for (const b of Buffer.from(text)) p.byte(b); return p; }
test("strict complete result", () => assert.equal(feed(complete).done, true));
for (const [name, text] of [
  ["missing begin", "@NEPL3/1 END\n"],
  ["missing case", "@NEPL3/1 BEGIN\n@NEPL3/1 END\n"],
  ["failed case", complete.replace("PASS", "FAIL")],
  ["duplicate case", complete.replace("@NEPL3/1 END", "@NEPL3/1 PASS core.source\n@NEPL3/1 END")],
  ["unknown case", complete.replace("core.source", "core.unknown")],
  ["panic", "@NEPL3/1 BEGIN\n@NEPL3/1 PANIC\n"],
  ["trailing uart", complete + "@NEPL3/1 PANIC\n"],
  ["non ascii", "日"], ["line limit", "x".repeat(129)],
]) test(name, () => assert.throws(() => feed(text)));
test("partial result is not completion", () => assert.equal(feed(complete.slice(0, -1)).done, false));

function image() {
  const bytes = Buffer.alloc(1024);
  for (let n = 0; n < 2; n++) {
    const b = bytes.subarray(n * 512);
    [0x0a324655,0x9e5d5157,0x2000,0x10000000+n*256,256,n,2,0xe48bff56]
      .forEach((v,i) => b.writeUInt32LE(v,i*4));
    b.writeUInt32LE(0x0ab16f30,508);
  }
  bytes.writeUInt32LE(0x20042000,544);
  bytes.writeUInt32LE(0x10000109,548);
  return bytes;
}
test("UF2 vectors and exact payload", () => {
  const flash = new Uint8Array(2*1024*1024);
  assert.deepEqual(loadUF2(image(),flash),{sp:0x20042000,pc:0x10000109});
  assert.equal(Buffer.from(flash).readUInt32LE(256),0x20042000);
});
for (const [name, offset, value] of [
  ["magic",0,0], ["family",28,0], ["flags",8,0], ["size",16,257],
  ["count",24,3], ["ordinal",20,1], ["address",12,0x20000000],
  ["duplicate address",524,0x10000000], ["stack",544,0x20042008],
  ["unaligned stack",544,0x20042001], ["nonthumb pc",548,0x10000108],
  ["pc outside image",548,0x10000201],
]) test(`UF2 rejects ${name}`, () => {
  const bytes = image(); bytes.writeUInt32LE(value, offset);
  assert.throws(() => loadUF2(bytes,new Uint8Array(2*1024*1024)));
});
test("UF2 truncated and empty", () => {
  for (const b of [Buffer.alloc(0),image().subarray(0,1023)]) assert.throws(() => loadUF2(b,new Uint8Array(2*1024*1024)));
});
