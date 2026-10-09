// Negative lifecycle fixture, never a parser authenticity/fidelity oracle.
import { writeFileSync } from 'node:fs';
import { parseFragment as actual } from '../node_modules/parse5/dist/index.js';
const config = new URL(import.meta.url).searchParams;
const mode = config.get('mode');
const enter = () => writeFileSync(config.get('marker'), 'entered parser', 'utf8');
if (mode === 'busy-import') { enter(); while (true) { /* termination fixture */ } }
if (mode === 'warning-import') console.warn('parser import warning');
let calls = 0;
export function parseFragment(html, options) {
  calls++;
  if (mode === 'busy-call') { enter(); while (true) { /* termination fixture */ } }
  if (mode === 'throw') throw Error('parser failure detail must not escape');
  if (mode === 'warning-call') console.warn('parser call warning');
  if (mode === 'counter') console.warn(`parser invocation ${calls}`);
  if (mode === 'stdio') process.stderr.write('unexpected parser stdio');
  if (mode === 'exit') process.exit(0);
  if (mode === 'keep-alive') setTimeout(() => {}, 250);
  const tree = actual(html, options);
  if (mode === 'serialization') tree.childNodes[0].childNodes[0].value = { toJSON() { throw Error('serialization detail'); } };
  if (mode === 'serialization-loop') JSON.stringify = () => { enter(); while (true) { /* serialization termination fixture */ } };
  return tree;
}
