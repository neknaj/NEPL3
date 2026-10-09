// Fixed package-source route. Source binding remains separate from same-Math
// issuance, resource/Doc admission, runtime/bootstrap trust and remote proof.
import { render as preflight } from '../render.mjs';
import { run } from './realm.mjs';
import { decodeVisualReply } from './visual.mjs';
const FLAGS=['--experimental-vm-modules','--disable-warning=ExperimentalWarning'];
export async function renderFixedVisual(root,request,limits,parseLimits,
    {timeoutMillis,diagnosticBytes,replyBytes,moduleBytes,signal}={}) {
  if(signal?.aborted)return {result:{kind:'stopped',reason:'cancelled'}};
  if(!(root instanceof URL)||root.protocol!=='file:'||!root.pathname.endsWith('/')||
      request?.output!=='html'||!parseLimits||
      ![parseLimits.inputBytes,parseLimits.nodes,parseLimits.depth,diagnosticBytes,replyBytes,moduleBytes].every(n=>Number.isSafeInteger(n)&&n>=0)||
      !Number.isSafeInteger(timeoutMillis)||timeoutMillis<=0||timeoutMillis>2147483647)return {result:{kind:'invalid-request'}};
  const valid=preflight(null,request,limits);if(valid.kind!=='unavailable')return {result:valid};
  return run(new URL('./fixed-worker.mjs',import.meta.url),{
    root:root.href,request:{tex:request.tex,displayMode:request.displayMode,output:'html'},
    limits:{inputBytes:limits.inputBytes,outputBytes:limits.outputBytes},
    parseLimits:{inputBytes:parseLimits.inputBytes,nodes:parseLimits.nodes,depth:parseLimits.depth},
    diagnosticBytes,replyBytes,moduleBytes,
  },{timeoutMillis,signal},value=>decodeVisualReply(value,replyBytes),FLAGS);
}
