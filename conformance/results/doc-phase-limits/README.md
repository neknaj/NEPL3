# Doc parse and lower phase limits

Production da40ffc configures finite parse/lower budgets before each operation, binds actual parse limits into Profile, and preserves the output-only execution identity while adding a separate phase identity. Defaults are unchanged; this API does not resume a preconsumed parse/lower budget. Public core contracts and no_std boundaries remain unchanged.

The original author, independent authoring, production and tests-only fc10fb7 review manifests and payloads are retained byte-for-byte. Native/WASI production tests and independent CLI negative cases passed within the recorded scope. Root workspace verification belongs to da40ffc; subsequent authored content and tests-only additions have separate reviews. Browser execution, large-document completion, canonical Doc migration and Pages deployment are not established by these records. The specification and authored draft remain separate until migration acceptance.

Invalid JSON receiver fixtures retain their original bytes with a `.fixture` suffix. `production-review/fixture-paths.json` maps the original reviewer manifest paths to stored paths; duplicate-key fixtures are intentionally invalid and are not repository metadata. The original reviewer manifest is unchanged.

Two original reviewer probe sources contain a UTF-8 BOM. Their bytes are likewise preserved as mapped `.fixture` payloads, rather than normalized into new proof sources. Restore mapped names in a scratch directory when replaying the recorded commands. Repository metadata/source checks remain enabled without exceptions.
