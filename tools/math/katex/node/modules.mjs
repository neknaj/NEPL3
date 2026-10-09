// Fixed trusted package bytes -> privately retained text -> controlled linking.
// This is not an OS sandbox, arbitrary package loader or remote proof verifier.
import * as vm from 'node:vm';
import { open } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { PINS, IDENTITY } from './module-pins.mjs';
import { render } from '../render.mjs';
import { parse } from '../parse.mjs';
const owned=new WeakMap();
export function denyDynamicImport() {throw Error('dynamic imports are outside the fixed source graph');}
async function read(root,pin) {
  let file;
  try {file=await open(new URL(pin.id,root),'r');}
  catch {return {kind:'unavailable',reason:'module-file',id:pin.id};}
  try {
    const stat=await file.stat();
    if(!stat.isFile()||stat.size!==pin.bytes)return {kind:'provider-violation',reason:'module-size',id:pin.id};
    const bytes=Buffer.alloc(pin.bytes);let offset=0;
    while(offset<bytes.length){const part=await file.read(bytes,offset,bytes.length-offset,offset);if(!part.bytesRead)return {kind:'provider-violation',reason:'module-size',id:pin.id};offset+=part.bytesRead;}
    if((await file.read(Buffer.alloc(1),0,1,offset)).bytesRead)return {kind:'provider-violation',reason:'module-size',id:pin.id};
    if(createHash('sha256').update(bytes).digest('hex')!==pin.sha256)return {kind:'provider-violation',reason:'module-identity',id:pin.id};
    try {return {kind:'source',source:new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(bytes)};}
    catch {return {kind:'provider-violation',reason:'module-encoding',id:pin.id};}
  } finally {await file.close();}
}
/** No package code executes during loading. Bytes are retained privately as
 * exact decoded strings; no later path read participates in execution. */
export async function loadModules(root,byteLimit) {
  if(!(root instanceof URL)||root.protocol!=='file:'||!root.pathname.endsWith('/')||!Number.isSafeInteger(byteLimit)||byteLimit<0)return {kind:'invalid-request'};
  const total=PINS.files.reduce((n,f)=>n+f.bytes,0);
  if(total>byteLimit)return {kind:'stopped',reason:'module-limit'};
  const sources=new Map();
  try {
    for(const pin of PINS.files){const result=await read(root,pin);if(result.kind!=='source')return result;sources.set(pin.id,result.source);}
  } catch {return {kind:'provider-violation',reason:'module-read'};}
  const bundle=Object.freeze({identity:IDENTITY,sourceBytes:total,moduleCount:PINS.files.length});
  owned.set(bundle,sources);
  return {kind:'loaded',bundle};
}
/** Every call instantiates a fresh graph/context from verified owned text.
 * Only console is supplied; string/Wasm code generation and dynamic imports are
 * disabled. vm is not a security mechanism and the package code stays trusted. */
export async function executeVisual(bundle,request,limits,parseLimits,consoleSink) {
  const valid=render(null,request,limits);
  if(valid.kind!=='unavailable')return valid;
  if(request.output!=='html'||parse(null,'',parseLimits).kind!=='unavailable')return {kind:'invalid-request'};
  const sources=owned.get(bundle);
  if(!sources)return {kind:'provider-violation',reason:'unissued-module-bundle'};
  if(typeof vm.SourceTextModule!=='function'||!('moduleRequests' in vm.SourceTextModule.prototype))return {kind:'unavailable',reason:'vm-modules'};
  const modules=new Map();
  try {
    const context=vm.createContext({console:consoleSink},{codeGeneration:{strings:false,wasm:false}});
    for(const pin of PINS.files){
      const module=new vm.SourceTextModule(sources.get(pin.id),{context,identifier:pin.id,importModuleDynamically:denyDynamicImport});
      const requests=module.moduleRequests;
      if(requests.length!==pin.imports.length||requests.some((r,i)=>r.specifier!==pin.imports[i].specifier||r.phase!=='evaluation'||Object.keys(r.attributes).length))return {kind:'provider-violation',reason:'module-graph'};
      modules.set(pin.id,module);
    }
    const linker=(specifier,referencing,extra)=>{
      if(Object.keys(extra.attributes??{}).length)throw Error('import attributes');
      const pin=PINS.files.find(p=>p.id===referencing.identifier);
      const edge=pin?.imports.find(edge=>edge.specifier===specifier);
      if(!edge||!modules.has(edge.target))throw Error('unknown static import');
      return modules.get(edge.target);
    };
    const renderer=modules.get(PINS.entries.renderer),parser=modules.get(PINS.entries.parser);
    await renderer.link(linker);await parser.link(linker);
    await renderer.evaluate();await parser.evaluate();
    const output=render(renderer.namespace.default,request,limits);
    if(output.kind!=='rendered-unchecked')return output;
    if(typeof parser.namespace.parseFragment!=='function')return {kind:'provider-violation',reason:'parser-export'};
    const visual=parse(parser.namespace.parseFragment,output.html,parseLimits);
    if(visual.kind!=='parsed-unchecked')return visual;
    return {kind:'visual-parsed-unchecked',version:output.version,html:output.html,visual,inputBytes:output.inputBytes,outputBytes:output.outputBytes,
      sourceIdentity:IDENTITY,loaderPolicy:PINS.loaderPolicy,sourceBytes:bundle.sourceBytes,moduleCount:bundle.moduleCount};
  } catch {return {kind:'provider-violation',reason:'module-execution'};}
}
