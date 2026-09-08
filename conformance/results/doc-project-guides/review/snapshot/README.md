# NEPL3

[![CI](https://github.com/neknaj/NEPL3/actions/workflows/ci.yml/badge.svg)](https://github.com/neknaj/NEPL3/actions/workflows/ci.yml)

Grammar・Doc・Math・Circuitの4言語を、共通の構文・位置・診断・操作契約で接続するプロジェクトです。言語の意味モデルと出力backend、言語間adapterを分離し、Rustの型付きAPIと他実装向けの交換経路を設計しています。

現在は**共通runtimeの実装段階**です。`no_std` のcoreとNDF codecを実装し、source・値・schema・Origin・予算の検査とnative/WASIでの試験を進めています。4言語の処理系、CLI、LSP、Webアプリは完成していません。CI成功から全受入条件の達成を推定しません。[実装記録](doc/progress/foundation-runtime.md) に検査対象と残る範囲を記録します。

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

Rustの版は [rust-toolchain.toml](rust-toolchain.toml) で固定しています。20 crateは目標構成であり、実際のworkspace memberは [Cargo.toml](Cargo.toml) を参照してください。

設計識別子は `nepl3-design-2026-09-06-r4`。[foundationの訂正](doc/decisions/0005-foundation-runtime-contracts.md)、[Web・Doc移行の判断](doc/decisions/0003-web-tea-doc-migration.md)、[r2の契約訂正](doc/decisions/0002-design-contract-corrections.md)、未解消の課題は [独立レビュー](doc/review.md) に記録します。説明用の `.tmp/` はGit管理・CI・配布の対象外です。

ライセンスは [MIT](LICENSE) です。
