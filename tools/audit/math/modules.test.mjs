import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { readFileSync,writeFileSync,mkdirSync,mkdtempSync,rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join,dirname } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { PINS,IDENTITY } from '../../math/katex/node/module-pins.mjs';
import { loadModules,executeVisual,denyDynamicImport } from '../../math/katex/node/modules.mjs';
import { renderFixedVisual } from '../../math/katex/node/fixed.mjs';
import { renderVisual } from '../../math/katex/node/visual.mjs';
import { run } from '../../math/katex/node/realm.mjs';
const root=new URL('./node_modules/',import.meta.url);
const limits={inputBytes:100000,outputBytes:1000000};
const parsing={inputBytes:1000000,nodes:100000,depth:1000};
const options={timeoutMillis:10000,diagnosticBytes:10000,replyBytes:5000000,moduleBytes:953744};
const request=tex=>({tex,displayMode:false,output:'html'});
const sink={log(){},info(){},warn(){},error(){},debug(){}};
const flags=['--experimental-vm-modules','--disable-warning=ExperimentalWarning'];
function fixture(){const dir=mkdtempSync(join(tmpdir(),'nepl3-owned-modules-'));for(const pin of PINS.files){const path=join(dir,pin.id);mkdirSync(dirname(path),{recursive:true});writeFileSync(path,readFileSync(new URL(pin.id,root)));}return {dir,url:pathToFileURL(dir+'/')};}

test('complete fixed graph identity and opaque owned source loading',async()=>{
  const text=readFileSync(new URL('../../math/katex/modules.json',import.meta.url));
  assert.equal(IDENTITY,createHash('sha256').update('nepl3.katex.source-catalog/1\0').update(text).digest('hex'));
  assert.equal(PINS.files.length,20);assert.equal(PINS.files.reduce((n,f)=>n+f.bytes,0),953744);
  assert.ok(PINS.files.flatMap(f=>f.imports).some(e=>e.specifier==='entities/decode'));
  assert.ok(PINS.files.flatMap(f=>f.imports).some(e=>e.specifier==='entities/escape'));
  assert.ok(Object.isFrozen(PINS)&&Object.isFrozen(PINS.files)&&Object.isFrozen(PINS.files[0].imports));
  const loaded=await loadModules(root,953744);assert.equal(loaded.kind,'loaded');assert.ok(Object.isFrozen(loaded.bundle));
  assert.equal((await executeVisual({...loaded.bundle},request('x'),limits,parsing,sink)).reason,'unissued-module-bundle');
  assert.equal((await loadModules(new URL('./missing-modules/',import.meta.url),953743)).reason,'module-limit');
  assert.equal((await loadModules(new URL('./missing-modules/',import.meta.url),953744)).kind,'unavailable');
});

test('every changed source is rejected before a bundle is issued',async()=>{
  const f=fixture();
  try {
    for(const pin of PINS.files){const path=join(f.dir,pin.id);const original=readFileSync(path);const bad=Buffer.from(original);bad[0]^=1;writeFileSync(path,bad);
      const result=await loadModules(f.url,953744);assert.equal(result.kind,'provider-violation');assert.equal(result.reason,'module-identity');assert.equal(result.id,pin.id);assert.equal(Object.hasOwn(result,'bundle'),false);writeFileSync(path,original);}
    const pin=PINS.files[0],path=join(f.dir,pin.id),original=readFileSync(path);
    for(const bytes of [original.subarray(0,original.length-1),Buffer.concat([original,Buffer.from('x')])]){writeFileSync(path,bytes);assert.equal((await loadModules(f.url,953744)).reason,'module-size');}
    const invalid=Buffer.from(original);invalid[0]=255;writeFileSync(path,invalid);assert.equal((await loadModules(f.url,953744)).kind,'provider-violation');
    rmSync(path);assert.equal((await loadModules(f.url,953744)).kind,'unavailable');
  }finally{rmSync(f.dir,{recursive:true,force:true});}
});

test('post-verification path replacement cannot substitute executing source',async()=>{
  const f=fixture();
  try {
    const loaded=await loadModules(f.url,953744);assert.equal(loaded.kind,'loaded');
    writeFileSync(join(f.dir,PINS.entries.renderer),'throw Error("substituted file")');
    const result=await executeVisual(loaded.bundle,request(String.raw`\frac{x}{2}`),limits,parsing,sink);
    assert.equal(result.kind,'visual-parsed-unchecked');assert.equal(result.sourceIdentity,IDENTITY);
    assert.equal((await loadModules(f.url,953744)).kind,'provider-violation');
    const first=await executeVisual(loaded.bundle,request(String.raw`\gdef\privateMacro{x}\privateMacro`),limits,parsing,sink);
    assert.equal(first.kind,'visual-parsed-unchecked');
    const next=await executeVisual(loaded.bundle,request(String.raw`\privateMacro`),limits,parsing,sink);
    assert.equal(next.kind,'render-error');
  }finally{rmSync(f.dir,{recursive:true,force:true});}
});

