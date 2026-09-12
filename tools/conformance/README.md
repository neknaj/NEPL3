# 実行証拠の収集

`runner.py` は宣言したargvを実行し、原stdout/stderr、終了結果、Git revision、
環境とhashを保存する。実行結果を判定するtestと、受入群を判定するRustの
`tools/src/evidence/` は別の責務である。レビュー本文を生成しない。

```sh
python -m tools.conformance.runner run conformance/specs/evidence-tools.json dist/evidence-tools
python -m tools.conformance.runner verify dist/evidence-tools
```

新規の出力先のみを使用する。sourceは先にcommitし、そのcommit/pathで参照する。
実行specとlogはdataとして保存し、runnerやrepositoryを結果配下にコピーしない。
再利用する回帰probeは通常のtestへ置く。例外的な外部入力や再構成できない資料には
保存理由を記す。独立レビューは対象commit・scope・指摘・未検証範囲を文章で残す。

このツールは信頼した開発commandの収集器でありsandboxではない。historicalな
結果directoryを実行先にする記述は拒否するが、任意プログラムの作用は解析しない。
source検査は開始時の未追跡ファイル検出とtracked HEAD/diffの前後比較に限る。
ignored入力、実行途中の一時的変更、toolchainそのものの再現性は保証しない。
外部依存はlock・command・環境の記録および個別のtarget runnerで扱う。

timeoutはunknownであり、後続commandを実行しない。子孫processの終了とlogの
最終byte列は保証しないので、timeout記録をimmutableな成功証拠として使わない。
verifyは記録内の矛盾・改変を検出する。manifestの作者や実行事実を暗号学的に
証明するものではない。CI記録・独立レビューと併せて確認する。

`inventory.py` は指定Git commit内の過去scriptを実行せず分類候補と重複を抽出する。
分類候補は人による内容監査の代替ではない。過去の証拠を再sealしない。
