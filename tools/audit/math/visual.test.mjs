import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, existsSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { Worker } from 'node:worker_threads';
import { parseFragment } from 'parse5';
import { parse } from '../../math/katex/parse.mjs';
import { render } from '../../math/katex/node/render.mjs';
import { renderVisual } from '../../math/katex/node/visual.mjs';
const katex=new URL('./node_modules/katex/dist/katex.mjs',import.meta.url);
const parser=new URL('./node_modules/parse5/dist/index.js',import.meta.url);
const renderer=new URL('./fixtures/renderer.mjs',import.meta.url);
const request=tex=>({tex,displayMode:false,output:'html'});
const limits={inputBytes:10000,outputBytes:100000};
const parsing={inputBytes:100000,nodes:10000,depth:1000};
const options={timeoutMillis:10000,diagnosticBytes:1000,replyBytes:1000000};
function fixture(mode,marker) {const url=new URL('./fixtures/parser.mjs',import.meta.url);url.searchParams.set('mode',mode);if(marker)url.searchParams.set('marker',marker);return url;}
const visual=(r,p,req=request('x'),l=limits,pl=parsing,o=options)=>renderVisual(r,p,req,l,pl,o);

test('real visual parsing matches baseline for inline and display Math',async()=>{
  for(const displayMode of [false,true]) {
    const req={...request(String.raw`\sqrt{\frac{x}{2}}`),displayMode};
    const old=await render(katex,req,limits,options);
    const next=await visual(katex,parser,req);
    assert.equal(next.result.kind,'visual-parsed-unchecked');
    assert.equal(next.result.html,old.result.html);
    assert.deepEqual(next.result.visual,parse(parseFragment,old.result.html,parsing));
    assert.equal(next.replyBytes,Buffer.byteLength(JSON.stringify({result:next.result,diagnostics:next.diagnostics,diagnosticBytes:next.diagnosticBytes})));
  }
});

test('exact byte and structural caps include serialized envelope expansion',async()=>{
  const req=request('unicode-json');
  const good=await visual(renderer,parser,req);
  assert.equal(good.result.kind,'visual-parsed-unchecked');
  assert.ok(good.replyBytes>good.result.outputBytes);
  const zero=await visual(renderer,parser,req,limits,parsing,{...options,replyBytes:0});
  assert.deepEqual(zero,{result:{kind:'stopped',reason:'reply-limit'}});
  for(const [kind,amount] of [['input',good.result.inputBytes],['html',good.result.outputBytes],['parse-input',good.result.outputBytes],['nodes',good.result.visual.nodes.length],['depth',2],['reply',good.replyBytes]]) {
    for(const enough of [true,false]) {
      const bound=amount-Number(!enough);const l={...limits},pl={...parsing},o={...options};
      if(kind==='input')l.inputBytes=bound;
      if(kind==='html')l.outputBytes=bound;
      if(kind==='parse-input')pl.inputBytes=bound;
      if(kind==='nodes')pl.nodes=bound;
      if(kind==='depth')pl.depth=bound;
      if(kind==='reply')o.replyBytes=bound;
      const got=await visual(renderer,parser,req,l,pl,o);
      assert.equal(got.result.kind,enough?'visual-parsed-unchecked':'stopped',kind);
      if(!enough)assert.equal(got.result.reason,{'input':'input-limit',html:'output-limit','parse-input':'input-limit',nodes:'node-limit',depth:'depth-limit',reply:'reply-limit'}[kind]);
    }
  }
});

