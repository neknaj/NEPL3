# 開発と検査

## Portable execution CI

WASI foundationは同じcomponentをWasmtime 44.0.1のnative codegenと
Pulley64で実行し、終了状態・試験数・elapsed以外の出力を比較します。
PulleyもCraneliftでbytecodeを生成するため、別compilerの検証とは扱いません。
空のlibrary test binaryはemptyとして記録し、集約では実試験の成功を要求します。
各processは180秒で停止・回収し、失敗ログもartifactへ保存します。
比較用componentは `cargo test --release` で最適化します。通常のdebug WASI試験は
別途維持します。非最適化のengine parse全20件はCIのPulleyで180秒を超えたため、
試験の削除やcoreの資源上限変更ではなく、配布時にも使う最適化コードを比較します。

[RP2040 adapter](../conformance/targets/rp2040/README.md) は別workspaceの
bare-metal harnessです。production core/reader/wire/engineの依存を使用し、
HAL・UART・allocatorを外側へ置きます。ARMv6-M buildと固定版rp2040jsでの
UF2実行は別job・別証拠です。初期実行範囲はsource位置・budget・NDFの4件で、
Reader/Engine全受入や実機試験の代わりにはしません。

実ブラウザのWasm・Playground、RISC-V、big-endianの検査はそれぞれ独立した未達範囲です。
このCI整備をDoc HTML・文書移行・Pages公開の完成へ読み替えません。
CIの区切り後はDoc生成を進め、意味・リンク・安定IDの対応を検証できたページから
nepld正本へ移行し、検査済みの同じsite artifactを公開します。

## ローカル環境

`doc/canonical.json` に登録された仕様はnepldを編集します。生成Markdownを直接変更しないでください。
`cargo run --locked -p nepl3-tools -- doc-canonical --check` はproduction APIで再生成して差分を検査します。
更新時は `doc-markdown annotated` で新しい一時ファイルへ生成し、差分をレビューして既存のprojectionへ反映します。
ページ間リンクを持つ `nepl3-tools.markdown-annotated-pages/1` の登録後は、
`cargo run --locked -p nepl3-tools -- doc-canonical markdown dist/canonical-markdown` で全登録ページを
新規ディレクトリへ生成します。参照先・別ページのaliasも同じ入力集合に含め、旧rendererのページは
既存byte列を維持します。成功した生成物の差分をレビューしてからprojectionへ反映してください。
`cargo run --locked -p nepl3-tools -- doc-canonical html dist/doc-canonical` は、同じ正本からHTML一式を新規出力します。
HTML集合の有限出力予算を明示する場合は、registryの `html_output_limits` に8資源をすべて指定します。
Markdown用の `output_limits` とは別の設定であり、未指定時は従来のHTML既定値を維持します。
停止した実行を成功として再試行せず、新しい設定での生成は別の実行として記録します。
いずれも文書の明示的な生成工程とし、Cargo build.rsへ入れません。

Doc HTMLの配置は、固定版Playwrightと対応するChromium・Firefox・WebKitでCI実行します。
実NEPL3入力からproduction backendで生成した13例を、2画面幅・3文字サイズ・3行高で表示し、
Ruby/Annoのbaseline、複数行、入れ子、注釈と前後行の高さ予約を検査します。
文書のJavaScriptは無効です。これは静的HTMLの検査であり、Wasm実行や支援技術の操作試験ではありません。
runner不在・例の欠落・配置不一致は失敗となり、quality jobも失敗します。

```sh
python -m pip install -r tools/audit/doc_html/requirements.txt
python -m playwright install chromium firefox webkit
mkdir -p dist/doc-browser
cargo test --locked -p nepl3-tools --test doc html::browser_layout_corpus_from_real_doc_source -- --exact --nocapture > dist/doc-browser/corpus.log
python tools/audit/doc_html/browser.py --corpus dist/doc-browser/corpus.log --css crates/languages/doc/html/assets/doc.css --output dist/doc-browser/results.json
```

