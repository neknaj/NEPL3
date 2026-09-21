# NEPL3 ドキュメント

正式な仕様・開発情報の入口です。会話やローカルの `.tmp/` を参照しなくても、ここから実装契約と現在の状態を確認できます。

[正本registry](canonical.json)に登録したページはnepldを編集し、Markdownは生成物として扱います。
[再現性の仕様](spec/13-reproducibility.nepld)はこの方式へ移行し、既存のMarkdownのURLも生成projectionで維持します。
未登録ページは引き続きMarkdownが正本です。`migration/authored/` の原稿が存在するだけでは移行済みとしません。

Docで例や解説を書く際は、[文書の執筆指針](authoring.md)のsentence literal・前置構築・parallel・Ruby/Annoの使い分けに従ってください。

## 目的から探す

| したいこと | 最初に読む資料 | 確認できること |
| --- | --- | --- |
| NEPL3を理解する | [README](../README.md)、[共通契約](spec/00-contract.md)、[アーキテクチャ](spec/01-architecture.md) | 多階層の言語埋め込みと、基盤・各言語の所有境界 |
| 実際に試す・小さな言語を追加する | [外部Hello言語](../conformance/extensions/hello/README.md)、[公開拡張契約](spec/22-external-extensions.md) | 公開APIだけで独立言語を登録・解析し、入力を変えて結果を確認する |
| 構文・交換契約を調べる | [Foundation](spec/02-foundation.md)、[reader](spec/03-reader.md)、[Grammar](spec/04-grammar.md)、[交換](spec/09-portability.md)、[統合](spec/10-integration.md) | 入出力・shape・source・失敗条件。schema正本は下表 |
| 文書・数式を扱う | [Doc](spec/05-document.md)、[Math](spec/06-math.md)、[Sentence/annotation](spec/23-sentence-annotation.md)、[執筆指針](authoring.md) | 意味モデルと文章表記。Sentenceの契約とconsumer移行状態は区別する |
| HTML・公開の境界を調べる | [数式表示](spec/17-math-html.md)、[markup](spec/19-html-fragment.md)、[Doc HTML](spec/20-doc-html.md)、[ページ参照](spec/21-doc-pages.md)、[site](spec/15-site.md) | 各層の独立した保証。HTMLは正式文書のprojection |
| 開発・検査する | [開発手順](development.md)、[受入条件](spec/11-conformance.md)、[モデル不変条件](spec/12-model-invariants.md)、[再現性](spec/13-reproducibility.md) | 変更に対応する検査と、正式受入との区別 |
| 編集先・現在状態を確認する | [canonical registry](canonical.json)、[実装状態](../implementation-status.json)、[タスク索引](../tasks/README.md) | 正本、実行済み範囲、依存する成果物と残件 |

個別の設計対象は [Circuit](spec/07-circuit.md)、[editor](spec/08-editor.md)、[Web UI](spec/14-web-ui.md) を参照してください。文書移行の条件は [第16章](spec/16-doc-migration.md) にあります。仕様全章や過去レビューの通読は、最小例の実行の前提ではありません。

## 設計理由と実装記録

[統合設計](decisions/multilanguage-hca.md) と [NEPL3h案](decisions/nepl3h-ghc-frontend.md) は設計の意図を説明します。実装契約は仕様・schema、実装完了は状態正本で確認します。[Pages情報設計](decisions/pages-information-architecture.md) も採用状態と実装状態を分けています。

[Foundation](progress/foundation-runtime.md)、[Doc](progress/doc-runtime.md)、[Math](progress/math-runtime.md) の記録は能力と検証範囲を説明します。日時付きの結果を現在の全体状態と同一視しません。[初期レビュー](review.md)、[r2訂正](decisions/0002-design-contract-corrections.md)、[Web/Doc移行の判断](decisions/0003-web-tea-doc-migration.md)、[Pages復旧の判断](decisions/0004-pages-recovery-doc-inventory.md)、[foundation契約の判断](decisions/0005-foundation-runtime-contracts.md)、[inventory監査](doc-inventory.md) は経緯や根拠を調べる入口です。

## 正本と派生資料

| 場所 | 責務 |
| --- | --- |
| `doc/spec/` | 構文・意味・操作・失敗条件・受入条件の文章仕様 |
| [design/forms.json](../design/forms.json) | formの構文signature |
| [design/dependencies.json](../design/dependencies.json) | crate責務と依存の許可集合 |
| [design/tasks.json](../design/tasks.json) | タスクID、依存、成果物、受入条件の正本 |
| [interfaces/model.json](../interfaces/model.json)、[contracts.json](../interfaces/contracts.json) | 言語中立の意味モデル・操作schema |
| [interfaces/foundation.json](../interfaces/foundation.json) | contractsから生成しproduction registryで検査する共通package descriptor |
| [interfaces/doc.json](../interfaces/doc.json)、[doc-reader.json](../interfaces/doc-reader.json) | Doc arena値と、domain coreから分離したsentence reader adapterの実schema |
| [interfaces/math.json](../interfaces/math.json) | Mathの表記を保持するarenaとsource閉包の実schema。構造proofと式の評価を分離する |
| `languages/*/syntax.neplg` | 各LanguagePackageの文法source |
| [conformance/cases.json](../conformance/cases.json)、[examples/](../examples/) | 受入条件と検証入力 |
| [implementation-status.json](../implementation-status.json) | 実装・試験の実行状態。仕様定義と分離する |
| [design/acceptance.json](../design/acceptance.json) | 必須受入群と必須targetのcatalog |
| [design/ui.json](../design/ui.json)、[site.json](../design/site.json) | UI/siteの責務・配置計画。実行可能schemaではない |
| [tasks/](../tasks/) | タスク正本から生成する読み物 |
| [history/](history/README.md) | 取り込み元の由来と当時の検査報告 |

仕様間の矛盾は実装の都合で読み替えず、影響するschema・文法・例・受入条件を合わせて修正します。[設計判断](decisions/0001-repository-foundation.md) に理由と検証範囲を残します。文法の詳細表は [Grammar](spec/grammar-signatures.md)、[Doc](spec/doc-signatures.md)、[Math](spec/math-signatures.md)、[Circuit](spec/circuit-signatures.md) を参照してください。

設計一式は実装の完成証拠ではありません。取り込み元の検査報告をruntimeの受入試験結果へ転記しません。外部資料は [参考文献](spec/references.md) にまとめています。

HTMLの先行利用は [T22〜T25の実装段階](spec/18-html-delivery.md) に従います。既存のDoc処理を再利用し、Doc単独HTML、数式HTML、Web preview/exportへ接続します。全体の最終範囲は維持します。

型付きHTML fragmentの入力・検査・serializerは [19章](spec/19-html-fragment.md) と [markup schema](../interfaces/markup.json) を参照してください。

- [Doc HTML変換の契約](spec/20-doc-html.md) — local準備・表示・portable再検査と未解決資源の扱い。
- [Docページ集合とリンク解決](spec/21-doc-pages.md) — 明示された文書と配置を索引化し、ページ・見出しへの参照を検査する。
