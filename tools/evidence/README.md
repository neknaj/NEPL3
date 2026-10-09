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

## A01 architecture command evidence

`specs/a01.json` collects compiler/target identity, repository dependency checks,
dependency-checker regressions and an ARMv6-M library build of every implemented
portable crate, including both Math output adapters and both Suite adapters.
Run from a clean committed checkout with the pinned Rust toolchain and installed
`thumbv6m-none-eabi` target:

```sh
python -m tools.evidence.runner run tools/evidence/specs/a01.json dist/evidence/a01-run
python -m tools.evidence.runner verify dist/evidence/a01-run
```

Use a new output path for every run. The current repository inventory is checked
by `tools.evidence.test_a01`; adding an implemented portable crate requires
updating this spec and both CI compile-target package lists. Planned crates
without manifests are not claimed as implemented or compiled.

This spec collects command evidence for A01 / host-repository only. The collector
keeps `acceptance_decision=false`; ARM compilation is not ARM execution, and no
other acceptance group is implied. Formal AcceptanceEvidence still requires an
independent scope review, current source/spec identities, typed target records,
and nonempty hashed logs in its required format. Preserve the collector bundle
unchanged. Arrange durable retention and clean-checkout restoration of referenced
logs before recording `passed`; this change does not add that publication or
restoration workflow and does not change acceptance status.

## Local raw-bundle transport

`archive.py` preserves a collector bundle byte-for-byte in a deterministic,
flat, uncompressed ZIP. It neither executes the recorded argv nor creates
AcceptanceEvidence, changes status, authenticates an execution, or publishes
anything. Failed/interrupted collector records remain failed/interrupted data.
The original collector bundle, including `acceptance_decision=false`, is retained.

