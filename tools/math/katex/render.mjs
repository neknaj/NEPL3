// Shared synchronous generation call. The caller owns the isolated execution
// realm, timeout/cancellation, console capture, identity and artifact validation.
// KaTeX is injected so an unavailable module never prevents MathML startup.
export const KATEX_VERSION = '0.18.7';
const EXPANSION_LIMIT = 'Too many expansions: infinite loop or need to increase maxExpand setting';

// Count Unicode scalar UTF-8 bytes without allocating an encoded copy. Reject
// isolated surrogates instead of silently changing source through replacement.
export function byteLength(text, limit) {
  let size = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    if (c >= 0xd800 && c <= 0xdbff) {
      const next = text.charCodeAt(++i);
      if (!(next >= 0xdc00 && next <= 0xdfff)) return null;
      size += 4;
    } else if (c >= 0xdc00 && c <= 0xdfff) return null;
    else size += c < 0x80 ? 1 : c < 0x800 ? 2 : 3;
    if (size > limit) return size;
  }
  return size;
}

/** No returned HTML is a checked artifact or fidelity proof. Limits are byte
 * admission/output limits, not measurements of KaTeX allocation or execution
 * work. Synchronous work must be terminated by the outer Worker/process host.
 * Only generated TeX should reach this entry in the product pipeline. */
export function render(katex, request, limits) {
  if (!request || typeof request.tex !== 'string' ||
      typeof request.displayMode !== 'boolean' ||
      !['html', 'htmlAndMathml'].includes(request.output) || !limits ||
      ![limits.inputBytes, limits.outputBytes].every(n => Number.isSafeInteger(n) && n >= 0)) {
    return { kind: 'invalid-request' };
  }
  const inputBytes = byteLength(request.tex, limits.inputBytes);
  if (inputBytes === null) return { kind: 'invalid-request' };
  if (inputBytes > limits.inputBytes) return { kind: 'stopped', reason: 'input-limit' };
  if (!katex) return { kind: 'unavailable' };
  if (katex.version !== KATEX_VERSION || typeof katex.renderToString !== 'function' ||
      typeof katex.ParseError !== 'function') return { kind: 'provider-violation', reason: 'renderer' };
  try {
    const html = katex.renderToString(request.tex, {
      displayMode: request.displayMode,
      output: request.output,
      throwOnError: true,
      strict: 'error',
      trust: false,
      // Never reuse the mutable macro environment across expressions/documents.
      macros: Object.create(null),
      maxExpand: 1000,
      maxSize: 100,
    });
    if (typeof html !== 'string') return { kind: 'provider-violation', reason: 'output' };
    const outputBytes = byteLength(html, limits.outputBytes);
    if (outputBytes === null) return { kind: 'provider-violation', reason: 'output' };
    if (outputBytes > limits.outputBytes) return { kind: 'stopped', reason: 'output-limit' };
    return { kind: 'rendered-unchecked', version: KATEX_VERSION, html, inputBytes, outputBytes };
  } catch (error) {
    if (error instanceof katex.ParseError) {
      // Match only the underlying library message, never source excerpts that
      // a caller could use to spoof a resource failure.
      if (error.rawMessage === EXPANSION_LIMIT) return { kind: 'stopped', reason: 'expansion-limit' };
      // Do not send raw source-bearing exception text to a document or UI HTML.
      return { kind: 'render-error', reason: 'parse' };
    }
    return { kind: 'provider-violation', reason: 'exception' };
  }
}
