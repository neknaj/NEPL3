# NEPL3 ドキュメント

正式な仕様・開発情報の入口です。会話やローカルの `.tmp/` を参照しなくても、ここから実装契約と現在の状態を確認できます。

Docで例や解説を書く際は、[文書の執筆指針](authoring.md)のsentence literal・前置構築・parallel・Ruby/Annoの使い分けに従ってください。

## 読み順

1. [対象範囲と共通契約](spec/00-contract.md)、[アーキテクチャ](spec/01-architecture.md)
2. [共通基盤](spec/02-foundation.md)、[reader](spec/03-reader.md)、[Grammar](spec/04-grammar.md)
3. [Doc](spec/05-document.md)、[Math](spec/06-math.md)、[Circuit](spec/07-circuit.md)
4. [エディタ支援](spec/08-editor.md)、[交換契約](spec/09-portability.md)、[統合](spec/10-integration.md)
5. [受入条件](spec/11-conformance.md)、[モデル不変条件](spec/12-model-invariants.md)、[再現性](spec/13-reproducibility.md)
6. [開発手順](development.md)、[タスク索引](../tasks/README.md)、[実装状態](../implementation-status.json)
7. [初期設計の独立レビュー](review.md)、[r2の契約訂正](decisions/0002-design-contract-corrections.md)
8. [Web UI・TEA](spec/14-web-ui.md)、[静的サイト・Pages](spec/15-site.md)、[Doc DSL移行](spec/16-doc-migration.md)、[r3の判断](decisions/0003-web-tea-doc-migration.md)
9. [Pages復旧と早期Doc inventoryの判断](decisions/0004-pages-recovery-doc-inventory.md)、[文書inventoryとgap audit](doc-inventory.md)
10. [r4 foundation契約](decisions/0005-foundation-runtime-contracts.md)、[実装・検証の進捗](progress/foundation-runtime.md)
11. [Doc runtime の実装範囲と残り](progress/doc-runtime.md)
12. [Math runtime の実装範囲と残り](progress/math-runtime.md)
13. [Doc・MathのHTML生成と数式表示](spec/17-math-html.md)

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
| `languages/*/syntax.neplg` | 4言語の文法source |
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