```sh
python -m tools.evidence.archive pack dist/evidence/a01-run dist/evidence/a01-run.zip --source-revision <collected-40-digit-revision>
python -m tools.evidence.archive restore dist/evidence/a01-run.zip dist/evidence/a01-restored --source-revision <collected-40-digit-revision> --expected-sha256 <independently-recorded-archive-digest>
python -m tools.evidence.runner verify dist/evidence/a01-restored
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
must fail verification; local archive roundtrip success does not mark A01 passed.

## Opt-in CI preservation

The CI workflow's `collect_a01` boolean is false by default. Explicit manual
workflow dispatch with that input enabled runs the committed `a01.json` spec on
Ubuntu with the pinned Rust toolchain, the ARMv6-M compile-only target and Python
3.13. It preserves raw collector output and, when valid, the bounded inner ZIP.
Failed collector or archive steps remain failures; upload does not approve them.
Job cancellation or runner loss can prevent upload altogether. Missing output
must fail verification rather than become an empty successful bundle.
The quality gate requires this job to succeed when requested, and to be skipped
otherwise. Normal push and pull-request runs do not opt into this collection. Explicit
collection uses a run-ID-suffixed concurrency group, so it neither cancels nor is
cancelled by ordinary CI on the same ref. Direct cancellation and runner loss
are still possible.

The Actions artifact name contains the selected source commit, run ID and attempt.
Its run summary records the collector/archive outcomes, artifact URL and inner-ZIP
SHA-256. Requested retention is 30 days; actual artifact metadata and availability
must be checked when retrieving it. This is neither permanent storage nor a
promise that an expired artifact can be recovered. Arrange authorized longer-term
retention before expiry when the evidence still needs to be kept.

No token or new write permission is added, and checkout does not persist Git
credentials. The job never edits acceptance status or creates a formal evidence
record. Authenticated selection/retrieval, clean-CI restoration, the formal-record
bridge and independent acceptance review remain separate work. A local source
contract test is not evidence that this workflow has executed on GitHub.


## Actions metadata selection

`actions.select_download` checks caller-supplied REST artifact and run metadata
against an independently selected repository name/ID, source commit, run ID,
attempt, artifact ID and inner-archive digest. The caller must obtain metadata
through authenticated GitHub REST access and establish source/workflow eligibility;
these checks do not authenticate arbitrary supplied JSON. For an older attempt,
obtain that attempt's run metadata rather than silently selecting the latest run.

Both metadata documents are limited to 64 KiB. Duplicate keys, nonfinite numbers,
excessive nesting, mismatched repositories/commits/attempts, incomplete runs,
unknown outcomes and unavailable/expired artifacts fail. Timestamp ordering must
satisfy created_at <= updated_at <= caller UTC observation time < expires_at. Names bind this
uploader's attempt label because artifact `workflow_run` has no attempt field.
The workflow path may include the REST API's informational `@ref` suffix; that
suffix never selects source or grants execution authority.

The advertised digest and byte length are checked against the downloaded outer
ZIP bytes, before extraction. This digest identifies the outer download byte
boundary. The independently selected inner ZIP digest is retained but not yet
checked or compared with an inner archive by this selection stage.
The returned URL is the exact GitHub API artifact download endpoint, not its
short-lived signed redirect. A failure/cancelled run may preserve diagnostic
bytes; its conclusion remains failure/cancelled and is never an acceptance pass.

This stage performs no HTTP request, redirect, ZIP parsing, filesystem write or
command execution. Bounded outer-container validation, inner-byte restoration,
current source/spec checks, formal evidence records and independent acceptance
review remain separate requirements. Synthetic selection tests do not establish
that any GitHub workflow ran or that an artifact is currently downloadable.

API contracts: [artifact metadata](https://docs.github.com/en/rest/actions/artifacts#get-an-artifact),
[run metadata](https://docs.github.com/en/rest/actions/workflow-runs#get-a-workflow-run),
and [artifact download](https://docs.github.com/en/rest/actions/artifacts#download-an-artifact).


## Bounded Actions outer container

`container.inspect` performs metadata selection and then inspects the downloaded
outer ZIP entirely in memory. It returns the original verified inner ZIP bytes;
it does not extract files, issue HTTP requests, execute commands or decide A01.
The expected inner SHA-256 remains a caller-selected value. An uploaded sidecar
must agree with it but cannot choose a different trusted archive.

This is a deliberately narrow contract for this workflow's pinned uploader and
bounded ASCII evidence paths, not a general-purpose GitHub artifact ZIP reader.
It accepts streamed DEFLATE and stored files, variable timestamps/order/regular
file permissions, and signed data descriptors. ZIP64, extras, comments, explicit
directories, encryption, special files and unexpected paths fail. The pinned
producer does not need ZIP64 below these size/count bounds. Its common upload
root is removed, leaving a01.zip, a01.zip.sha256 and a01-command/<raw filename>.

Before ZipFile metadata allocation, the actual central-directory records, names,
count and byte span are bounded. Local headers must match central names, flags,
compression, sizes and CRCs; local payload spans must be contiguous and disjoint.
Stream descriptors are checked explicitly. DEFLATE output is capped at the
advertised admitted size plus one byte, with exact end-of-stream, size and CRC
checks. Trailing compressed payload, hidden entries and prefix/gap bytes fail.
Total expanded data includes both the inner archive and redundant raw copies.
The raw file set and every copy must match the validated inner archive exactly.

These checks cannot prove that an original uploader input was never a symlink:
the uploader can resolve a link and store regular target bytes. Collector source
checks and independent execution review retain their own responsibilities.
Synthetic streamed-ZIP tests verify this consumer, not a hosted CI artifact.
Actual authenticated retrieval, clean-CI restoration and formal evidence remain
unverified until separately executed for the selected source/run/attempt.

Producer sources: [pinned uploader bundle](https://github.com/actions/upload-artifact/blob/043fb46d1a93c77aae656e7c1c64a875d1fc6a0a/dist/upload/index.js),
[upload path search](https://github.com/actions/upload-artifact/blob/043fb46d1a93c77aae656e7c1c64a875d1fc6a0a/src/shared/search.ts),
and [ZIP framing dependency](https://github.com/archiverjs/node-compress-commons/blob/6.0.2/lib/archivers/zip/zip-archive-output-stream.js).
