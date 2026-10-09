// Negative VM lifecycle/observer fixture, never a renderer or source-pin oracle.
import { parentPort, workerData } from 'node:worker_threads';
import { writeFileSync } from 'node:fs';
import { setImmediate as tick } from 'node:timers/promises';
import vm from 'node:vm';
import { observeWarnings } from '../../../math/katex/node/fixed-warning.mjs';
const warnings=observeWarnings();
const enter=()=>writeFileSync(workerData.marker,'entered VM','utf8');
const context=vm.createContext({enter},{codeGeneration:{strings:false,wasm:false}});
const source=workerData.mode==='evaluate'?'enter(); while(true){}':workerData.mode==='invoke'?'export function busy(){enter();while(true){}}':'export const x=1';
const module=new vm.SourceTextModule(source,{context});
await module.link(()=>{throw Error('unexpected import');});await module.evaluate();
if(workerData.mode==='invoke')module.namespace.busy();
if(workerData.mode==='unexpected-warning')process.emitWarning('unexpected experimental warning',{type:'ExperimentalWarning'});
await tick();parentPort.postMessage({result:{kind:'observed',...warnings.snapshot()}});parentPort.close();
