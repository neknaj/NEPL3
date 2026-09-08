# Doc parse and lower phase limits

Production da40ffc configures finite parse/lower budgets before each operation, binds actual parse limits into Profile, and preserves the output-only execution identity while adding a separate phase identity. Defaults are unchanged; this API does not resume a preconsumed parse/lower budget. Public core contracts and no_std boundaries remain unchanged.

The original author, independent authoring, production and tests-only fc10fb7 review manifests and payloads are retained byte-for-byte. Native/WASI production tests and independent CLI negative cases passed within the recorded scope. Root workspace verification belongs to da40ffc; subsequent authored content and tests-only additions have separate reviews. Browser execution, large-document completion, canonical Doc migration and Pages deployment are not established by these records. The specification and authored draft remain separate until migration acceptance.
