
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {loadUF2} from './original/tools/emulators/rp2040js/uf2.mjs';
import {Protocol} from './original/tools/emulators/rp2040js/protocol.mjs';
const real=readFileSync(new URL('./firmware.uf2',import.meta.url));let checks=0;
const bytes=(s,p=new Protocol())=>{for(const b of Buffer.from(s))p.byte(b);return p;};
const start='@NEPL3/1 BEGIN\n';const passed=['core.source','core.budget','wire.cbor','wire.rejection'].map(x=>'@NEPL3/1 PASS '+x+'\n').join('');const end='@NEPL3/1 END\n';
assert.equal(bytes(start+passed+end).done,true);checks++;
for(const s of [end,start+end,start+passed.replace('PASS','FAIL')+end,start+passed+'@NEPL3/1 PASS core.source\n',start+'@NEPL3/1 PANIC\n',start+start,start+'@NEPL3/1 PASS missing\n',start+passed+end+'@NEPL3/1 PANIC\n',start+'x'.repeat(129),start+'\0']){assert.throws(()=>bytes(s));checks++;}
assert.equal(bytes(start+passed).done,false);checks++;
assert.equal(bytes(start+passed+end.slice(0,-1)).done,false);checks++;
const ok=loadUF2(real,new Uint8Array(2*1024*1024));assert.equal(ok.pc&1,1);checks++;
for(const mutate of [b=>b.subarray(1),b=>Buffer.alloc(0),b=>{b.writeUInt32LE(0,0);return b;},b=>{b.writeUInt32LE(0,8);return b;},b=>{b.writeUInt32LE(0,28);return b;},b=>{b.writeUInt32LE(0x0fffff00,12);return b;},b=>{b.writeUInt32LE(0x10200000,12);return b;},b=>{b.writeUInt32LE(0x10000001,12);return b;},b=>{b.writeUInt32LE(255,16);return b;},b=>{b.writeUInt32LE(2,20);return b;},b=>{b.writeUInt32LE(1,24);return b;},b=>{b.writeUInt32LE(0x10000000,512+12);return b;},b=>{b.writeUInt32LE(0,512+32);return b;},b=>{b.writeUInt32LE(0x20042008,512+32);return b;},b=>{b.writeUInt32LE(0x10000100,512+36);return b;},b=>{b.writeUInt32LE(0x101fff01,512+36);return b;}]){
 assert.throws(()=>loadUF2(mutate(Buffer.from(real)),new Uint8Array(2*1024*1024)));checks++;
}
console.log(JSON.stringify({checks,result:'passed',sp:ok.sp.toString(16),pc:ok.pc.toString(16)}));
