# 根拠と参照資料

取得日: 2026-09-06（Asia/Tokyo）。外部仕様の説明とNEPL3自身の新規設計を区別する。

## 本人の要件

本会話の最新指示: 4言語と共通基盤をRustで実装、後のNEPL3言語への置換、依存関係、sentence literal/prefix両経路、sentence単位parallel、汎用kind、editor支援、暫定設計を認めない。

元の設計資料は、ChatGPTのLibraryに保存された2026-09-05の方針・構文基盤・意味論・API・検証計画・apply追補を参照したと記録している。
それらのLibrary本文はこのリポジトリへ提供されていないため、本整備では直接確認していない。
本リポジトリの契約は提供された会話と74ファイルの設計資料に基づく。来歴は `doc/history/README.md` を参照。

## 公開資料

- 本人の設計指針: https://zenn.dev/bem130/articles/1b352797de94e7
- Cargo feature設計: https://doc.rust-lang.org/cargo/reference/features.html
- Rust edition 2024 / resolver 3: https://doc.rust-lang.org/edition-guide/rust-2024/cargo-resolver.html
- wasm32-wasip2 target: https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip2.html
- Langium grammar: https://langium.org/docs/reference/grammar-language/
- Langium features: https://langium.org/docs/features/
- 開発者による事例: https://www.typefox.io/blog/langium-1.0-a-mature-language-toolkit/
- Racket syntax-spec: https://docs.racket-lang.org/syntax-spec-v3/Specifying_languages.html
- LSP 3.17の参照契約: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/
- MathML Core: https://www.w3.org/TR/mathml-core/
- OpenType MATH（独自layout backendを追加する際の境界の参考）: https://learn.microsoft.com/en-us/typography/opentype/doc/spec/math
- HTML ruby: https://html.spec.whatwg.org/multipage/text-level-semantics.html
- CBOR: https://www.rfc-editor.org/rfc/rfc8949.html
- WITの言語中立interface（本仕様のNDFと同一のABIではない）: https://component-model.bytecodealliance.org/design/wit.html
- num-bigint no_std: https://docs.rs/num-bigint
- num-rational: https://crates.io/crates/num-rational

- Language tags RFC 5646: https://www.rfc-editor.org/rfc/rfc5646.html
- Unicode 16.0.0: https://www.unicode.org/versions/Unicode16.0.0/

設計のGrammar/Doc/Math/Circuitの具体的な表層文法、NDF profile、crate構成は、この成果物で定めた設計であり、これらの外部資料が採用を保証するものではない。
