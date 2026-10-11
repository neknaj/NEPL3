// Native Doc process entry. The Rust owner supplies a bounded private request
// and owns the process deadline/cancellation and response byte limit.
import { readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { withExecution } from './execution.mjs';
import { loadAssets } from './assets.mjs';
import { renderParsed } from './render.mjs';
import pins from '../execution.json' with { type: 'json' };
const REQUEST_BYTES = 1048576;
const RESPONSE_BYTES = 8388608;
const root = pathToFileURL(`${process.argv[2]}/`);
let bytes = 0;
const chunks = [];
for await (const chunk of process.stdin) {
  bytes += chunk.length;
  if (bytes > REQUEST_BYTES) throw Error('request-limit');
  chunks.push(chunk);
}
const input = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(Buffer.concat(chunks)));
if (!input || typeof input !== 'object' || Array.isArray(input) ||
    Object.keys(input).sort().join(',') !== 'identity,include_assets,requests' ||
    typeof input.include_assets !== 'boolean' ||
    typeof input.identity !== 'string' || !/^[a-f0-9]{64}$/.test(input.identity) ||
    !Array.isArray(input.requests) || input.requests.length > 256) throw Error('request-shape');
for (let i = 0; i < input.requests.length; i++) {
  const r = input.requests[i];
  if (!r || typeof r !== 'object' || Array.isArray(r) ||
      Object.keys(r).sort().join(',') !== 'displayMode,id,tex' || r.id !== i ||
      typeof r.tex !== 'string' || typeof r.displayMode !== 'boolean') throw Error('request-shape');
}
let response = await withExecution(root, 2000000, async ({ renderer, parser }) => {
  const assets = await loadAssets(new URL('katex/', root), 2000000);
  if (assets.kind !== 'loaded') return assets;
  const css = assets.files.find(f => f.path === 'katex.min.css');
  if (!css || css.sha256 !== pins.stylesheetSha256) return { kind: 'provider-violation', reason: 'stylesheet-identity' };
  const files = input.include_assets ? assets.files.map(file => ({ path: file.path, mime: file.mime,
    sha256: file.sha256, hex: file.bytes.toString('hex') })) : [];
  const results = [];
  let retained = Buffer.byteLength(JSON.stringify(files));
  for (const r of input.requests) {
    const output = await renderParsed(renderer, parser, { tex: r.tex, displayMode: r.displayMode, output: 'html' },
      { inputBytes: 65536, outputBytes: 262144, nodes: 65536, depth: 256, transportBytes: 1048576 },
      { timeoutMillis: 10000, diagnosticBytes: 8192 });
    if (output.terminationFailure) return { kind: 'provider-violation', reason: 'termination' };
    if (['stopped', 'provider-violation', 'invalid-request'].includes(output.result.kind)) return output.result;
    const entry = { id: r.id, ...output };
    retained += Buffer.byteLength(JSON.stringify(entry));
    if (retained > RESPONSE_BYTES - 65536) return { kind: 'stopped', reason: 'transport-limit' };
    results.push(entry);
  }
  return { kind: 'generated', results, files: results.some(r => r.result.kind === 'parsed-unchecked') ? files : [], node_version: process.versions.node,
    execution_sha256: createHash('sha256').update(await readFile(new URL('../execution.json', import.meta.url))).digest('hex') };
});
response = { identity: input.identity, ...response };
const encoded = JSON.stringify(response);
if (Buffer.byteLength(encoded) > RESPONSE_BYTES) throw Error('response-limit');
process.stdout.write(encoded);
