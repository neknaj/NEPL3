# 開発への参加

最初に [仕様の入口](doc/README.md)、[AGENTS.md](AGENTS.md)、[開発手順](doc/development.md) を読んでください。実装対象は [タスク定義](design/tasks.json) の依存順に選び、[実装状態](implementation-status.json) を確認します。

変更は具体的な問題と受入条件に結び付け、必要最小限の範囲にします。設計の誤りが見つかった場合は、根拠を示して仕様・schema・文法・例・conformanceを整合させ、`doc/decisions/` に理由を記録します。ChatGPTが生成した設計も検証対象です。

agentによる開発では、メインagentが統括し、実装subagentと独立レビューsubagentを分けます。レビュー担当は契約と実際の差分・試験根拠を確認し、メインagentが指摘の解消と再検査を確認します。

PRには変更後の動作、関連タスク・受入条件、実行した検査、未実行の範囲を記載してください。`doc/development.md` の検査とdiff確認を行います。タスク本文の直接編集、未実装の成功stub、golden更新だけによる正しさの主張は避けます。`.tmp/` や秘密情報をcommitしません。

提供されるコードと文書は、このリポジトリの [MIT License](LICENSE) の下で扱います。
