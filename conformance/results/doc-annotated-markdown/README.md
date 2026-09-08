# Annotated Doc Markdown checkpoint

Production: `2e8fafa8c07fdec95394fca04332c9a41342cd41` (PR83).
Root final checks: native 538 passed, 0 failed, 1 ignored; format, Clippy and repository checks passed. The recorded starting head plus source.patch is the final production source.
Independent review found decorated adjacent-code acceptance in 7b449ca. The original failure is retained; the fix and final Work charge were revalidated with native 7, WASI 6, legacy projection native 10, and 13 CLI cases at each recorded checkpoint. These counts are different scopes, not one acceptance total.

The new chapter 21 section was manually authored and independently reviewed: 47 sentences, 14 inline codes, 258 Ruby nodes; existing draft bytes retained. The isolated unchanged Section was wrapped in a test Article and rendered with production HTML. This is not whole-chapter rendering evidence.
Chapter 13 was generated from its unchanged draft. Final output bytes equal the earlier reviewed output. Independent content review compared 37 blocks, 674 Ruby nodes, 33 inline codes and 2 external URIs. This chapter contains no Anno nodes; Anno is covered by the separate implementation tests.

The eight old anchors were manually mapped and independently checked against fixed-commit GitHub HTML. GitHub Markdown API rendering retained their names and the external URIs. That API does not emit the blob page's automatic heading IDs; browser navigation, automatic-anchor collision freedom, accessibility and canonical cutover remain unverified. The independent view review retains a Mistune link parsing difference; do not infer compatibility with every Markdown parser.

Original payload bytes and failed probes are preserved. `fixture-paths.json` maps original malformed/encoded test inputs to opaque fixture filenames without changing bytes; resolve original manifest paths through that map. `payloads.json` lists stored bytes and hashes. Large generated adversarial inputs are identified and recreated by the independent scripts rather than committed. No task or required acceptance status was changed to passed. This is not completion of T21, T16, the static site, Pages, or the four-language runtime.

The initial CI failed API documentation because Rustdoc parsed the notation example as an intra-doc link. The notation was enclosed in code spans without changing runtime logic. Original CI failure and successful strict workspace rustdoc execution are preserved under integration/ci. Exact-head CI remains required before merge.
