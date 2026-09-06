# 実装指示

## 規範

`doc/spec/` と `design/forms.json` を実装契約とする。既存の会話にある説明用の文法へ勝手に戻さない。設計の誤りが発覚した場合は、仕様・schema・全影響箇所・conformanceを同じ変更で修正し、変更理由を記録する。実装困難な部分を省略して完了と報告してはならない。

## コード品質

全coreは `#![no_std]` と `extern crate alloc`。プラットフォームI/O、時計、環境変数、ネットワーク、stdout/stderrはappsまたは開発用toolsだけで扱う。通常の失敗は型付きResult、未存在はOption。入力によるpanic、unwrap、todo、unimplemented、黙殺fallbackは禁止。内部不変条件のassertも外部入力の検査後に限る。

core内の公開データはstruct/enumで表す。schemaの動的境界を除いて、文字列キーの万能辞書や `Any` を使わない。外部境界では必ずschema validationを行う。coreの標準出力へのログは禁止。診断・eventを返り値として返す。

`design/dependencies.json` にないworkspace依存を追加する前に依存仕様を修正し、DAG・no_std・責務を再検査する。doc/math/circuit core同士の直接依存は禁止。production側からtools/appsへの依存は禁止。ファイル名へ擬似的な階層を `_` で詰めず、ディレクトリで責務を表す。

## 完成条件

各タスクに記載した失敗系・境界系・roundtrip・cross-target・portable-provider比較を実行する。goldenの更新を正しさの代わりにしない。期待値の根拠をテストに記述する。未実行を成功として報告しない。

## 作業の順序

`design/tasks.json` の依存順に実装する。段階ごとにbuildできることと、最終仕様を縮小することは別である。後段の未実装を成功Resultやダミー成果物で覆わない。実装状態は `implementation-status.json` で仕様状態と分離する。

Rust以外によるcore実装への置換では、同じ操作schemaとconformanceを使用する。公開契約をRust型のABI、serdeのデフォルトenum表現、ネイティブpointerに依存させない。

## このリポジトリでの作業

- テキストはUTF-8。PowerShellで読み書きする場合はEncoding UTF8を明示する。通常ファイルはLF、source位置試験のfixtureは元byte列を維持する。
- ユーザー指示を方針・契約に照らして検証し、問題があれば根拠を示す。ライブラリ仕様が曖昧ならWeb検索で公式資料を確認する。
- `.tmp/` は説明用のローカル資料でありGit管理・ビルド・CI・配布へ含めない。正式な入口は `doc/README.md`。
- 現在はリポジトリ基盤の整備段階。19 crateは目標構成で、実装済みmemberはCargo.tomlが示す。空のcrateや成功stubを揃えない。
- 実装状態の正本は `implementation-status.json`、タスク定義の正本は `design/tasks.json`。タスク本文・索引は `cargo run --locked -p nepl3-tools -- tasks --write` で生成する。
- 変更後は `doc/development.md` の検査を実行し、diffを確認する。未実行の受入試験をCI成功から推定しない。
- `tools` は開発host用。JSONなどの動的入力境界でのみ動的な値を使い、言語の意味モデルの代用にしない。
- Web UIは純粋TEAのnepl3-ui-coreとhost adapterを分け、UI coreからsuite・DOM・Worker・I/Oを実行しない。R006/R009を解消してから閉じたUI/Worker schemaを実装する。
- 最終文書はNEPL3 Doc DSLへ移行する。現在はMarkdownを正本とし、T21の表現gap監査・意味同等性・リンク/安定ID・bootstrap検査後にページ単位で切り替える。AGENTS等の必要なMarkdownはDoc正本からの生成projectionを許すが二重手書き保守は禁止。
- 受入群と必須targetはdesign/acceptance.jsonを正本とし、T16は全required群を動的に要求する。CI成功や証拠pathの存在だけでruntime/Web/文書移行をpassedへ変更しない。

## 実装・独立レビュー・統括の分担

- 開発は、実装するsubagentと、その実装を独立にレビューする別のsubagentを用いて行う。
- メインagentは統括を担当する。作業範囲と担当ファイル、依存関係、受入条件を定め、担当間を調整し、レビュー結果と実行証拠を確認して統合・報告する。
- 実装担当の自己確認だけで独立レビューを代替しない。レビュー担当は元の要求・仕様・変更差分・試験を自分で確認し、設計上の誤りと検証不足も指摘する。
- 指摘の修正は実装担当へ戻し、必要な再レビュー・再検証を経てから完了とする。未解決事項や未実行の検査は統括agentが明示する。
- ChatGPT作成の設計や過去の検証記録を無条件に採用しない。各契約を本文・schema・文法・例・受入条件および必要な公式資料と照合し、訂正理由と影響範囲を残す。