test('preflight avoids imports and missing capability differs from malformed output',async()=>{
  const dir=mkdtempSync(join(tmpdir(),'nepl3-visual-preflight-'));
  try {
    const marker=join(dir,'entered');const hostile=fixture('busy-import',marker);
    for(const req of [{...request('x'),output:'htmlAndMathml'},request('\ud800')]) {
      assert.equal((await visual(renderer,hostile,req)).result.kind,'invalid-request');
      assert.ok(!existsSync(marker));
    }
    assert.equal((await visual(renderer,hostile,request('x'),limits,{...parsing,nodes:Infinity})).result.kind,'invalid-request');
    assert.ok(!existsSync(marker));
    assert.equal((await visual(renderer,new URL('./missing-parser.mjs',import.meta.url))).result.phase,'parser-import');
    assert.equal((await visual(new URL('./missing-renderer.mjs',import.meta.url),parser)).result.phase,'renderer-import');
    assert.equal((await visual(renderer,renderer)).result.reason,'parser-export');
    assert.equal((await visual(renderer,parser,request('bad-markup'))).result.reason,'markup');
    assert.equal((await visual(renderer,fixture('throw'))).result.reason,'parser');
    const serialization=await visual(renderer,fixture('serialization'));
    assert.equal(serialization.result.reason,'serialization');
    assert.equal(Object.hasOwn(serialization,'diagnostics'),false);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('parser import and synchronous parsing loops terminate before resolution',async()=>{
  const dir=mkdtempSync(join(tmpdir(),'nepl3-visual-loops-'));
  try {
    for(const mode of ['busy-import','busy-call','serialization-loop','renderer-loop']) {
      const marker=join(dir,mode);
      const got=await visual(renderer,mode==='renderer-loop'?parser:fixture(mode,marker),request(mode==='renderer-loop'?`busy:${marker}`:'x'),limits,parsing,{...options,timeoutMillis:1500});
      assert.deepEqual(got.result,{kind:'stopped',reason:'deadline'});
      assert.equal(readFileSync(marker,'utf8'),mode==='renderer-loop'?'entered synchronous render':'entered parser');
    }
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('parser cancellation before start, while running and during termination wins',async()=>{
  const cancelled=new AbortController();cancelled.abort();
  assert.equal((await visual(renderer,parser,request('x'),limits,parsing,{...options,signal:cancelled.signal})).result.reason,'cancelled');
  const dir=mkdtempSync(join(tmpdir(),'nepl3-visual-cancel-'));
  try {
    const marker=join(dir,'entered');const controller=new AbortController();
    const pending=visual(renderer,fixture('busy-call',marker),request('x'),limits,parsing,{...options,signal:controller.signal});
    const end=Date.now()+5000;
    while(!existsSync(marker)&&Date.now()<end)await delay(10);
    controller.abort();const got=await pending;
    assert.ok(existsSync(marker));assert.equal(got.result.reason,'cancelled');
  } finally {rmSync(dir,{recursive:true,force:true});}
  const controller=new AbortController();const terminate=Worker.prototype.terminate;
  Worker.prototype.terminate=function(){controller.abort();return terminate.call(this);};
  try {assert.equal((await visual(renderer,parser,request('x'),limits,parsing,{...options,signal:controller.signal})).result.reason,'cancelled');}
  finally {Worker.prototype.terminate=terminate;}
});

test('parser diagnostics, stdio, premature exit and module state stay isolated',async()=>{
  for(const mode of ['warning-import','warning-call']) {
    const good=await visual(renderer,fixture(mode));assert.equal(good.diagnostics.length,1);
    assert.equal(good.diagnosticBytes,Buffer.byteLength(good.diagnostics[0].text)+1);
    for(const enough of [true,false]) {
      const bounded=await visual(renderer,fixture(mode),request('x'),limits,parsing,{...options,diagnosticBytes:good.diagnosticBytes-Number(!enough)});
      assert.equal(bounded.result.kind,enough?'visual-parsed-unchecked':'stopped');
      if(!enough)assert.equal(bounded.result.reason,'diagnostic-limit');
    }
    const bad=await visual(renderer,fixture(mode),request('x'),limits,parsing,{...options,diagnosticBytes:0});
    assert.equal(bad.result.reason,'diagnostic-limit');
  }
  for(const mode of ['stdio','exit'])assert.equal((await visual(renderer,fixture(mode))).result.kind,'provider-violation');
  for(let n=0;n<2;n++)assert.equal((await visual(renderer,fixture('counter'))).diagnostics[0].text,'parser invocation 1');
});

test('termination rejection does not claim exit while the Worker stays alive',async()=>{
  const terminate=Worker.prototype.terminate;let observed=false,settled=false;
  Worker.prototype.terminate=function(){observed=true;return Promise.reject(Error('test termination refusal'));};
  try {
    const pending=visual(renderer,fixture('keep-alive')).then(value=>{settled=true;return value;});
    const end=Date.now()+5000;while(!observed&&Date.now()<end)await delay(5);
    assert.ok(observed);await delay(20);assert.equal(settled,false);
    const result=await pending;assert.equal(result.terminationFailure,true);
    assert.equal(result.result.kind,'visual-parsed-unchecked');
  } finally {Worker.prototype.terminate=terminate;}
});
