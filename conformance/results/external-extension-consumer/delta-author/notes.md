# Manual authored follow-up: external extensions

Canonical input commit: 75e73a79022e238ac7355c37588c37b910759398. Scope is only its additions to doc/README.md, doc/spec/01-architecture.md and doc/spec/11-conformance.md.

- doc/README.md -> authored/guide/README.nepld: append one ordered reading entry, exact target spec/22-external-extensions.md and original label, using a prefix Sentence with typed Link/Concat and Kanji-only Ruby.
- spec01 -> authored/01-architecture.nepld: add the two source sentences immediately after the existing independent-core paragraph. First Sentence uses prefix for the original relative link and exact surrounding ASCII spaces. Second is an ordinary sentence literal. Monorepo retention, foundation-source nonmutation, independent workspace public API, future distribution/provider/compatibility testing all retained.
- spec11 -> authored/11-conformance.nepld: add X01 and X02 as separate list items, each containing three sentence literals. Requirements, limitations and 22-chapter references retained exactly after removing readings. No new translations/Anno or parallel variants invented where the source supplies none.

Author self-check: X01/X02 plain text equals canonical source exactly; all three before/after diffs contain insertion only (no deletion/replacement); UTF-8 LF, no BOM; git diff --check succeeds. Existing bodies/code/link targets are unchanged. These checks do not replace the required independent content review.

Runtime self-check: pinned production binary from 2e8fafa, identified by SHA in runtime.json, executes all three actual inputs through parse/lower and reports NeedsResolution. No output directories created. Existing and newly added relative links were not removed or replaced with dummy registrations. This is not HTML success, no canonical cutover, no deployment or acceptance completion.

Shared worktree: new22 manuscript belongs to Carson, code/contracts to root/H. Only the listed three drafts were edited here. Changes intentionally remain uncommitted for root's shared-worktree integration; no unrelated files staged. Independent review pending.