test('fixed Worker output matches native-import output and captures VM console',async()=>{
  for(const tex of ['x',String.raw`\frac{1}{0}`,String.raw`\sqrt{x}`,String.raw`\sum_{i=1}^{n}i`,String.raw`\text{日本}`])for(const displayMode of [false,true]){
    const req={...request(tex),displayMode};
    const got=await renderFixedVisual(root,req,limits,parsing,options);
    const old=await renderVisual(new URL('katex/dist/katex.mjs',root),new URL('parse5/dist/index.js',root),req,limits,parsing,options);
    assert.equal(got.result.kind,'visual-parsed-unchecked');assert.equal(got.result.html,old.result.html);assert.deepEqual(got.result.visual,old.result.visual);assert.equal(got.result.sourceIdentity,IDENTITY);assert.equal(got.implementation.expectedVmWarnings,1);
  }
  const warning=await renderFixedVisual(root,request(String.raw`\message{owned diagnostic}x`),limits,parsing,options);
  assert.ok(warning.diagnostics.some(d=>d.text.includes('owned diagnostic')));
  const stopped=await renderFixedVisual(root,request(String.raw`\message{owned diagnostic}x`),limits,parsing,{...options,diagnosticBytes:0});assert.equal(stopped.result.reason,'diagnostic-limit');
});

test('fixed route caps and abort preserve stopped outcomes',async()=>{
  assert.equal((await renderFixedVisual(root,request('x'),limits,parsing,{...options,moduleBytes:953743})).result.reason,'module-limit');
  const valid=await renderFixedVisual(root,request(String.raw`\text{字}`),limits,parsing,options);
  assert.equal(valid.result.kind,'visual-parsed-unchecked');
  for(const enough of [true,false])assert.equal((await renderFixedVisual(root,request(String.raw`\text{字}`),limits,parsing,{...options,replyBytes:valid.replyBytes-Number(!enough)})).result.kind,enough?'visual-parsed-unchecked':'stopped');
  const controller=new AbortController();const pending=renderFixedVisual(root,request('x'),limits,parsing,{...options,signal:controller.signal});controller.abort();assert.equal((await pending).result.reason,'cancelled');
  assert.equal((await renderFixedVisual(root,request('x'),limits,parsing,{...options,timeoutMillis:1})).result.reason,'deadline');
  assert.equal((await renderFixedVisual(root,request('\ud800'),limits,parsing,options)).result.kind,'invalid-request');
});

test('dynamic imports have no fallback and string code generation is disabled',async()=>{
  const context=vm.createContext({}, {codeGeneration:{strings:false,wasm:false}});
  const module=new vm.SourceTextModule('export const value=import("node:fs")',{context,importModuleDynamically:denyDynamicImport});await module.link(()=>{throw Error('unexpected static import');});await module.evaluate();await assert.rejects(module.namespace.value,/dynamic imports/);
  const code=new vm.SourceTextModule('Function("return 1")()',{context});await code.link(()=>{throw Error('import');});await assert.rejects(code.evaluate(),/Code generation/);
});

test('VM warning observation catches unexpected suppressed-category warnings',async()=>{
  for(const mode of ['expected-warning','unexpected-warning']){
    const output=await run(new URL('./fixtures/vm-worker.mjs',import.meta.url),{mode},{timeoutMillis:5000},v=>v,flags);
    assert.equal(output.result.expected,1);assert.equal(output.result.unexpected,mode==='unexpected-warning');
  }
});

test('entered VM evaluation and invocation loops still terminate under parent deadline',async()=>{
  const dir=mkdtempSync(join(tmpdir(),'nepl3-vm-lifecycle-'));
  try{for(const mode of ['evaluate','invoke']){const marker=join(dir,mode);const got=await run(new URL('./fixtures/vm-worker.mjs',import.meta.url),{mode,marker},{timeoutMillis:1500},v=>v,flags);assert.equal(got.result.reason,'deadline');assert.equal(readFileSync(marker,'utf8'),'entered VM');}}
  finally{rmSync(dir,{recursive:true,force:true});}
});
