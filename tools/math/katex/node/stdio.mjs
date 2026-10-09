// Internal one-shot native-host transport. This is unchecked renderer data,
// not same-Math admission, a portable Report or a document artifact.
import { performance } from 'node:perf_hooks';
import { renderFixedVisual } from './fixed.mjs';

const control = (kind, reason) => ({ version: 1, result: { kind, reason } });
const exact = (x, keys) => x !== null && typeof x === 'object' && !Array.isArray(x) &&
  Object.keys(x).length === keys.length && keys.every(k => Object.hasOwn(x, k));
const natural = n => Number.isSafeInteger(n) && n >= 0;
function valid(x) {
  return exact(x, ['version','request','limits','parseLimits','options']) && x.version === 1 &&
    exact(x.request, ['tex','displayMode','output']) &&
    typeof x.request.tex === 'string' && typeof x.request.displayMode === 'boolean' && x.request.output === 'html' &&
    exact(x.limits, ['inputBytes','outputBytes']) && Object.values(x.limits).every(natural) &&
    exact(x.parseLimits, ['inputBytes','nodes','depth']) && Object.values(x.parseLimits).every(natural) &&
    exact(x.options, ['timeoutMillis','diagnosticBytes','replyBytes','moduleBytes']) && Object.values(x.options).every(natural) &&
    x.options.timeoutMillis > 0 && x.options.timeoutMillis <= 2147483647;
}
function read(input, cap, signal) {
  return new Promise(resolve => {
    const chunks=[]; let bytes=0, ended=false;
    const finish = value => {
      if(ended)return; ended=true;
      input.off('data', data); input.off('end', end); input.off('error', error);
      signal.removeEventListener('abort', abort);
      // No further stdin is part of this one-shot request, including on failure.
      input.destroy(); resolve(value);
    };
    const data = chunk => {
      if(chunk.length > cap-bytes) { finish(control('stopped','transport-input-limit')); return; }
      bytes+=chunk.length; chunks.push(chunk);
    };
    const end = () => finish({ bytes: Buffer.concat(chunks, bytes) });
    const error = () => finish(control('provider-violation','transport-input'));
    const abort = () => finish(control('stopped','transport-deadline'));
    input.on('data', data); input.once('end', end); input.once('error', error);
    signal.addEventListener('abort', abort, {once:true}); if(signal.aborted)abort();
  });
}
async function main() {
  // Host-configured directory URL and transport caps, never renderer URLs from
  // the JSON request. The parent must bound pipes and own process termination.
  const args=process.argv.slice(2);
  if(args.length!==4) return control('invalid-request','transport-config');
  let root; try {root=new URL(args[0]);} catch {return control('invalid-request','transport-config');}
  const numbers=args.slice(1).map(s => /^(0|[1-9][0-9]*)$/.test(s) ? Number(s) : NaN);
  const [inputCap,outputCap,timeout]=numbers;
  if(root.protocol!=='file:' || root.hostname!=='' || root.search!=='' || root.hash!=='' || !root.pathname.endsWith('/') || !numbers.every(natural) ||
      outputCap<256 || timeout<1 || timeout>2147483647) return control('invalid-request','transport-config');
  const controller=new AbortController(), deadline=performance.now()+timeout;
  const timer=setTimeout(()=>controller.abort(), timeout);
  try {
    const invoke=async()=>{
    const incoming=await read(process.stdin,inputCap,controller.signal);
    if(incoming.result)return incoming;
    let x;
    try {
      const text=new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(incoming.bytes);
      x=JSON.parse(text);
      // Closed internal wire spelling: rejects collapsed duplicate keys, unsafe
      // rounded numbers and alternate whitespace/escape/number spellings.
      if(JSON.stringify(x)!==text)return control('invalid-request','transport-canonical');
    }
    catch {return control('invalid-request','transport-json');}
    if(!valid(x))return control('invalid-request','transport-schema');
    if(performance.now()>=deadline)controller.abort();
    const value=await renderFixedVisual(root,x.request,x.limits,x.parseLimits,{...x.options,signal:controller.signal});
    if(controller.signal.aborted || performance.now()>=deadline)return control('stopped','transport-deadline');
    const encoded=JSON.stringify({version:1,...value});
    if(Buffer.byteLength(encoded,'utf8')>outputCap)return control('stopped','transport-output-limit');
    if(performance.now()>=deadline)return control('stopped','transport-deadline');
    return encoded;
    };
    const outcome=await invoke();
    // Also covers synchronous malformed/schema/overflow paths before a timer
    // callback has had a turn. Deadline takes precedence over those outcomes.
    if(controller.signal.aborted || performance.now()>=deadline)return control('stopped','transport-deadline');
    return outcome;
  } finally {clearTimeout(timer);}
}
let result;
try {result=await main();} catch {result=control('provider-violation','transport-exception');}
// Small failure controls have a reserved 256-byte channel. No source text or
// stack trace is returned. Exit/drain, rather than JSON alone, authorizes use.
const encoded=typeof result==='string'?result:JSON.stringify(result);
process.stdout.on('error',()=>{process.exitCode=1;});
process.stdout.write(encoded, error=>{if(error)process.exitCode=1;});
