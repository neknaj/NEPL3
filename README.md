# NEPL3

[![CI](https://github.com/neknaj/NEPL3/actions/workflows/ci.yml/badge.svg)](https://github.com/neknaj/NEPL3/actions/workflows/ci.yml)

Grammar・Doc・Math・Circuitの4言語を、共通の構文・位置・診断・操作契約で接続するプロジェクトです。言語の意味モデルと出力backend、言語間adapterを分離し、Rustの型付きAPIと他実装向けの交換経路を設計しています。

現在は**リポジトリ基盤の整備段階**です。設計仕様、文法source、schema、例、受入条件と開発用検査ツールを管理しています。4言語のruntime、CLI、LSP、WASI・ブラウザ向け実装は未実装です。CIの成功はリポジトリ検査の成功を示し、言語実装の完成を示しません。

最終成果物にはTEAによる4言語Web Playground、文書・例の静的サイトとGitHub Pages配布を含めます。現在のMarkdown文書は、表現能力と変換の受入を満たしてからNEPL3 Doc DSLへ移す[必須計画](doc/spec/16-doc-migration.md)としています。Web公開・文書変換は未実装です。

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

Rustの版は [rust-toolchain.toml](rust-toolchain.toml) で固定しています。19 crateは目標構成であり、実際のworkspace memberは [Cargo.toml](Cargo.toml) を参照してください。

設計識別子は `nepl3-design-2026-09-06-r3`。直近の追加範囲は [Web・Doc移行の判断](doc/decisions/0003-web-tea-doc-migration.md)、以前の訂正は [r2の契約訂正](doc/decisions/0002-design-contract-corrections.md)、未解消の課題は [独立レビュー](doc/review.md) に記録します。説明用の `.tmp/` はGit管理・CI・配布の対象外です。

ライセンスは [MIT](LICENSE) です。
