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

## Local raw-bundle transport

`archive.py` preserves a collector bundle byte-for-byte in a deterministic,
flat, uncompressed ZIP. It neither executes the recorded argv nor creates
AcceptanceEvidence, changes status, authenticates an execution, or publishes
anything. Failed/interrupted collector records remain failed/interrupted data.
The original collector bundle, including `acceptance_decision=false`, is retained.

```sh
python -m tools.evidence.archive pack dist/evidence/raw-run dist/evidence/raw-run.zip --source-revision <collected-40-digit-revision>
python -m tools.evidence.archive restore dist/evidence/raw-run.zip dist/evidence/raw-restored --source-revision <collected-40-digit-revision> --expected-sha256 <independently-recorded-archive-digest>
python -m tools.evidence.runner verify dist/evidence/raw-restored
```

Select the expected digest and collected revision outside the incoming archive.
The digest identifies this inner ZIP, not an enclosing GitHub Actions download.
The archive does not contain its own expected digest or a locator-bearing formal
record. Current source/spec identity must still be checked separately before
using restored logs in any formal acceptance attempt.

Inputs have at most 256 regular files, each at most 1 MiB and together at most
32 MiB. Only the collector's spec, manifest and recorded stdout/stderr files are
allowed. Empty raw output channels are preserved. The ZIP central-directory
size and entry count are bounded before parsing; compressed, encrypted, ZIP64,
noncanonical, linked/special, duplicate, nested and unexpected members fail.
Symlink/reparse ancestors, hardlinked inputs and existing destinations fail.
A source snapshot must pass the same collector consistency checks after restore.

Every archive/member/hash check precedes creation of the output directory.
Restore exclusively creates that directory; it never replaces an existing
directory. A write, close or readback failure may leave incomplete output. That
output is unaccepted, is not automatically deleted, and retries require a new
destination. This preserves unrelated files if another process replaces or
modifies the destination. Consumers must wait for successful return and verify
the returned digest; filesystem creation is not an atomic publication protocol.
This is not a sandbox against concurrent workspace replacement. Pack/restore
errors do not grant acceptance or publication.

This is the local preservation/restoration boundary only. Authenticated remote
artifact selection, advertised retention/expiry, clean-CI retrieval, the formal
record bridge and independent scope review remain required. Unavailable logs
must fail verification; local archive roundtrip success does not mark an acceptance group passed.

## Committed input identity

`cargo run --locked -p nepl3-tools -- evidence identity --commit <40-hex-commit>`
reads locally available immutable Git objects through the current tool. It shares
the checkout identity profile and exclusions; it does not check out or execute
historical source, honor replacement objects, or fetch missing objects. Dirty
working-tree/index inputs cannot silently replace the pinned commit's bytes.
A matching profile is an input-identity comparison, not authentication of a CI
run or acceptance of its results. Artifact selection, expiry, safe log restoration,
and the formal-record bridge remain separate requirements.

## Actions ZIP byte boundary

`artifact.py` decodes an independently pinned outer Actions ZIP into unchanged
member bytes. Unlike the canonical local collector archive, this transport accepts
stored or deflated members and signed streaming data descriptors. It requires
flat lowercase ASCII names, regular files, bounded directory records before ZIP
allocation, matching contiguous local headers/payloads/descriptors, and no ZIP64,
extra fields, comments, links, duplicate names or unowned local bytes. Every member
is CRC-checked; the complete outer ZIP must match the caller-supplied SHA-256.
Limits are 256 members, 96-byte names, 1 MiB per expanded member and 32 MiB for
both the complete ZIP and all expanded data. Empty output channels are preserved.

This is a pure byte-decoding boundary. It does not authenticate the digest pin,
select or download an artifact, verify its repository/run/source/expiry, write
logs, or grant acceptance. The typed formal-record locator below preserves those pins; authenticated
retrieval and filesystem restoration are separate boundaries described below. Real Actions ZIP compatibility tests
are diagnostic observations, not formal execution evidence.

## Typed artifact locator

Command and review runs may carry an optional `artifact` object with schema
`nepl3.github-artifact/1`. It pins this repository and repository ID, run/attempt,
source commit, artifact ID/name, outer ZIP digest/size, exact raw-log member and
advertised expiry as UTC Unix seconds. Missing remains backward-compatible;
explicit null, unknown fields, duplicate fields and positional arrays fail.
Integer tokens must have no fractional part or exponent. A locator does not
replace the existing raw-log digest, target-kind, independent-review or result
checks, and cannot turn a rejected review into a successful process run.

The initial metadata verifier is restricted to completed successful main-push
CI runs. GitHub PR workflow head IDs and actual merge checkout IDs differ; this
backend rejects PR runs rather than assuming equality. It verifies repository,
run attempt, source, workflow path, advertised digest/size/expiry and the artifact
creation interval. This metadata boundary does not download or restore files.
The actual selected source/spec identity must still match the current checkout
and formal record. Unavailable or expired artifacts must fail retrieval, even
when a cached log exists. No latest-run substitution or automatic pin refresh is
permitted. No acceptance group is promoted by these tooling components.

## Pinned log retrieval

Run `python -m tools.evidence.retrieval` from the current checkout. It builds the
current Rust identity tool, reads only status-owned acceptance records, compares
their source/spec identity with the current checkout and pinned Git commit, then
restores unchanged member bytes under `dist/evidence/`. It never changes status,
selects a latest artifact, executes historical code, or treats transport success
as acceptance. Run the existing `tasks --check` and repository `check` afterward;
CI runs retrieval before these gates. An empty plan performs no HTTP requests.

The existing `GITHUB_TOKEN` needs only repository contents/read and Actions/read.
The token goes solely to fixed `api.github.com` endpoints. A separately opened
HTTPS connection follows one authenticated redirect to Azure Blob storage without
the token, cookies or proxy credentials. Other redirect hosts fail explicitly.
A killed/reaped worker enforces a 45-second total network deadline; metadata,
HTTP framing, archive bytes and decompressed members all have explicit limits.
Artifacts created in the same second as either run-attempt interval boundary are
rejected as ambiguous. Advertised expiry is checked before and after retrieval.

At most 256 log requests, 64 MiB of outer downloads and 32 MiB of expanded data
are accepted per attempt. All sources and raw hashes are checked before output
creation. Existing matching logs are preserved; conflicting files, links and
unsafe ancestors fail. A locator always requires fresh metadata/retrieval, even
when a local log matches. Without a locator an explicitly supplied matching local
log remains usable, but a fresh checkout lacking it fails. A failed write may
leave partial unaccepted output; no unrelated files are deleted or replaced.
This is not a sandbox against a process concurrently replacing the workspace.

Offline transport, metadata and filesystem fixtures are tooling tests. Real
artifact collection, successful authenticated retrieval, scope review and the
unchanged formal Rust checks are required before claiming an acceptance attempt.
