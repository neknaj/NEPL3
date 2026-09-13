// Host-side HTML5 parsing for the visual-only profile. The returned finite tree
// is still untrusted: the Rust fragment validator must admit it. Never pass a
// repaired/stripped string to a document. Run synchronous parsing in the owning
// cancellable realm; input/node/depth caps do not measure parser heap or time.
import { byteLength } from './render.mjs';
const HTML = 'http://www.w3.org/1999/xhtml';
const SVG = 'http://www.w3.org/2000/svg';

export function parse(parseFragment, html, limits) {
  if (typeof html !== 'string' || !limits ||
      ![limits.inputBytes, limits.nodes, limits.depth].every(n => Number.isSafeInteger(n) && n >= 0)) {
    return { kind: 'invalid-request' };
  }
  const size = byteLength(html, limits.inputBytes);
  if (size === null) return { kind: 'invalid-request' };
  if (size > limits.inputBytes) return { kind: 'stopped', reason: 'input-limit' };
  if (typeof parseFragment !== 'function') return { kind: 'unavailable' };
  const malformed = Symbol('malformed');
  const exhausted = Symbol('exhausted');
  let stopReason;
  const stopped = reason => { stopReason = reason; throw exhausted; };
  try {
    const tree = parseFragment(html, { scriptingEnabled: false, sourceCodeLocationInfo: true,
      onParseError() { throw malformed; } });
    if (tree.childNodes.length !== 1 || tree.childNodes[0].tagName !== 'span') throw malformed;
    const rootLocation = tree.childNodes[0].sourceCodeLocation;
    if (!rootLocation || rootLocation.startOffset !== 0 || rootLocation.endOffset !== html.length) throw malformed;
    const nodes = [];
    const pending = [{ node: tree.childNodes[0], depth: 1, close: false }];
    const ids = new Map();
    let count = 0;
    while (pending.length) {
      const item = pending.pop();
      const node = item.node;
      if (!item.close) {
        if (++count > limits.nodes) stopped('node-limit');
        if (item.depth > limits.depth) stopped('depth-limit');
        const location = node.sourceCodeLocation;
        if (!location) throw malformed;
        if (node.nodeName === '#text') {
          // A coalesced text node can otherwise hide a discarded end tag.
          // Fixed KaTeX escapes literal '<'; never accept markup inside a text
          // node's source range merely because HTML5 tree repair removed it.
          if (html.slice(location.startOffset, location.endOffset).includes('<')) throw malformed;
          ids.set(node, nodes.length);
          nodes.push({ kind: 'text', text: node.value });
          continue;
        }
        if (!['span', 'svg', 'path', 'line'].includes(node.tagName)) throw malformed;
        if (node.namespaceURI !== (node.tagName === 'span' ? HTML : SVG)) throw malformed;
        if (['span', 'svg'].includes(node.tagName) && !node.sourceCodeLocation.endTag) throw malformed;
        if (['path', 'line'].includes(node.tagName) && node.childNodes.length) throw malformed;
        // HTML5's error callback does not report every ignored end tag or
        // implicit close. Require complete contiguous source coverage by the
        // explicit start tag, children, and end tag (or SVG self-close).
        if (!location.startTag || location.startOffset !== location.startTag.startOffset) throw malformed;
        let offset = location.startTag.endOffset;
        for (const child of node.childNodes) {
          const part = child.sourceCodeLocation;
          if (!part || part.startOffset !== offset || part.endOffset < part.startOffset) throw malformed;
          offset = part.endOffset;
        }
        if (location.endTag) {
          if (offset !== location.endTag.startOffset || location.endOffset !== location.endTag.endOffset) throw malformed;
        } else if (!['path', 'line'].includes(node.tagName) || offset !== location.endOffset ||
            html.slice(location.startTag.startOffset, offset).slice(-2) !== '/>') throw malformed;
        // Reject namespaced attributes except the exact SVG namespace binding.
        for (const a of node.attrs) {
          if (a.namespace && !(node.tagName === 'svg' && a.name === 'xmlns' &&
              a.namespace === 'http://www.w3.org/2000/xmlns/' && a.value === SVG)) throw malformed;
        }
        pending.push({ ...item, close: true });
        for (let i = node.childNodes.length - 1; i >= 0; i--) {
          pending.push({ node: node.childNodes[i], depth: item.depth + 1, close: false });
        }
        continue;
      }
      const attrs = new Map(node.attrs.map(a => [a.name, a.value]));
      const take = (key, fallback) => {
        const value = attrs.has(key) ? attrs.get(key) : fallback;
        attrs.delete(key);
        if (value === undefined) throw malformed;
        return value;
      };
      const children = node.childNodes.map(child => ids.get(child));
      let result;
      switch (node.tagName) {
        case 'span': {
          const hidden = take('aria-hidden', null);
          if (![null, 'true', 'false'].includes(hidden)) throw malformed;
          result = { kind: 'span', classes: take('class', ''), style: take('style', ''),
            aria_hidden: hidden === null ? null : hidden === 'true', children };
          break;
        }
        case 'svg':
          if (take('xmlns', SVG) !== SVG) throw malformed;
          result = { kind: 'svg', width: take('width'), height: take('height'),
            view_box: take('viewBox', null), aspect: take('preserveAspectRatio', null), children };
          break;
        case 'path': result = { kind: 'path', data: take('d') }; break;
        case 'line': result = { kind: 'line', x1: take('x1'), y1: take('y1'), x2: take('x2'),
          y2: take('y2'), stroke_width: take('stroke-width') }; break;
      }
      if (attrs.size) throw malformed;
      ids.set(node, nodes.length);
      nodes.push(result);
    }
    return { kind: 'parsed-unchecked', nodes };
  } catch (error) {
    if (error === exhausted) return { kind: 'stopped', reason: stopReason };
    return { kind: 'provider-violation', reason: error === malformed ? 'markup' : 'parser' };
  }
}
