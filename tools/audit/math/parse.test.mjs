import test from 'node:test';
import assert from 'node:assert/strict';
import { parseFragment } from 'parse5';
import katex from 'katex';
import { parse } from '../../math/katex/parse.mjs';
const limits = { inputBytes: 1000000, nodes: 10000, depth: 1000 };
const read = html => parse(parseFragment, html, limits);

test('fixed renderer visual HTML becomes a finite postorder tree', () => {
  for (const tex of ['x', String.raw`\frac{1}{\sqrt{x}}`, String.raw`\overbrace{x+y}`, String.raw`\cancel{x}`]) {
    for (const displayMode of [false, true]) {
      const result = read(katex.renderToString(tex, { output: 'html', displayMode, trust: false, throwOnError: true }));
      assert.equal(result.kind, 'parsed-unchecked', tex);
      for (const [index, node] of result.nodes.entries()) {
        assert.ok(!node.children || node.children.every(child => child < index));
      }
    }
  }
});
test('reject repair, namespaces, unknown attributes and elements without stripping', () => {
  for (const html of ['<span>', '<span/>', '<span><script>x</script></span>', '<span onclick="x"></span>',
    '<span style="" style=""></span>', '<span><!--x--></span>', '<span><math></math></span>',
    '<span><svg width="1em" height="1em"><foreignObject><span>x</span></foreignObject></svg></span>',
    '<span><svg width="1em" height="1em"><path d="M0 0" onload="x"/></svg></span>',
    '<span><svg width="1em" height="1em"><path d="M0 0" xlink:href="x"/></svg></span>',
    '<span>\0</span>', '<span></span><span></span>']) {
    assert.equal(read(html).kind, 'provider-violation', html);
  }
  for (const html of ['<span></span></span>', '<span><span>x</span></span></bogus>',
    '<span>a</bogus>b</span>', '<span><span>x</span></bogus></span>',
    '<span><svg width="1em" height="1em"><path d="M0 0"></svg></span>',
    '<span><svg width="1em" height="1em"><line x1="0" y1="0" x2="100%" y2="100%" stroke-width="1em"></svg></span>']) {
    assert.deepEqual(read(html), { kind: 'provider-violation', reason: 'markup' }, html);
  }
});
test('HTML5 text decoding preserves literal content and limits remain explicit', () => {
  assert.equal(read('<span>&lt;script&gt;&amp;日本</span>').nodes[0].text, '<script>&日本');
  // HTML5 reports a control-character reference even for CR; the strict
  // renderer-input profile rejects parse errors instead of repairing them.
  assert.equal(read('<span>&#xD;</span>').kind, 'provider-violation');
  assert.equal(read('<span>\ud800</span>').kind, 'invalid-request');
  for (const [key, reason] of [['inputBytes', 'input-limit'], ['nodes', 'node-limit'], ['depth', 'depth-limit']]) {
    assert.deepEqual(parse(parseFragment, '<span>x</span>', { ...limits, [key]: 0 }), { kind: 'stopped', reason });
  }
});
