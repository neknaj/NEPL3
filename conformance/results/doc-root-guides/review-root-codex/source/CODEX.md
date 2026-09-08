# 実装作業の入口

まず [AGENTS.md](AGENTS.md)、[文書索引](doc/README.md)、[開発手順](doc/development.md) を読む。

Grammar・Doc・Math・Circuitと共有基盤の目標仕様は `nepl3-design-2026-09-06-r4`。
`design/tasks.json` の依存順でT01から実装する。今回のリポジトリ基盤整備はT01やT16の完成ではない。
現状は [implementation-status.json](implementation-status.json) を参照する。

実装・試験・指摘修正はメインagentが行い、subagentは独立レビューを担当する。ユーザーが明示許可したDoc文書の書き直しに限り、subagentによる並行執筆と、執筆者とは別のsubagentによる独立レビューを行う。通常の処理系実装はメインagentが担当する。未レビューの変更と検証済みの変更を区別し、専用branchでこまめにcommit・pushする。

T17〜T20はTEA UI・4言語Playground・静的文書/例サイト・Pages受入、T21は正式文書のDoc DSL移行。番号順ではなく依存DAGで進め、T16はこれらを含む最終受入とする。現在Markdownの正本を維持し、Doc表現gap、形式依存checker、安定URLと意味同等性を解消する前に変換しない。

実装では型付きRust APIと言語中立schema・操作契約を並行して整備する。
全constructorのparse/lower/check/print/render/wire/editor対応を追跡し、埋め込みから評価を自動実行しない。
設計に問題があれば仕様・schema・例・受入条件を同時に修正し、`doc/decisions/` に理由を残す。
task.acceptanceは試験群へのcoverage参照である。T16以外のタスクは当該deliverableとscope付き検証証拠、依存タスクの完了、関連設計blockerの解消で完了を判定する。参照群に含まれる後続機能が未実装なら、その群をpassedにしない。
最終タスクT16と処理系全体の完了には [登録された受入条件](doc/spec/11-conformance.md) すべての実行証拠が必要。
