import test from 'node:test';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const script=fileURLToPath(new URL('../../math/katex/node/stdio.mjs',import.meta.url));
const root=new URL('./node_modules/',import.meta.url).href;
const request={version:1,request:{tex:'x',displayMode:false,output:'html'},limits:{inputBytes:10000,outputBytes:100000},parseLimits:{inputBytes:100000,nodes:10000,depth:100},options:{timeoutMillis:5000,diagnosticBytes:10000,replyBytes:1000000,moduleBytes:953744}};
function invoke(input,{cap=100000,output=1000000,timeout=10000,end=true,rootURL=root}={}) {
 return new Promise((resolve,reject)=>{
  const p=spawn(process.execPath,[script,rootURL,String(cap),String(output),String(timeout)],{env:{...process.env,NODE_OPTIONS:''},stdio:['pipe','pipe','pipe']});
  let out='',err=''; const watchdog=setTimeout(()=>{p.kill();reject(Error('test watchdog'));},15000);
  p.on('error',reject);p.stdin.on('error',()=>{});p.stdout.on('data',b=>{out+=b;assert.ok(out.length<2000000);});p.stderr.on('data',b=>{err+=b;});
  p.on('close',(code,signal)=>{clearTimeout(watchdog);try {assert.equal(code,0);assert.equal(signal,null);assert.equal(err,'');resolve(JSON.parse(out));}catch(e){reject(e);}});
  if(input!==undefined)p.stdin.write(input);if(end)p.stdin.end();
 });
}
test('real fixed one-shot transport yields bounded unchecked tree',async()=>{
 const x=await invoke(JSON.stringify(request));assert.equal(x.result.kind,'visual-parsed-unchecked');assert.equal(x.result.moduleCount,20);
});
test('transport rejects malformed encoding, multiple JSON values and extra fields',async()=>{
 for(const x of [Buffer.from([255]),'{} {}','',JSON.stringify({...request,extra:true}),JSON.stringify({...request,request:{...request.request,renderer:'evil'}})]) assert.equal((await invoke(x)).result.kind,'invalid-request');
});
test('transport input/output caps and stalled stdin terminate',async()=>{
 assert.equal((await invoke('ab',{cap:1})).result.reason,'transport-input-limit');
 assert.equal((await invoke(JSON.stringify(request),{output:256})).result.reason,'transport-output-limit');
 assert.equal((await invoke(undefined,{timeout:30,end:false})).result.reason,'transport-deadline');
});

test('closed versioned wire rejects duplicate keys, alternate spellings and bad roots',async()=>{
 const text=JSON.stringify(request);
 for(const value of [text+' ',text.replace('"version":1','"version":1,"version":1'),text.replace('"version":1','"version":2'),text.replace('10000','1e4'),text.replace('10000','9007199254740993')])assert.equal((await invoke(value)).result.kind,'invalid-request');
 for(const url of ['file://server/share/','file:///tmp/?query','file:///tmp/#fragment'])assert.equal((await invoke(text,{rootURL:url})).result.reason,'transport-config');
 assert.equal((await invoke(text,{cap:Buffer.byteLength(text)})).result.kind,'visual-parsed-unchecked');
 assert.equal((await invoke(text,{cap:Buffer.byteLength(text)-1})).result.reason,'transport-input-limit');
 for(const limits of [{inputBytes:-1,outputBytes:100},{inputBytes:1.5,outputBytes:100},{inputBytes:1,outputBytes:100,extra:0}])assert.equal((await invoke(JSON.stringify({...request,limits}))).result.kind,'invalid-request');
});
test('final stdout cap covers exact re-encoded envelope and closed pipe fails',async()=>{
 const text=JSON.stringify(request), first=await invoke(text);
 const bytes=Buffer.byteLength(JSON.stringify(first));
 assert.equal((await invoke(text,{output:bytes})).result.kind,'visual-parsed-unchecked');
 assert.equal((await invoke(text,{output:bytes-1})).result.reason,'transport-output-limit');
 await new Promise((resolve,reject)=>{
  const p=spawn(process.execPath,[script,root,'100000','1000000','10000'],{env:{...process.env,NODE_OPTIONS:''},stdio:['pipe','pipe','pipe']});
  const timer=setTimeout(()=>{p.kill();reject(Error('closed-pipe watchdog'));},15000);
  let err='';p.stderr.on('data',b=>err+=b);p.stdin.on('error',()=>{});p.on('error',reject);
  p.stdout.destroy();p.stdin.end(text);
  p.on('close',code=>{clearTimeout(timer);try{assert.equal(code,1);assert.equal(err,'');resolve();}catch(e){reject(e);}});
 });
});
test('renderer failure categories survive the bridge without fallback',async()=>{
 const stopped=(value,reason)=>{assert.equal(value.result.kind,'stopped');assert.equal(value.result.reason,reason);};
 const changed=(tex,options={})=>JSON.stringify({...request,request:{...request.request,tex},options:{...request.options,...options}});
 assert.equal((await invoke(changed('x'),{rootURL:new URL('./missing-node-modules/',import.meta.url).href})).result.kind,'unavailable');
 assert.equal((await invoke(changed(String.raw`\unknownCommand`))).result.kind,'render-error');
 stopped((await invoke(changed(String.raw`\message{diagnostic}x`,{diagnosticBytes:0}))),'diagnostic-limit');
 stopped((await invoke(changed('x',{replyBytes:0}))),'reply-limit');
 stopped((await invoke(changed('x',{moduleBytes:0}))),'module-limit');
 stopped((await invoke(changed('x',{timeoutMillis:1}))),'deadline');
 assert.equal((await invoke(changed('\ud800'))).result.kind,'invalid-request');
 stopped((await invoke(changed('x'),{timeout:1})),'transport-deadline');
});
test('multibyte input byte caps and repeated-call independence',async()=>{
 const text=JSON.stringify({...request,request:{...request.request,tex:String.raw`\text{日本}`}}),bytes=Buffer.from(text);
 const got=await new Promise((resolve,reject)=>{
  const p=spawn(process.execPath,[script,root,String(bytes.length),'1000000','10000'],{env:{...process.env,NODE_OPTIONS:''},stdio:['pipe','pipe','pipe']});
  const timer=setTimeout(()=>{p.kill();reject(Error('split-input watchdog'));},15000);let out='',err='';
  p.on('error',reject);p.stdin.on('error',()=>{});p.stdout.on('data',b=>out+=b);p.stderr.on('data',b=>err+=b);
  p.on('close',code=>{clearTimeout(timer);try{assert.equal(code,0);assert.equal(err,'');resolve(JSON.parse(out));}catch(e){reject(e);}});
  // Parent writes split a codepoint; the OS may coalesce them. This does not
  // certify separate read-side chunks, only exact UTF-8 byte admission.
  const split=bytes.indexOf(Buffer.from('日'))+1;p.stdin.write(bytes.subarray(0,split));setTimeout(()=>p.stdin.end(bytes.subarray(split)),5);
 });
 assert.equal(got.result.kind,'visual-parsed-unchecked');
 assert.equal((await invoke(text,{cap:bytes.length-1})).result.reason,'transport-input-limit');
 const first={...request,request:{...request.request,tex:String.raw`\gdef\transportMacro{x}\transportMacro`}};
 assert.equal((await invoke(JSON.stringify(first))).result.kind,'visual-parsed-unchecked');
 assert.equal((await invoke(JSON.stringify({...request,request:{...request.request,tex:String.raw`\transportMacro`}}))).result.kind,'render-error');
});
