# 取り込み元の記録

設計識別子 `nepl3-design-2026-09-06-r1` の説明用資料から、仕様・schema・文法・例・タスクを正式な配置へ取り込みました。ローカルの `.tmp/` は管理対象ではなく、開発・CI・配布から参照しません。

- [source-manifest.json](source-manifest.json) は元の設計パッケージのmanifestです。pathとdigestは**取り込み元**のものです。現在のファイル配置や修正後のdigestを保証しません。
- [design-validation.json](design-validation.json) は設計作成時に行ったと報告された検査です。由来を保持するため元byte列を保存しています。Rust処理系の試験結果ではなく、このリポジトリで再実行した結果でもありません。

元の独立検査器の実行可能なsourceは設計パッケージに含まれていません。外部の会話・ライブラリ資料も、本文や実行環境をここに完全保存しているわけではありません。このため、報告済みの結果だけを根拠に受入試験をpassedへ変更しません。現行の検査は開発toolsとCIで実行し、runtimeの状態は [implementation-status.json](../../implementation-status.json) で別に管理します。

正式資料の移設、契約と状態の分離などの訂正理由は [設計判断](../decisions/0001-repository-foundation.md) に記録します。
