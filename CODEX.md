# 実装作業の入口

まず [AGENTS.md](AGENTS.md)、[文書索引](doc/README.md)、[開発手順](doc/development.md) を読む。

Grammar・Doc・Math・Circuitと共有基盤の目標仕様は `nepl3-design-2026-09-06-r2`。
`design/tasks.json` の依存順でT01から実装する。今回のリポジトリ基盤整備はT01やT16の完成ではない。
現状は [implementation-status.json](implementation-status.json) を参照する。

実装では型付きRust APIと言語中立schema・操作契約を並行して整備する。
全constructorのparse/lower/check/print/render/wire/editor対応を追跡し、埋め込みから評価を自動実行しない。
設計に問題があれば仕様・schema・例・受入条件を同時に修正し、`doc/decisions/` に理由を残す。
task.acceptanceは試験群へのcoverage参照である。T01〜T15は当該deliverableとscope付き検証証拠、依存タスクの完了、関連設計blockerの解消で完了を判定する。参照群に含まれる後続機能が未実装なら、その群をpassedにしない。
最終タスクT16と処理系全体の完了には [37群の受入条件](doc/spec/11-conformance.md) すべての実行証拠が必要。
