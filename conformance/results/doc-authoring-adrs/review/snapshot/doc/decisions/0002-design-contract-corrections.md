# 0002: 独立レビューによる契約の訂正

- 日付: 2026-09-06
- 状態: 採用
- 取り込み元: `nepl3-design-2026-09-06-r1`
- 現行設計: `nepl3-design-2026-09-06-r2`

[独立レビュー](../review.md) で、交換形式の順序、Circuitの操作前提、Codeの意味値保持、Numberのprint roundtrip、MathMLの長さ、停止理由、XML文字列出力の矛盾を確認しました。

| 指摘 | 訂正 |
| --- | --- |
| R001 | recordとvariant payloadのfieldを順序付きarrayに変更。variantはNDFの名前で識別し、map順に意味を持たせない。 |
| R002 | initialの入力をPreparedNetlistに統一。NORは4種のnodeと2種のsink配列で表す。 |
| R003 | Doc自身のCodeもForeignSyntaxを保持。表示のためにguestのlowerや意味検査を要求しない。 |
| R004 | Numberは有限十進の有理数に限定。任意有理数からの式構築helperが必要に応じFracを作り、著者のFracは保存する。 |
| R005 | mspaceの長さを非負のcanonical十進 + emに限定。SVG座標のDecimalと区別する。 |
| R007 | sourceBytes超過の型付き停止理由SourceLimitを追加する。 |
| R008 | Markupに出力可能な文字集合を設け、XMLで表せない文字を拒否。>とCR、属性内のTAB/LF/CRのescapeを固定する。 |
| R011 | task.acceptanceをcoverage参照と定義。前段タスクは自身のscope付き証拠で判定し、群全体のpassedとT16の全37群合格を分離する。 |

対応する文章仕様・意味モデル・属性schema・conformance入力・受入条件を合わせて修正しました。source文法はNumberの有限十進とForeignSyntaxの構文をすでに表現しており、formのarity変更はありません。source r1の履歴は書き換えません。現行metadataのdesign_revisionと実装タスクのdesignはr2を示します。

R006（操作・protocol schemaの閉包不足）とR009（解決済みProfileの生成）は、関連タスクの完成を阻む設計課題として記録しています。現時点のinterfaceを外部実装がそのまま実行できる完全なschemaとして扱いません。各課題の対象タスクと状態は [design/review.json](../../design/review.json) を参照してください。

conformanceへ追加した期待値は仕様入力で、runtimeによる検証済み結果ではありません。実装後にfield順/digest、print roundtrip、Code、回路、停止、Markupの成功系・失敗系を実行します。今回の訂正はレビュー結果と公開規格に基づきます。根拠は [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259.html#section-4)、[MathML Core](https://www.w3.org/TR/mathml-core/#space-mspace)、[XML 1.0](https://www.w3.org/TR/xml/#charsets) です。

段階完了の訂正は、T01が参照するE03/E04/A04に後続エディタ等の責務も含まれることが根拠です。参照群全体の合格を前段完了の条件にすると依存順で進められません。各タスクの実装責務と全体の受入条件を分け、前段をcompleteにしても未実行の群はnot-runを維持します。
