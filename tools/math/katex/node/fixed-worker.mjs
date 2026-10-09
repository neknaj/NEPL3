import { parentPort, workerData } from 'node:worker_threads';
import { setImmediate as tick } from 'node:timers/promises';
import { loadModules, executeVisual } from './modules.mjs';
import { render as preflight, byteLength } from '../render.mjs';
import { observeWarnings } from './fixed-warning.mjs';
const warnings=observeWarnings();
const diagnostics=[];let diagnosticBytes=0,overflow=false;
const sink={};
for(const level of ['log','info','warn','error','debug']) {
  sink[level]=(...args)=>{
    if(overflow)return;
    for(const arg of args){const text=typeof arg==='string'?arg:'[non-text renderer diagnostic]';const bytes=Buffer.byteLength(text,'utf8')+1;
      if(bytes>workerData.diagnosticBytes-diagnosticBytes){overflow=true;throw Error('diagnostic limit');}
      diagnosticBytes+=bytes;diagnostics.push({level,text});}
  };
  console[level]=sink[level];
}
Object.freeze(sink);
async function invoke() {
  const valid=preflight(null,workerData.request,workerData.limits);if(valid.kind!=='unavailable')return valid;
  const loaded=await loadModules(new URL(workerData.root),workerData.moduleBytes);
  if(loaded.kind!=='loaded')return loaded;
  return executeVisual(loaded.bundle,workerData.request,workerData.limits,workerData.parseLimits,sink);
}
let result=await invoke();
// Node emits warning events asynchronously. Drain that turn before accepting a
// result; do not suppress unexpected warnings along with the expected VM one.
await tick();
const observed=warnings.snapshot();
if(observed.unexpected)result={kind:'provider-violation',reason:'runtime-warning'};
if(overflow)result={kind:'stopped',reason:'diagnostic-limit'};
let encoded;
try {encoded=JSON.stringify({result,diagnostics,diagnosticBytes,implementation:{node:process.version,platform:process.platform,arch:process.arch,expectedVmWarnings:observed.expected}});}
catch {parentPort.postMessage({result:{kind:'provider-violation',reason:'serialization'}});parentPort.close();}
if(encoded!==undefined){
  const size=byteLength(encoded,workerData.replyBytes);
  if(size===null||size>workerData.replyBytes)parentPort.postMessage({result:{kind:'stopped',reason:'reply-limit'}});
  else parentPort.postMessage(encoded);
  parentPort.close();
}
