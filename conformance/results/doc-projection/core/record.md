# Independent restricted Doc Markdown projection review

Review-only. No production or Git mutation. Initial base: 64970b2. A private 629-file workspace was copied and hashed before tests. The only scratch changes are an additional independent test module and its parent module registration. Original production hashes remain in files.json/workspace-files.json; subsequent changed projection source and managed tests are retained under delta/ and final/ with separate manifests. The original input probe and original failure log are retained; the initial workspace source paths were subsequently overlaid by these explicit deltas.

## Findings and corrections

1. Actual Doc adjacent InlineCode(a), InlineCode(b) was emitted as one CommonMark Code payload a``b. Initial test failed with one event versus two. The final source explicitly rejects adjacent InlineCode, including the additional delimiter variants. It does not claim to preserve them via insertion of new text.
2. An unordered list inside the final Section body followed by another list in the containing Article body was emitted as one Markdown list. Initial test failed with one List event versus two. Final source explicitly rejects mixed outer Section/content and nested sections. Root-only sibling sections remain supported. This also avoids silently reparenting outer blocks through Markdown heading scope.
3. A source-less typed document consumed 1563 OutputBytes during prepare::inspect. Giving the combined markdown operation exactly that allowance originally still returned generated Markdown with Usage.output_bytes=1563. Final Writer::emit charges output bytes before allocation/write; the unchanged probe now returns Stopped(OutputLimit), without exposing a partial String.

The initial three failures are recorded in native-before.log. The intermediate fix passed the two structure probes but still failed OutputBytes (native-delta-first.log). Final native tests: 8 passed (4 managed + 4 independent). WASI result: the same 8 tests passed. No additional blocking finding remains in this fixed scope.

## Additional exercised controls

The independent tests use actual production parse/lower/prepare before projection, except the deliberately source-less typed output budget fixture. Fourteen code payloads cover one/multiple/internal backticks, leading/trailing/both-side spaces, all-space payloads, punctuation, backslash, and Japanese written in the Rust probe using Unicode escapes. All are parsed by pulldown-cmark and compared to exact original Code payloads. Empty Article Body, empty sibling Sections, and adjacent Paragraphs are successful controls. Managed tests additionally cover the actual 00-contract.nepld against canonical Markdown event structure, literal punctuation, unsupported Ruby/Break/multiple flows/Text LF/nested sections/adjacent lists, repeated work/allocation stops, cancellation, and unchanged input.

CommonMark delimiter interpretation was checked against the official specification: https://spec.commonmark.org/0.31.2/#code-spans . Markdown event equality is scoped to this restricted compatibility view; it is not general Doc source/ID/language/position reconstruction.

## Static scope and limits

The public entry validates through prepare::inspect and rejects nonempty resolution requirements. The host entry uses the actual production parser/lower. The CLI uses bounded UTF-8 input, create_new output, and escaped provenance comment metadata. These CLI I/O properties were read, not independently executed. The README retains canonical Markdown and explicitly leaves legacy-anchor compatibility and canonical-source switching unfinished. The CI addition generates the view outside the site manifest directory; the inspected upload path is still only dist/doc-migration/, so the new view is not itself uploaded by that step. No CI execution, migration authorization, deployment, or full workspace regression is inferred here.

Final inspected production source: tools/src/doc/projection.rs SHA256 5ed4897ae6902f6f4b4c3a4fd547d8cab47fa3347511fa7c945468af26cc3fe4.
