# Independent restricted Markdown projection CLI review

Scope: tools/src/doc/projection.rs from_source()/write(), tools/src/main.rs command, README/development command recipes, CI output placement/upload. Typed Markdown writer semantics are reviewed separately by infrastructure. No live edits by this reviewer. Frozen initial dirty source: sources.json (2157 files). Final documentation/workflow bytes: final-static.json. Final host function text is unchanged from the tested snapshot.

No blocking finding in this host scope. Isolated native Windows build succeeded. cli.py exercised 16 actual CLI cases, and a seventeenth same-input/output check preserved the original source. Successful cases include ordinary input, valid Unicode path/content, CRLF source, metadata path containing -- and &, and the actual 00-contract.nepld migration candidate. Generated source SHA-256 matches the actual raw UTF-8 bytes read, including CRLF. Renderer marker is nepl3-tools.markdown/1.

For the --& path fixture, the metadata was parsed as exactly one HTML comment, with no elements. Static review confirms &, <, >, and - are escaped in that order so a source path cannot terminate the comment. Control characters and paths over 4096 UTF-8 bytes are rejected before file access/output creation. Windows does not permit literal < or > in normal filenames, so an actual Unix filename containing --> was not executed here; the full replace-chain was inspected directly.

Failure cases: existing output, absent output parent, invalid or truncated UTF-8, input over 10000000 bytes, trailing source tokens, malformed source, unsupported annotation, overlong/control-containing input path, missing source. Validation/input failures produced no output file. Existing and same-path input/output files retained their bytes. write() finishes parsing/lowering/projection before create_new; create_new also protects against a file appearing after exists(). Errors during actual disk writing can still leave partial files; no filesystem transaction guarantee is claimed.

from_source() checks the input-size cap and uses real checked parsing, bundle validation, lower::document and typed projection with separately bounded operation budgets. No fallback or plain-text substitute occurs on unsupported forms. This host test does not re-prove the writer's full shape/whitespace contracts or claim one shared end-to-end budget.

Documentation issue corrected by root: the clean-checkout recipes initially lacked creation of dist. Final README and development recipes contain mkdir -p dist before generation, including the standalone projection example. Root remains responsible for all source changes.

CI inspection: projection generation is inside the common native step after mkdir -p dist. Its output is dist/00-contract.view.md, outside dist/doc-migration/, so no unregistered file is added to the site's manifest scope. Ubuntu uploads it as a separate NEPL3-doc-markdown-${github.sha} artifact using the existing pinned upload action and if-no-files-found:error. Site artifact path remains dist/doc-migration/. No Pages deploy or permissions expansion occurs. Changed remote CI has not been run by this reviewer.

No human meaning approval, legacy ID/anchor compatibility, canonical-source switch, general roundtrip, site deployment, or T21 completion is claimed.
