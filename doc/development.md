# 開発と検査

## ローカル環境

[rust-toolchain.toml](../rust-toolchain.toml) に固定したRustと、Gitを使用します。rustupはworkspace内で指定toolchainを選びます。`cargo` の各コマンドはリポジトリrootで実行してください。`Cargo.lock` は管理対象で、CIでは `--locked` を使います。

Doc inventoryのbaseline検証は固定commitのGit objectを必要とするため、CI checkoutは全履歴を取得します。shallow cloneではbaselineを取得してから検査し、object不在を監査成功としてskipしません。source archive単体にはこの履歴が含まれません。

開発toolchainは1.97.0、現時点で宣言・検査するMSRVは1.97です。元設計の「1.85以上」は選定可能な下限であり、1.85での実行証拠を意味しません。初期段階では実際に検査するtoolchainとMSRVを一致させ、未検証の旧版対応を広告しない方針を採ります。将来MSRVを変更するときはCargo.toml、toolchainとCIの検査対象を合わせて見直します。

独立した構造監査にはPython 3.13を使用します。これは標準ライブラリだけで動く開発host用の補助検査で、productionの依存ではありません。Rustの共通parserの完成を示すものでもありません。

テキストはUTF-8、通常はLFです。PowerShellのファイルI/Oでは `Get-Content -Encoding UTF8`、`Set-Content -Encoding UTF8` など、encodingを明示します。source位置試験のfixtureと取り込み元の証拠ファイルは元byte列を維持します。

```sh
rustup show active-toolchain
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -p nepl3-tools -- check
cargo run --locked -p nepl3-tools -- tasks --check
python tools/audit/structure.py
python -m unittest discover -s tools/audit -p test.py
```

APIドキュメントも警告をエラーとして検査します。

```powershell
$env:RUSTDOCFLAGS = '-D warnings'
cargo doc --workspace --no-deps --locked
if ($LASTEXITCODE -ne 0) { throw 'cargo doc failed' }
```

PowerShellではnative commandの失敗がスクリプトを自動停止させるとは限りません。複数コマンドを自動化するときは、各終了コードを検査してください。POSIX shellでは `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked` と実行できます。

タスクを変更したときは正本の `design/tasks.json` を編集し、生成後の差分を確認します。

```sh
cargo run --locked -p nepl3-tools -- tasks --write
cargo run --locked -p nepl3-tools -- tasks --check
git diff --check
git diff
```

`check` はリポジトリ内の設計metadataと実際のworkspaceの整合を検査します。言語処理系の代用ではなく、登録されたruntime受入試験を実行するコマンドでもありません。`implementation-status.json` の状態は実際の実行証拠に基づいて更新します。

タスクのacceptance参照はcoverageを示します。T16以外のタスクは自身の成果物・scope付き証拠・依存完了・設計blocker解消で判定し、後段を含む試験群全体の合格は別に記録します。証拠にはtask ID、検査対象、コマンド、target、結果、未検証範囲を残します。T16の完了には登録された全必須群のpassedが必要です。

scope付き証拠は `conformance/results/` 以下へJSONで保存します。次は形式を示す例で、実行済みの記録ではありません。実際の検査名・コマンド・targetと未検証部分に置き換え、実行を確認してから状態を更新してください。

```json
{
  "task_id": "T01",
  "checks": ["実際に検査したsource契約の条件"],
  "commands": ["実際に成功した検査コマンド"],
  "targets": ["実際のtarget triple"],
  "result": "passed",
  "excluded_acceptance_portions": ["E03/E04の未実装のエディタ操作"]
}
```

群全体のpassedは別のAcceptanceEvidenceを使用します。必須群・targetは [design/acceptance.json](../design/acceptance.json)、形式は [証拠schema](../interfaces/acceptance-evidence.schema.json) に従います。scope付きTaskEvidenceを群全体の成功証拠として流用しません。現在の入力identityは次の操作で取得します。

```sh
cargo run --locked -p nepl3-tools -- evidence identity
```

identity profileは `nepl3.repository-inputs/1` です。Gitから見える非ignoreファイルをpathのUTF-8 byte順で並べ、implementation-status.json、conformance/results/、生成tasks/を除外します。source digestはこの集合全体、spec digestはそのうちdoc/spec/、interfaces/、design/を対象にします。各入力はdomain文字列 `nepl3.repository-inputs/1`、zero byte、`source` または `spec`、zero byteで開始し、各pathの長さ（u64 big-endian）・UTF-8 path・内容長（u64 big-endian）・元byte列を順にSHA-256へ入力します。

CIと同じ.gitattributesに従うfresh checkoutで通常ファイルをLFにそろえ、実際に検査したtreeからidentityを取得します。digest処理自体は改行・BOM・Unicodeを正規化しません。source位置fixtureや保存資料の元byte列を変えてdigestを合わせることは禁止です。

