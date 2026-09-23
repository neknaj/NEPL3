# 実行証拠の収集

`runner.py` は宣言したargvを実行し、原stdout/stderr、終了結果、Git revision、
環境とhashを保存する。実行結果を判定するtestと、受入群を判定するRustの
`tools/src/evidence/` は別の責務である。レビュー本文を生成しない。

これは開発commandの実行記録収集器であり、仕様適合試験そのものではない。
適合ケース・期待値・targetは `conformance/`、独立レビューはPR、公開journalは
publisherへ分離する。保存とhash照合だけで適合・公開・LKGへ昇格させない。

```sh
python -m tools.evidence.runner run tools/evidence/specs/evidence-tools.json dist/evidence-tools
python -m tools.evidence.runner verify dist/evidence-tools
```

新規の出力先のみを使用する。sourceは先にcommitし、そのcommit/pathで参照する。
実行specとlogはdataとして保存し、runnerやrepositoryを結果配下にコピーしない。
生成したraw log・大きな証拠は `dist/` とCI artifactへ保存し、Gitへ恒久追加しない。
Gitには小さなscope・source参照・必要なmanifestを残す。Actions artifactの保存期限を
永久保存と扱わず、長期保全が必要な証拠は期限前に承認済みの外部保存先へ退避する。
再利用する回帰probeは通常のtestへ置く。例外的な外部入力や再構成できない資料には
保存理由を記す。独立レビューは対象commit・scope・指摘・未検証範囲を文章で残す。
`conformance/results/` は状態索引が直接参照する64 KiB以下の型付きJSONを所有する。
許可する型は段階履歴参照、TaskEvidence、正式AcceptanceEvidenceである。
未知の型・余剰field・未参照ファイル・source snapshot・実行scriptをrepository checkで拒否する。
段階履歴参照 `nepl3.stage-history/1` はtask IDと元記録のrevision・path・SHA-256を保持する。
この参照を使用できる状態はin-progressであり、completeや正式受入へ転用できない。
正式受入の原logは `dist/evidence/` へ取得し、validatorが元byte列とdigestを検査する。
CI artifact等の取得先・保存期限は公開記録に示す。取得不能時は検証を失敗とする。
通常のrepository checkはarchive refを取得しない。履歴は固定Git revisionから取得する。
Doc inventoryの明示的な履歴監査は別のbaselineを使用する。通常のrepository
checkからは分離しており、歴史資料の検査を依頼したときだけ旧revisionを取得する。
保存済みの過去記録は [履歴索引](../../conformance/history.md) を参照する。

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
