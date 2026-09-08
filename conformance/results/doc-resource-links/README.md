# Explicit Doc file resources

Production 2a448b0 adds typed Page/File destinations and exact file bytes to the PageSet schema, native/portable resolver and HTML host export. Tests-only 67c48dc additionally exercises real file input containment and byte preservation. The original independent production review covers native/WASI, receiver forgery and identity changes, two actual CLI successes and 19 expected failures. Authoring checkpoint 721afd9 is independently compared against the changed specification.

Original manifests and payload bytes are retained. fixture-paths.json maps any BOM or invalid JSON proof input to a .fixture filename without changing bytes; restore the original names in scratch for replay. Normal repository checks are not weakened.

Root workspace results belong to 2a448b0. The actual unchanged foundation-runtime draft and its three original JSON references generate HTML and byte-identical copied files with default parse/lower and explicitly selected finite output limits. This is not a default-output-budget result, canonical Doc cutover, arbitrary-file rendering certification, browser verification or Pages deployment. The host records opaque copied file MIME; public serving policy remains separate.