証拠は同じdesign revisionとsource/spec digest、群ID、必須target、実行結果、log digestへ照合します。改変した証拠・別ID・古い版・未実行targetをpassedとして受理しません。形式検査だけで試験の正しさを証明した扱いにせず、期待値とlogの独立レビューを行います。

実行証拠は `kind: command` としてコマンド、終了コード、runner/tool版を記録します。人による意味レビューは `kind: review` としてreviewer、独立性、scope、approved/rejectedを記録し、架空のコマンドを作りません。catalogのtarget種別と一致させ、非空の.txt/.log記録とSHA-256を添付します。詳細な必須fieldは証拠schemaに従います。

## CIと配布

[CI workflow](../.github/workflows/ci.yml) はpush・pull request・手動実行で起動し、Linux・Windows・macOSで上記のRust検査とリポジトリ検査を実行します。文書だけの変更も対象です。すべてのmatrix jobの成功を集約する固定名 `quality` を、mainの必須status checkとして使用します。失敗・cancel・skipを成功へ読み替えません。

`main` へのpushで `quality` が成功した後、同じcommit SHAのGit管理対象をsource archiveとしてActions artifactへ保存します。archiveにはLICENSE・仕様・schema・文法・例・開発toolsが入り、SHA-256とcommit識別情報を添付します。保存期間は30日です。これは基盤整備段階の継続的な成果物配布であり、言語runtimeのbinary releaseではありません。`.tmp/` はarchiveへ入りません。

source archiveは内容の参照とbuildに使うsnapshotで、`.git/` は含みません。`nepl3-tools check` はGit管理対象も検査するため、完全なリポジトリ検査にはcloneしたcheckoutが必要です。artifact内の `SOURCE.txt` のcommit値を使って `git checkout <commit>` し、上記の検査を実行してください。

Actionsの権限は読み取りに限定し、checkoutにcredentialを残しません。外部actionはcommit SHAで固定し、DependabotがCargo依存とActionsの更新PRを作成します。依存更新時も仕様と検査を確認します。

GitHub側ではdescription・topics・文書へのhomepageを設定し、Issuesを有効、Wiki・Projectsを無効にします。merge後のbranch削除、依存の脆弱性通知・修正PR、secret scanning・push protection、非公開の脆弱性報告を使用します。mainの保護には必須check `quality` を用い、設定後は実際のrepository状態を確認します。

開発はbranchとPRで進め、mainへ統合する前に独立レビューとCIを確認します。mainの保護設定は `quality` 必須・最新mainに対する検査必須（strict）、管理者にも適用、force push・削除は禁止とします。GitHubアカウントによる必須承認数は設定せず、agentの独立レビュー記録と区別します。

WASI・ブラウザWasm・LSP・operation providerは目標仕様です。対応する実装とrunnerができた段階で、buildに加えてrunnerによる受入試験を必須jobとして追加します。現時点のnative開発toolsの検査からcross-target対応を推定しません。runtime releaseは該当するconformanceの実行証拠がそろってから設けます。

Web/TEA/siteとDoc移行の計画は [14章](spec/14-web-ui.md)〜[16章](spec/16-doc-migration.md) に従います。現在のCIはsource配布を維持し、Pages公開はT20の実装・受入後です。T21は初回公開とは別に最終完了へ必須で、移行前はMarkdownを正本とします。

Pagesの実装では、公開後smokeが失敗したcandidateに対して [15章の復旧契約](spec/15-site.md) を実行します。public smoke済みLKGの元tarを通常のActions retentionとは別に保持し、同じpublisher lockで対象identityを確認して1回だけ復旧・再smokeします。新しい健康な公開や対象不明時は上書きせず停止します。復旧できても元candidate/runはfailedです。現在はこの設計の整備であり、live Pagesの保存先・journal・復旧workflowは未実装です。

workflowの構文・権限・依存jobの扱いは [GitHub Actions公式仕様](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax)、artifactの保存は [公式ガイド](https://docs.github.com/en/actions/tutorials/store-and-share-data) に従います。

## 実装と独立レビュー

Doc関連の共通値・wire・文書意味APIを固定する前に、[早期inventoryとgap audit](doc-inventory.md) を確認します。`cargo run --locked -p nepl3-tools -- doc-inventory --check` は固定commitの原本と照合し、現在の文書・契約の追加/変更/削除を報告します。通常の `check` にも含まれます。現在の網羅性を主張する場合は新しいcommitを監査して `doc-inventory --check-current` を通します。現時点のbaselineと作業treeには差分があり、strict検査が失敗することを未移行/未監査の成功へ読み替えません。再生成方法と履歴要件は監査文書を参照してください。

メインagentが作業範囲、依存順、担当ファイル、完了条件を統括し、実装subagentと独立したレビューsubagentを分けます。レビュー担当は実装担当自身の確認とは別に、契約、失敗系、テストの根拠、差分を評価します。メインagentが指摘の解消と必要な再検査を確認して統合します。具体的な規範は [AGENTS.md](../AGENTS.md) を参照してください。
