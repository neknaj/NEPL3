import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { scan } from '../../generate/katex/selectors.mjs';
import { catalog } from '../../generate/katex/classes.mjs';
const css=readFileSync(new URL('./node_modules/katex/dist/katex.min.css',import.meta.url),'utf8');
const fixed=JSON.parse(readFileSync(new URL('../../math/katex/classes.json',import.meta.url),'utf8'));
test('fixed class catalog binds exact CSS and reviewed producer exceptions',()=>{
  assert.deepEqual(catalog(css,'0.18.7'),fixed);
  assert.equal(fixed.css_classes.length,138);assert.equal(fixed.classes.length,155);
  assert.deepEqual(fixed.classes,[...new Set(fixed.classes)].sort());
  assert.deepEqual(fixed.important_properties,['-ms-high-contrast-adjust']);
  const inert=Object.values(fixed.reviewed_inert).flatMap(v=>v.classes);
  assert.equal(inert.length,17);
  for(const name of ['mbin','mclose','minner','mop','mopen','mord','mpunct','mrel','mtight','text','armenian_fallback','brahmic_fallback','cjk_fallback','cyrillic_fallback','georgian_fallback','hangul_fallback','latin_fallback'])assert.ok(inert.includes(name));
  assert.throws(()=>catalog(css+'.unknown{}','0.18.7'));
  assert.throws(()=>catalog(css,'0.18.8'));
});
test('inventory reads selector tokens, not URL or quoted declaration text',()=>{
  assert.deepEqual(scan('@font-face{src:url(fonts/a.woff)}.a{content:".fake"}.b:not(.c):before{height:1em!important}'),{classes:['a','b','c'],importantProperties:['height']});
  for(const value of ['.a[href]{}','@media screen{.a{}}','.a:beforex{}','.a:hover{}','.a\\b{}','.a{content:"broken}','.a{color:red;/*comment*/}','.a{.b{}}','.a{width:calc(1px}'])assert.throws(()=>scan(value),value);
});
