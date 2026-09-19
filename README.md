# NEPL3

[![CI](https://github.com/neknaj/NEPL3/actions/workflows/ci.yml/badge.svg)](https://github.com/neknaj/NEPL3/actions/workflows/ci.yml)

NEPL3は、**多数の異なるDSLを、括弧なし前置記法の共通規律で多階層・再帰的に相互埋め込みする言語基盤**を開発するプロジェクトです。reader/tokenizer、構文、source位置、診断、editor支援の契約を共有し、言語を切り替えたときの視覚的・構文的な断絶を小さくします。各言語の意味論は各言語が所有します。

各headの子の数と読取り規則は、その位置までに確定したcontextとtoken自身から決めます。先行する宣言やimportによって後続・内側のcontextを更新でき、後続情報で既読の構文shapeは変更しません。構文が確定した後の名前解決・型付け・評価等は、各言語固有の規則で行えます。

これは基盤の設計原則です。現行の宣言body・登録済みcontextの契約は[Grammar仕様](doc/spec/04-grammar.md)、新schemaやreaderを後続へ導入する一般経路の提案は後述の統合設計草案を参照してください。

純粋な処理と明示的な入出力を重視し、深い埋め込みでも作用範囲と結果の由来を追跡できる構成を目指します。coreは`no_std + alloc`、I/Oはhost側に分け、Rustの型付きAPIとNDFによる言語中立の交換経路を用意します。実装済みの範囲と拡張提案は、以下の資料で区別しています。

## 現在の開発範囲

現在は**共通runtimeの実装段階**です。Grammar・Doc・Math・Circuitは現在の設計対象であり、追加できる言語の上限ではありません。[外部言語の追加契約](doc/spec/22-external-extensions.md)では、公式言語も公開契約を使うreference extensionとして扱います。

- Foundationのsource・schema・Origin・Budget、reader/engine、NDF codecを実装・検証中です。
- Grammar・Doc・Mathのcore、型付きmarkup、Doc HTML、MathML・TeX出力の実装があります。Circuit coreはまだworkspace memberに含まれていません。
- DocのHTML生成と数式表示、文書のDoc DSL移行を進めています。TEAによるWeb Playground、CLI、LSP、全言語の処理系は完成していません。

CI成功は全受入条件の達成を意味しません。検証範囲は[Foundation](doc/progress/foundation-runtime.md)、[Doc](doc/progress/doc-runtime.md)、[Math](doc/progress/math-runtime.md)の実装記録と、[実装状態](implementation-status.json)を参照してください。実際のworkspace構成は[Cargo.toml](Cargo.toml)が示します。

## 次の統合設計案

[統合設計草案](doc/decisions/multilanguage-hca.md)では、NEPLの世代ごとの目的、前方contextと関数適用、用途別producer契約、回路モデル、言語の独立性を整理しています。

| 提案する言語 | 意味領域 |
| --- | --- |
| NEPL3sentence | 単独parse/printできる構造化文章・Sentence/Inline |
| NEPL3a | 文章を対象syntaxへ付与する`annotate Sentence target` |
| NEPL3d | Sentenceを本文として利用する文書構造 |
| NEPL3c / NEPL3hdl | primitive・帰還・伝搬の回路と、同期RTLの責務分離 |
| NEPL3h | GHCへ接続する独立Haskell frontend |

**設計と実装完了は区別します。** NEPL3sentenceの独立core・LanguagePackage・literal/prefixのparse/check/printと、Doc本文readerへの接続は実装されています。Docの旧Sentence所有の除去、Aへの注釈移行、旧lexical commentの撤去、新C/HDL、GHC adapter等は未完了です。現行文法を草案の例へ読み替えないでください。[NEPL3h案](doc/decisions/nepl3h-ghc-frontend.md)も、NEPL3全体の評価器や必須マクロ言語を定義するものではありません。

## 読む・開発する

| 入口 | 内容 |
| --- | --- |
| [ドキュメント](doc/README.md) | 仕様と設計資料の読み順 |
| [開発手順](doc/development.md) | ツールチェーン、検査、CI/CD |
| [実装状態](implementation-status.json) | 実装タスクと受入試験の実行状態 |
| [タスク索引](tasks/README.md) | 依存順の実装作業 |
| [貢献方針](CONTRIBUTING.md) | 実装・独立レビューと変更の進め方 |

```sh
git clone https://github.com/neknaj/NEPL3.git
cd NEPL3
cargo run --locked -p nepl3-tools -- check
cargo run --locked -p nepl3-tools -- tasks --check
```

Rustの版は [rust-toolchain.toml](rust-toolchain.toml) で固定しています。crateの目標構成は [依存計画](design/dependencies.json)、実際のworkspace memberは [Cargo.toml](Cargo.toml) を参照してください。

上のコマンドはrepository契約とtask生成物の確認です。runtimeの試験は`cargo test --workspace --locked`、その他の必須検査とtarget別の実行手順は[開発手順](doc/development.md)を参照してください。

文書は[移行条件](doc/spec/16-doc-migration.md)を満たしたページからDoc DSLを正本にしています。[doc/canonical.json](doc/canonical.json)の登録ページは.nepldを編集し、生成Markdownを手編集しません。未登録ページはMarkdownを正本とします。ローカルHTML生成、静的サイト、GitHub Pages配布の手順・保証範囲も[開発手順](doc/development.md)に記載しています。文書表記は[執筆指針](doc/authoring.md)に従います。

設計識別子は `nepl3-design-2026-09-06-r4`。[foundationの訂正](doc/decisions/0005-foundation-runtime-contracts.md)、[Web・Doc移行の判断](doc/decisions/0003-web-tea-doc-migration.md)、[r2の契約訂正](doc/decisions/0002-design-contract-corrections.md)、未解消の課題は [独立レビュー](doc/review.md) に記録します。説明用の `.tmp/` はGit管理・CI・配布の対象外です。

ライセンスは [MIT](LICENSE) です。
