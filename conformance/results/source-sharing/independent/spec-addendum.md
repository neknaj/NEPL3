# Foundation specification addendum

Read the two added storage/accounting paragraphs in doc/spec/02-foundation.md from root source-validation. SHA256 c081bf6da68338f41aaeeb3dc83a9b67cd610409c0b55177289ce06882a11481.

Storage privacy, wire full-content checking, immutable editing, retained full-text Work and no admission exemption all match the fixed code. Suggested one wording qualification: line 15 should explicitly limit identity/URI/slot-only clone Allocation accounting to pointer-atomic shared-storage targets, and mention fallback targets also charge copied text. Line 13 already states fallback owns String; implementation is correct. This is wording clarification, not a runtime finding. Root notified; no production edit.

Root runs the isolated full workspace gate from base 3da plus six-file overlay; independent review does not claim that separate run as independently executed. Independent tests remain the bounded 30 native / 29 WASI reported in record.md.