Linuxではbrowser用のsystem libraryも必要なため、CIは`playwright install --with-deps`を使います。
PowerShellで実行ログを保存する際は、後述のUTF-8の指針に従ってください。

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
python tools/audit/allocation/run.py
python tools/generate/grammar.py
python tools/generate/doc.py
python tools/generate/math.py
python -m unittest discover -s tools/bootstrap -p test_grammar.py
```

allocation検査は [単体probe](../tools/audit/allocation/probe.rs) の呼出し区間で実際の割当を計測します。SourceMapが予算ゼロを返す前にsource IDを複製したR020は、返却値と論理的な使用量だけの試験では捕捉できません。このため計測器に限定したGlobalAlloc wrapperのunsafeを開発用途で監査します。unsafeは同じpointer/layoutをSystem allocatorへ転送する箇所だけとし、計測counterは割当を伴わないatomic操作です。coreと通常workspaceの `unsafe_code = "forbid"` は維持し、productionへ計測器を依存させません。

同じ計測器はR037のSourceStore.applyも検査します。100000byteのSourceIdを持つ編集に対し、AllocationUnits=0で停止するまでに実割当がないことを要求します。共有SourceAdmission、全削除の入力入場、複数sourceの原子性と再試行は別のproduction API試験で検証します。古いapply署名はadmissionを受け取らないため、旧版の元反例と現行probeのAPI差を区別して記録します。

[driver](../tools/audit/allocation/run.py) は固定toolchainの `cargo build --locked` が出力するJSONから対象coreのartifactを一意に取得し、同じtoolchainのrustcで一時実行ファイルを作ります。既存workspaceのlint設定を変更せず、この独立した開発計測器にだけ上記の範囲を適用します。意図的な割当のpositive controlを先に検査し、入力構築・表示・返却値のdropを計測区間から外します。失敗・runner不在・artifactの曖昧さは非0終了であり、未実行を成功へ読み替えません。通常native CIでもこのコマンドを実行し、単なる任意の手元確認にはしません。計測はこの具体的な先行割当の回帰を捕捉するもので、一般の物理メモリ上限やWASIでの実測を保証しません。

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

共通型を変更したときは `cargo run --locked -p nepl3-tools -- foundation --write` で実際のpackage descriptorを生成します。`check` は正本との一致に加え、production core registryによるdigest照合・参照finalizeも行います。通常buildからschema生成を呼ぶbuild.rsは置きません。

reader包絡の正本は `interfaces/reader.json` です。変更時は `cargo run --locked -p nepl3-tools -- reader --write` でproductionの登録コードを生成します。参照するfoundation型と合わせたregistryのfinalizeを検査し、VMのstate・request・continuation型と同じ変更で同期します。

Grammarの型付きconstructor arenaは `design/forms.json` から `python tools/generate/grammar.py --write` で明示生成します。この生成物は構文shapeの投影であり、reader/binding/style/extensionを含むLanguagePackageをforms表だけから作るものではありません。初回seed入力adapterの `tools/bootstrap/grammar.py` は完全なsyntax.neplgを読み、元bytes/digest、constructor/literal/listのUTF-8 byte範囲と全metadataを保持したASTを出します。ASCII識別子のseed用部分集合に限定し、TextではNEPL3のescapeを使いJSON固有escapeを拒否します。これはproduction parserやbootstrap合格の代わりではなく、実Grammar compilerへの初期入力を用意する開発host処理です。P1/P2はproduction reader/engineで同じsourceを読み直して比較します。

CIのWASI jobはSHA-256を固定したWasmtime 44.0.1で実装済みcoreの実試験を実行し、browser向けWasmのcompileも行います。ローカルでは `CARGO_TARGET_WASM32_WASIP2_RUNNER` を `wasmtime run` とし、`cargo test --locked -p nepl3-core -p nepl3-reader -p nepl3-wire -p nepl3-engine -p nepl3-grammar-core -p nepl3-doc-core -p nepl3-math-core -p nepl3-markup --target wasm32-wasip2 -- --test-threads=1` を実行します。browser targetのcompile成功はブラウザ上の実行・描画試験を意味しません。

Binding の fixture と Python seed adapter の一致確認は子processを起動する native host 専用試験です。native の通常試験で実行し、Wasm target ではその host 試験だけを型条件で除外します。同じ fixture を使う production compile・parse・analyze・portable codec の試験は `cargo test --locked -p nepl3-tools --test grammar binding:: --target wasm32-wasip2 -- --test-threads=1` でも実行します。host 試験を WASI へ誤って含めた初回失敗は対象選択の失敗として記録し、後の runtime 試験成功へ読み替えません。

Doc は `cargo test --locked -p nepl3-doc-core` と `cargo test --locked -p nepl3-tools --test doc` で検査します。後者は実4言語文法のcompile、ParseSession、Doc provider/prefix lowerを通します。同じ試験は `cargo test --locked -p nepl3-tools --test doc --target wasm32-wasip2 -- --test-threads=1` で実行し、元sourceとPython seed adapterの一致確認1件だけをnative専用とします。Docの進行中の範囲と未接続操作は [段階実装](progress/doc-runtime.md) を参照してください。

Math は `cargo test --locked -p nepl3-math-core` と `cargo test --locked -p nepl3-tools --test math` で検査します。後者は実文法のcompile・ParseSessionからMathの表記を保持するlowerを通し、同じ元入力でWASIも実行します。構造検査と数値演算の個別試験から、式全体の評価・束縛・印字の完成を推定しません。[段階実装](progress/math-runtime.md)に検証済みの範囲と残りを記録します。

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

## ページ集合と移行候補

内部のページ参照を含むDoc原稿は、[21章](spec/21-doc-pages.md) の登録manifestから
一括生成できます。全ページの参照・表示anchor・出力pathを検査してから保存します。

```sh
python tools/migration/contract.py
python -m unittest discover -s tools/migration -p test_contract.py
mkdir -p dist
cargo run --locked -p nepl3-tools -- doc-html pages doc/migration/pages.json dist/doc-migration
```

[最初の移行候補](migration/README.md) は実際のDoc処理系でHTMLへ変換します。
CIは限定変換器の固定した移行前fixtureとの一致と生成を各native OSで検査し、
Ubuntuの生成物をartifactへ保存します。現在の仕様からの生成・差分検査は
別の `doc-canonical` 経路で行います。
これは公開deployでも正本切替でもありません。旧anchor・Markdown projection・意味審査の
条件を満たしてから、ページ単位で正式文書を移行します。

## CIと配布

Doc単独の生成物は次で確認できます。出力先は存在しないディレクトリを指定してください。

```sh
cargo run --locked -p nepl3-tools -- doc-html export examples/document/linear-combination.nepld .tmp/linear-combination-html
```

`document.html` と `assets/` を一緒に配布します。数式・外部ページ・画像等の解決はまだこのlocal入口の対象外で、必要な場合は生成を拒否します。正式文書の正本切替とPages公開は、リンク・意味同等性・配布検査を含む別の工程です。契約は [20章](spec/20-doc-html.md) を参照してください。

[CI workflow](../.github/workflows/ci.yml) はpush・pull request・手動実行で起動し、Linux・Windows・macOSで上記のRust検査とリポジトリ検査を実行します。文書だけの変更も対象です。すべてのmatrix jobの成功を集約する固定名 `quality` を、mainの必須status checkとして使用します。失敗・cancel・skipを成功へ読み替えません。

`main` へのpushで `quality` が成功した後、同じcommit SHAのGit管理対象をsource archiveとしてActions artifactへ保存します。archiveにはLICENSE・仕様・schema・文法・例・開発toolsが入り、SHA-256とcommit識別情報を添付します。保存期間は30日です。これは基盤整備段階の継続的な成果物配布であり、言語runtimeのbinary releaseではありません。`.tmp/` はarchiveへ入りません。

source archiveは内容の参照とbuildに使うsnapshotで、`.git/` は含みません。`nepl3-tools check` はGit管理対象も検査するため、完全なリポジトリ検査にはcloneしたcheckoutが必要です。artifact内の `SOURCE.txt` のcommit値を使って `git checkout <commit>` し、上記の検査を実行してください。

Actionsの権限は読み取りに限定し、checkoutにcredentialを残しません。外部actionはcommit SHAで固定し、DependabotがCargo依存とActionsの更新PRを作成します。依存更新時も仕様と検査を確認します。

GitHub側ではdescription・topics・文書へのhomepageを設定し、Issuesを有効、Wiki・Projectsを無効にします。merge後のbranch削除、依存の脆弱性通知・修正PR、secret scanning・push protection、非公開の脆弱性報告を使用します。mainの保護には必須check `quality` を用い、設定後は実際のrepository状態を確認します。

開発はbranchとPRで進め、mainへ統合する前に独立レビューとCIを確認します。mainの保護設定は `quality` 必須・最新mainに対する検査必須（strict）、管理者にも適用、force push・削除は禁止とします。GitHubアカウントによる必須承認数は設定せず、agentの独立レビュー記録と区別します。

共通基盤のWASI試験、ブラウザWasm向けbuild、ARMv6-M向けbuildとRP2040 emulator実行は、上記のPortable execution CIで検査します。これらの実行範囲と、WASI CLI・LSP・operation provider・Web Playgroundという製品入口の完成は区別します。各入口には実装した操作を実runnerで通す受入試験を追加し、foundationの試験やnative開発toolsの成功だけから製品全体のcross-target対応を推定しません。ブラウザ向けbuildも実ブラウザでの実行とは別の証拠です。runtime releaseは該当するconformanceの実行証拠がそろってから設けます。

Web/TEA/siteとDoc移行の計画は [14章](spec/14-web-ui.md)〜[16章](spec/16-doc-migration.md) に従います。現在のCIはsource配布を維持し、Pages公開はT20の実装・受入後です。T21は初回公開とは別に最終完了へ必須で、移行前はMarkdownを正本とします。

Pagesの実装では、公開後smokeが失敗したcandidateに対して [15章の復旧契約](spec/15-site.md) を実行します。public smoke済みLKGの元tarを通常のActions retentionとは別に保持し、同じpublisher lockで対象identityを確認して1回だけ復旧・再smokeします。新しい健康な公開や対象不明時は上書きせず停止します。復旧できても元candidate/runはfailedです。現在はこの設計の整備であり、live Pagesの保存先・journal・復旧workflowは未実装です。

workflowの構文・権限・依存jobの扱いは [GitHub Actions公式仕様](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax)、artifactの保存は [公式ガイド](https://docs.github.com/en/actions/tutorials/store-and-share-data) に従います。

## 実装と独立レビュー

Doc関連の共通値・wire・文書意味APIを固定する前に、[早期inventoryとgap audit](doc-inventory.md) を確認します。`cargo run --locked -p nepl3-tools -- doc-inventory --check` は固定commitの原本と照合し、現在の文書・契約の追加/変更/削除を報告します。通常の `check` にも含まれます。現在の網羅性を主張する場合は新しいcommitを監査して `doc-inventory --check-current` を通します。現時点のbaselineと作業treeには差分があり、strict検査が失敗することを未移行/未監査の成功へ読み替えません。再生成方法と履歴要件は監査文書を参照してください。

メインagentが設計具体化、実装、試験、指摘修正と統括を担当し、subagentには独立レビューだけを依頼します。レビュー担当はメインagentの説明だけを根拠にせず、元の契約、実コード、失敗系、期待値の根拠、差分と実行結果を確認します。メインagentが必要な修正・再レビュー・再検査を確認して統合します。専用branchでこまめにcommit・pushし、未レビューのcheckpointと統合可能な変更を区別します。利用上限等で独立レビューが未実行の場合も成功にせず、独立して進められる作業を続けます。具体的な規範は [AGENTS.md](../AGENTS.md) を参照してください。

HTML fragmentの生成adapterは `python tools/generate/markup.py`、schema projectionは `cargo run --locked -p nepl3-tools -- markup --write` を使用する。fragment単体の成功をDoc/asset/KaTeX/Web全体の受入へ拡張しない。

Doc HTMLの値schemaは `cargo run --locked -p nepl3-tools -- doc-html --write`、型付きadapterは `python tools/generate/doc_html.py --write` で生成し、通常checkで照合する。`cargo test --locked -p nepl3-doc-html` とtoolsのDoc試験でnative/実source経路を検査する。local-onlyのfragment成功を、資源解決・配布文書・T23全体の成功へ拡張しない。
