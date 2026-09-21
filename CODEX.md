# 実装作業の入口

このファイルは案内です。独立した開発規約は置かず、[AGENTS.md](AGENTS.md) を規約の入口とします。

- 仕様・執筆・実装資料は [文書索引](doc/README.md)、検査とPR統合手順は [開発手順](doc/development.md) を参照します。
- 着手範囲は今回のユーザー指示、[現在の実装状態](implementation-status.json)、[タスク定義と依存関係](design/tasks.json) を照合し、利用する前段成果物が成立した範囲から選びます。
- 文書の編集対象は [正本登録](doc/canonical.json) で確認します。登録済みページは登録されたNEPL3d sourceを編集し、Markdown projectionを直接編集しません。未移行ページはMarkdown正本を編集します。ページ単位の正本切替条件はAGENTS.mdに従います。
- 完成判定は [登録された受入条件](design/acceptance.json) と各タスクの条件に従います。個別試験・CI成功を処理系全体の完成へ読み替えません。
