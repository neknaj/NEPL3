# 開発と検査

この文書は現在の開発操作を案内します。編集先は [canonical registry](canonical.json)、必要な成果物は [タスク定義](../design/tasks.json)、必須受入群・targetは [受入catalog](../design/acceptance.json) で確認します。

- 初回準備と基本検査: [ローカル環境](#ローカル環境)。
- 文書変更: [文書生成と初期Pages公開](#文書生成と初期pages公開)、[ページ集合と移行候補](#ページ集合と移行候補)。
- 提出・統合: [branchとPRの統合手順](#branchとprの統合手順)、[実装と独立レビュー](#実装と独立レビュー)。
- target別の保証範囲: [Portable execution CI](#portable-execution-ci)。過去の停止理由と現在の実行構成は分けて読みます。

編集中は変更を再現する局所試験を使い、提出前に影響する契約・生成物・targetを検査します。下記の全体検査一覧とCIの必須条件は維持し、部分試験を正式受入の代用にしません。

## ローカル環境

[rust-toolchain.toml](../rust-toolchain.toml) に固定したRustと、Gitを使用します。rustupはworkspace内で指定toolchainを選びます。`cargo` の各コマンドはリポジトリrootで実行してください。`Cargo.lock` は管理対象で、CIでは `--locked` を使います。

Doc inventoryの明示的なbaseline監査は固定commitのGit objectを必要とします。通常の `check` からは分離しており、監査時だけbaselineを取得します。object不在を監査成功としてskipしません。source archive単体にはこの履歴が含まれません。

開発toolchainは1.97.0、現時点で宣言・検査するMSRVは1.97です。元設計の「1.85以上」は選定可能な下限であり、1.85での実行証拠を意味しません。初期段階では実際に検査するtoolchainとMSRVを一致させ、未検証の旧版対応を広告しない方針を採ります。将来MSRVを変更するときはCargo.toml、toolchainとCIの検査対象を合わせて見直します。

Pythonの開発ツールにはPython 3.13を使用する。仮想環境を作成し、`tools/typing/requirements.txt`、`tools/extensions/requirements.txt`、`tools/audit/doc_html/requirements.txt` の固定依存を導入する。型検査器は開発専用の依存として `tools/typing/requirements.txt` で管理する。

`pyproject.toml` のbasedpyright設定は、試験を含む `tools/` の全Pythonを `all` モードで検査する。仮想環境のPythonと型検査器を使用し、別の環境から起動する場合は `basedpyright --pythonpath <仮想環境のPython>` で対象を指定する。警告・エラーは失敗として扱う。repository検査は、管理対象のPythonが検査範囲内にあり、隠しディレクトリや仮想環境の自動除外に含まれないことも確認する。

JSON・TOMLの入力検証は `tools/serialization/`、文法の型付き定義は `tools/catalog/` が担当する。監査は `tools/audit/`、生成は `tools/generate/`、bootstrapは `tools/bootstrap/`、実行環境との接続は `tools/emulators/`、証拠収集は `tools/evidence/`、外部consumerの検査は `tools/extensions/`、サイト公開は `tools/site/` が担当する。外部入力を検証して型付きの値へ変換し、各責務の内部処理へ渡す。

仮想環境を有効にした後、次を実行する。ブラウザを使用する試験では、別途 `python -m playwright install` で対応ブラウザを導入する。

```sh
python -m pip install -r tools/typing/requirements.txt -r tools/extensions/requirements.txt -r tools/audit/doc_html/requirements.txt
basedpyright
python -m unittest tools.serialization.test_json tools.serialization.test_toml tools.catalog.test_forms tools.generate.test_adapters tools.extensions.test_cargo tools.extensions.test_execution tools.audit.allocation.test_run tools.audit.math.test_visual tools.audit.math.test_annotations
```

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

CIのWASI jobはSHA-256を固定したWasmtime 44.0.1で実装済みcoreの実試験を実行し、browser向けWasmのcompileも行います。ローカルでは `CARGO_TARGET_WASM32_WASIP2_RUNNER` を `wasmtime run` とし、`cargo test --locked -p nepl3-core -p nepl3-reader -p nepl3-wire -p nepl3-engine -p nepl3-grammar-core -p nepl3-doc-core -p nepl3-sentence-core -p nepl3-math-core -p nepl3-markup -p nepl3-doc-html --target wasm32-wasip2 -- --test-threads=1` を実行します。browser targetのcompile成功はブラウザ上の実行・描画試験を意味しません。

providerのstream transportは `cargo test --locked -p nepl3-provider` で検査する。WASIでは同じコマンドに `--target wasm32-wasip2` を追加する。frameの分割転送、切断、長さ制限、codecとI/Oの停止、Close後の再利用拒否、schema交換を対象とする。

Replyの追加sourceを検査するhostは、任意選択の `Connection::receive_pending_reply` を使用できる。
最初の段階は構造検査済みrequest IDだけを公開し、元のregistry・SourceAdmission・Budgetを保持する。
`finish_with` は保存済み要求のIDと選択操作の出力型を照合してから、Completeまたは存在するpartialのsource検査callbackを一度だけ呼ぶ。
callbackが返すSourceStoreはReportをdecodeするための明示的なclosureであり、実行権限を追加しない。
Awaitとpartialのない応答ではcallbackを呼ばず、元のclosureを使用する。Await内の子要求のsourceは外側のReportへ流用しない。
受信開始前に接続を閉状態へ移し、finish成功後だけ再開する。失敗・panic・未完了receiptの破棄・forgetは閉状態を保つ。
Readerの意味検査、request lifetimeの確定、生成sourceの認可、remote Usageの検証、子processの終了・回収はhostの後続責務であり、このAPIでは実行しない。
既存のreceive系APIは変更しない。局所試験は `cargo test --locked -p nepl3-wire --test operation pending::` と `cargo test --locked -p nepl3-provider --test transport reply::pending::` で実行する。

nativeの依存操作schedulerは `cargo test --locked -p nepl3-suite --test dispatch` で検査する。WASIでは `--target wasm32-wasip2 -- --test-threads=1` を追加する。schedulerは明示的なframe列でInvoke・依存要求・Resumeを逐次実行し、同じ実行Budgetへ祖先の上限と深さを適用する。要求ごとのsource権限、循環検出、Invalidのpartialと診断、停止後のcallback抑止、取消を検査する。追加生成sourceの認可・登録とprocess間の総予算管理は後続の実装範囲である。T11・T12は段階実装中であり、正式受入の状態は `implementation-status.json` のacceptanceを参照する。

nativeの `cargo test --locked -p nepl3-provider --test process_protocol` は、実processのstdin/stdoutでschemaを取得し、Invoke・Await・Resumeの結果とUnicode診断をnative経路と比較する。schema不足・identity不一致・応答前EOFと、Await中のCancelも検査する。このtest targetは専用harnessを使い、protocol用stdoutへテストランナーの表示が混入することを防ぐ。WASIではOS process試験を明示的にskipする。一般的なhost scheduler、process間の総予算管理、全providerの互換性は継続する実装・受入範囲である。

Sentenceの統合経路は `cargo test --locked -p nepl3-tools --test sentence --target wasm32-wasip2 -- --test-threads=1` で実行します。独立LanguagePackageのsurface compile、ReaderSession/provider、literalのportable受渡し、位置付き診断・停止、Doc bridgeを対象とし、Sentence core単体試験と区別します。nativeではworkspace試験に含み、WASIでも同じ入口を実行します。

Binding の fixture と Python seed adapter の一致確認は子processを起動する native host 専用試験です。native の通常試験で実行し、Wasm target ではその host 試験だけを型条件で除外します。同じ fixture を使う production compile・parse・analyze・portable codec の試験は `cargo test --locked -p nepl3-tools --test grammar binding:: --target wasm32-wasip2 -- --test-threads=1` でも実行します。host 試験を WASI へ誤って含めた初回失敗は対象選択の失敗として記録し、後の runtime 試験成功へ読み替えません。

Doc は `cargo test --locked -p nepl3-doc-core` と `cargo test --locked -p nepl3-tools --test doc` で検査します。後者は実4言語文法のcompile、ParseSession、Doc provider/prefix lowerを通します。同じ試験は `cargo test --locked -p nepl3-tools --test doc --target wasm32-wasip2 -- --test-threads=1` で実行します。元sourceとPython seed adapterの一致確認1件、およびhostのfile入力・resource出力・alias不正時の出力抑止を扱う3件はnative専用です。Docの進行中の範囲と未接続操作は [段階実装](progress/doc-runtime.md) を参照してください。

Math は `cargo test --locked -p nepl3-math-core` と `cargo test --locked -p nepl3-tools --test math` で検査します。後者は実文法のcompile・ParseSessionからMathの表記を保持するlowerを通し、同じ元入力でWASIも実行します。構造検査と数値演算の個別試験から、式全体の評価・束縛・印字の完成を推定しません。[段階実装](progress/math-runtime.md)に検証済みの範囲と残りを記録します。

タスクのacceptance参照はcoverageを示します。T16以外のタスクは自身の成果物・scope付き証拠・依存完了・設計blocker解消で判定し、後段を含む試験群全体の合格は別に記録します。証拠にはtask ID、検査対象、コマンド、target、結果、未検証範囲を残します。T16の完了には登録された全必須群のpassedが必要です。

scope付き証拠は `conformance/results/` 以下へ型付きJSONで保存する。原logはCI artifactまたは外部保存先から `dist/evidence/` へ取得し、検証時にSHA-256を照合する。in-progressの段階記録には、Git revision・path・SHA-256を持つ `nepl3.stage-history/1` を使用できる。completeには次のTaskEvidenceを要求する。例の検査名・コマンド・target・未検証部分を実際の結果に対応させ、実行を確認してから状態を更新する。

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

## 文書生成と初期Pages公開

初期Pages公開は `.github/workflows/pages.yml` が担当します。main pushのCI成功後、
そのrunのDoc site・検査report・元tarを取得し、commit/manifest/内容一致を再検査して公開します。
検査済みsiteのfile byte列を公式upload-pages-artifactで梱包します。CI tarは照合用に保持し、
Pages輸送tarと同一byteとは扱いません。初回の独自tar直接uploadはPages側でdeployment_failedとなりました。
tar entryの先頭`./`等の形式差が疑われるため、輸送を公式actionへ委譲します。HTMLは再buildしません。
公開後のHTTP smoke失敗はworkflow失敗として記録します。LKG/journalによる自動復旧は未提供です。
Markdown/NEPL3d混在公開は正本移行を支える段階であり、全ページ移行やT19/T20完成を意味しません。

`cargo run --locked -p nepl3-tools -- site build site/config.json dist/site` は、
登録済みDoc正本のHTMLに静的な索引を付け、新規ディレクトリへ一式を生成します。
追跡済みの設定とcleanな入力checkoutを要求し、生成中にHEADや入力の変更を検出した場合は出力しません。
`build.json` のsource commitは入力checkout、rendererは実行したbinaryのSHA-256と
そのbuild時のrustcを別々に記録します。binaryのsource commitを入力checkoutから推定しません。
CIでは同じcheckoutからbuildしたbinaryと生成ログを結び付けて保管します。
埋込template・CSSとcheckoutの不一致も拒否します。toolsのbuild.rsはcompiler識別のみを行い、
文書生成やGrammar compileを実行しません。
現在の入口は登録済みDocページ、未移行のMarkdown仕様書、`site/examples.json`の原文例を含むdocs-only生成です。
原文の配布byte・Profile・digestは同じcheckoutと照合し、表示から例を実行しません。
仕様書以外の未移行文書・rustdoc・実行例の操作結果を含む統合、
公開後smokeと復旧を含むPages配信、T19/T20全体の完了は別途検証します。

`python tools/site/payload.py dist/site dist/pages.tar --manifest-sha256 <検査済みmanifestのSHA-256>`
は、検査対象と同じfile集合・byte列を確認し、HTMLを再生成せずPages用のtarへ固定します。
CIは実ブラウザ検査のreportからdigestを渡し、元site・tar・receiptを同じartifactに保存します。
tar内の順序・時刻・所有者・modeは固定し、リンク（Windows junctionを含む）、読取り失敗、
未登録file、改変、上限超過を拒否します。出力先は入力siteの外にある新規fileに限ります。
receiptの `publication_verified` はfalseです。この梱包だけでは公開確認やLKG昇格になりません。
境界試験は `python -m unittest discover -s tools/site -p 'test_*.py'` で実行します。

復旧用の `tools/site/recovery.py` は、別途確認したLKGのtar/manifest digestと取得した元tarの
byte列を受け取ります。ファイルシステムへ展開せず、各member・manifest・file集合の衝突・
内容・上限を検査し、元byte列をそのまま返します。圧縮wrapperは取得側で分離し、原tarの
identityと混同しません。canonical tarとの比較は検査であり、比較用の再serialize結果を
復旧出力として使いません。保存物のimmutability、現在の公開対象、journalの復旧許可は
別途検査が必要です。通常CIには保存済みの実Doc tarを用いた回帰試験も含めます。

`python tools/site/smoke.py dist/site https://neknaj.github.io/NEPL3/ --manifest-sha256 <検査済みdigest>`
は、docs-only成果物とHTTPS応答のbyte列、HTML/CSSのMIME、directory indexと未知routeの404を
照合します。build identityを前後で取得し、redirectや内容の混在を拒否します。
`.nojekyll` は公開内容ではなく配信制御fileとして照合対象から外し、結果にも記録します。
socket timeoutに加えて外側processを最大300秒で終了させ、失敗時は非zero終了と理由を返します。
`--local-http` は明示port付き127.0.0.1だけで利用でき、結果はlocal試験として区別します。
このHTTP照合は公開smokeの一部であり、Pages API/journalの現行deployment、実browser表示、
全cacheの原子的切替やLKGを証明しません。publisherはそれぞれの証拠を別途照合します。

受入群の文書検査は、Markdownの本文・箇条書きの行頭にあるplainな `A01:` 型の定義を読みます。
escapeや文字参照をdecodeした表示上のprefixを使うため、nepldから生成した `- A01\:` も
同じ定義として扱います。見出し・引用・code・HTML・表・脚注や文中の単なる言及は定義にしません。
ID自体をlinkや強調で組み立てず、説明部分に必要な注釈・code・linkを配置してください。
inline HTMLを含む段落では、タグの後の改行から定義の抽出を再開しません。
次の独立した段落・list itemから再開し、非表示span中のIDを定義と誤認するのを避けます。
重複定義を拒否し、catalogとの集合一致と全required条件の検査は維持します。

`doc/canonical.json` に登録された仕様はnepldを編集します。生成Markdownを直接変更しないでください。
`cargo run --locked -p nepl3-tools -- doc-canonical --check` はproduction APIで再生成して差分を検査します。
更新時は `doc-markdown annotated` で新しい一時ファイルへ生成し、差分をレビューして既存のprojectionへ反映します。
ページ間リンクを持つ `nepl3-tools.markdown-annotated-pages/4` の登録後は、
`cargo run --locked -p nepl3-tools -- doc-canonical markdown dist/canonical-markdown` で全登録ページを
新規ディレクトリへ生成します。参照先・別ページのaliasも同じ入力集合に含め、旧rendererのページは
同じrendererでの単独生成と本文を一致させる。Rubyは`<ruby>`・`<rt>`で出力し、タイトル直後にNEPL3d正本への相対リンクを置く。成功した生成物の差分をレビューしてからprojectionへ反映する。
各Markdownのヘッダーは、そのページの正本・alias・出力設定と、利用するリンクのroute・fragmentに依存する。集合全体のidentity、schemaに依存するdocument digest、ページごとの参照依存一覧は生成manifestに記録する。無関係なページや内部schemaの変更で本文・参照が同じ場合、対象Markdownのbyte列を保持する。
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
cargo build --locked -p nepl3-tools
python tools/audit/doc_html/export.py target/debug/nepl3-tools dist/doc-browser/export-modes
```

Linuxではbrowser用のsystem libraryも必要なため、CIは`playwright install --with-deps`を使います。
PowerShellで実行ログを保存する際は、[ローカル環境](#ローカル環境)のUTF-8の指針に従ってください。

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
CIは限定変換器の固定した移行前fixtureとの一致と生成をLinuxで検査し、
Ubuntuの生成物をartifactへ保存します。現在の仕様からの生成・差分検査は
別の `doc-canonical` 経路で行います。
これは公開deployでも正本切替でもありません。旧anchor・Markdown projection・意味審査の
条件を満たしてから、ページ単位で正式文書を移行します。

## CIと配布

Doc単独の生成物は次で確認できます。出力先は存在しないディレクトリを指定してください。

```sh
cargo run --locked -p nepl3-tools -- doc-html export examples/document/linear-combination.nepld .tmp/linear-combination-html
```

既定の外部CSS方式では、`document.html` と `assets/` を同梱する。
HTML単体でスタイルを保持して配布する場合は、次のコマンドを使用する。

```sh
cargo run --locked -p nepl3-tools -- doc-html export --css inline examples/document/linear-combination.nepld .tmp/linear-combination-inline
```

`--css external` は既定の方式を明示する。`--css inline` は固定CSS全体をHTMLへ内包し、CSPのハッシュで許可する。manifestは方式、実際のファイル一覧、CSSのハッシュとライセンスを記録する。閲覧時はHTML単体を配布できる。複数文書でCSSを共有する場合は外部CSS方式を使用する。

Docの本文はKlee Oneの400・600をGoogle Fontsから読み込む。フォントファイルは出力へ同梱しない。ネットワークを使用できない場合は、システムの代替フォントで表示する。外部CSS・HTML内包の両方式で、CSPはfonts.googleapis.comのスタイルシートとfonts.gstatic.comのフォントだけを追加許可する。閲覧時にこれらの配信元への要求が発生する。

ブラウザー検証は、375px・1280pxでの外部CSS・内包CSS・複製HTMLの表示比較、オフラインの代替フォント、Google Fontsの実際の読み込みを含む。画像は`dist/doc-browser/export-modes/`へ出力する。サイト全体の検証ではGoogle Fontsの応答を空のCSSへ置換し、代替フォントによる表示を検査する。


数式・外部ページ・画像等の解決はまだこのlocal入口の対象外で、必要な場合は生成を拒否します。正式文書の正本切替とPages公開は、リンク・意味同等性・配布検査を含む別の工程です。契約は [20章](spec/20-doc-html.md) を参照してください。

[CI workflow](../.github/workflows/ci.yml) はmainへのpush・pull request・手動実行で起動します。feature branchのpushとPR更新による全workflowの二重実行を避け、PRではGitHubのmerge refを検査します。PR未作成のcheckpointは自動CI検証済みとは扱わず、必要なら手動実行します。Rustのworkspace testとClippyはLinux・Windows・macOSで実行し、fmt・rustdoc・allocation regression・外部consumer全体・Doc移行とcanonical生成はLinuxで一度実行します。repository checkはrepository-contract jobへ集約します。

tag pushの自動CIも維持します。CI成功だけで正式releaseや公開確認済みとは扱いません。

siteの全Python試験はLinuxのsite-publication jobで実行します。Windowsではjunction拒否、Windows/macOSではstdinへ渡す引数の構築と実processの強制終了・HTTP通信と期限、macOSではsymlink祖先を持つtemporary rootの回帰だけを追加実行します。stdinの試験はprocess呼出しをmockして渡す値を検査し、実際のprocess間転送の証拠とは区別します。共通のmanifest/hash/tar検査をOSごとに反復しません。evidence runnerのファイル・process境界は3 OSで維持します。

文書だけの変更も対象です。固定名 `quality` は引き続き全jobの成功を要求し、失敗・cancel・skipを成功へ読み替えません。ブラウザ・Pulley・RP2040の実行頻度変更は、この重複削減とは別に、変更の影響範囲とmainでの拡張試験を対応させて設計します。

検査対象sourceはCI runのcommit SHAで特定します。通常のmain pushではsource tarを別artifactとして再保存せず、Git checkoutまたはGitHubのcommit指定source archiveを使用します。正式releaseで同一の配布byte列を保全する必要がある場合は、そのreleaseの成果物として扱います。

source archiveには `.git/` は含まれません。`nepl3-tools check` の履歴・Git管理対象検査にはcloneしたcheckoutが必要です。CI runのcommit SHAを使って `git checkout <commit>` し、上記の検査を実行してください。GitHubの再生成archiveは同じcommitのファイル内容を参照する手段であり、圧縮byte列の永続的同一性を保証するものではありません。

Actionsの権限は読み取りに限定し、checkoutにcredentialを残しません。外部actionはcommit SHAで固定し、DependabotがCargo依存とActionsの更新PRを作成します。依存更新時も仕様と検査を確認します。

GitHub側ではdescription・topics・文書へのhomepageを設定し、Issuesを有効、Wiki・Projectsを無効にします。merge後のbranch削除、依存の脆弱性通知・修正PR、secret scanning・push protection、非公開の脆弱性報告を使用します。mainの保護には必須check `quality` を用い、設定後は実際のrepository状態を確認します。

開発はbranchとPRで進め、mainへ統合する前に独立レビューとCIを確認します。mainの保護設定は `quality` 必須・最新mainに対する検査必須（strict）、管理者にも適用、force push・削除は禁止とします。GitHubアカウントによる必須承認数は設定せず、agentの独立レビュー記録と区別します。

共通基盤のWASI試験、ブラウザWasm向けbuild、ARMv6-M向けbuildとRP2040 emulator実行は、上記のPortable execution CIで検査します。これらの実行範囲と、WASI CLI・LSP・operation provider・Web Playgroundという製品入口の完成は区別します。各入口には実装した操作を実runnerで通す受入試験を追加し、foundationの試験やnative開発toolsの成功だけから製品全体のcross-target対応を推定しません。ブラウザ向けbuildも実ブラウザでの実行とは別の証拠です。runtime releaseは該当するconformanceの実行証拠がそろってから設けます。

Web/TEA/siteとDoc移行の計画は [14章](spec/14-web-ui.md)〜[16章](spec/16-doc-migration.md) に従います。現在のCIは実装と生成物を検査し、Pages公開はT20の公開前条件を満たしてから行い、公開後の試験を別に記録します。T21は初回公開とは別に最終完了へ必須で、移行前はMarkdownを正本とします。

Pagesの実装では、公開後smokeが失敗したcandidateに対して [15章の復旧契約](spec/15-site.md) を実行します。public smoke済みLKGの元tarを通常のActions retentionとは別に保持し、同じpublisher lockで対象identityを確認して1回だけ復旧・再smokeします。新しい健康な公開や対象不明時は上書きせず停止します。復旧できても元candidate/runはfailedです。現在はこの設計の整備であり、live Pagesの保存先・journal・復旧workflowは未実装です。

workflowの構文・権限・依存jobの扱いは [GitHub Actions公式仕様](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax)、artifactの保存は [公式ガイド](https://docs.github.com/en/actions/tutorials/store-and-share-data) に従います。

## branchとPRの統合手順

通常は更新済みmainから短命のtopic branchを作り、一つの論理的変更と関連試験を同じPRで閉じます。同じ要求の修正を別の依存PRへ分散させず、無関係なtopicを取り込む統合branchへ転用しません。実装中の主要topic一つとレビュー・CI待ち一つ程度を目安とし、同じ領域の未統合stackを伸ばす前に先行変更を閉じます。独立した作業を禁止する件数上限ではありません。

mainの取得・確認だけでは作業branchを更新しません。必要なAPIや修正の導入、競合解消、strict gateへの対応など、理由がある時点で更新し、その理由をPRへ記録します。統合順序を決め、レビュー修正と必要なbase更新をまとめてから統合候補を固定し、必須CIを完了させます。同じtreeでも、別headの成功や別baseとの検証を現在候補の成功へ読み替えません。保護設定を緩めて同期を省略しません。

依存PRでは親PRと親head、子固有のcommit範囲を記録します。親の統合後は旧base・旧headと子branchからの参照を確認・保全し、子固有の差分だけを新mainへrebase/cherry-pickして整理します。親がsquash済みなら、古い親commitを子の変更として再投入しません。元系列とのrange-diff、最終差分、競合解消を確認し、必要な再レビューと新候補のCIを行います。共有branchの書換えは利用者・子branchへの影響を確認し、remoteの期待SHAを指定したforce-with-leaseを使います。通常のforce pushは使いません。確認できなければ新branchへ固有差分を移し、旧branchを保全します。

通常の単一変更PRはsquash mergeを既定とします。例外の理由を示したうえで、個別変更を線形に残す場合はrebase merge、元のcommitと統合点を保持する場合はmerge commitを選びます。rebase mergeは元のSHAを保持しません。統合後のtopic branchを別の作業へ再利用せず、公開済みmainは別途合意した履歴移行なしに書き換えません。既存stackも一括で無条件に書き換えず、親から順に整理します。

PRには対象commit・base、確認した契約と差分、重要な指摘と解消、独立実行とログ確認の区別、未確認範囲を短く残します。merge queueは導入済みとは扱いません。必要になった場合に既存GitHub機能と`merge_group`のCI対応を別途検証します。独自の統合管理frameworkは追加しません。

この運用は [Gitのtopic workflow](https://git-scm.com/docs/gitworkflows) と [GitHubの統合方式・squash後のbranchに関する注意](https://docs.github.com/en/pull-requests/reference/pull-request-merges) を参考に、NEPL3の独立レビュー・strict CI条件へ適用したものです。

## 実装と独立レビュー

失敗した試験を通すために要求を縮小したり、必須target・検査を除外したり、資源制限やbranch保護を緩和したりする変更は、通常の実装修正と区別します。元の要求、変更理由、失われる保証と代替検証を具体化し、既にユーザーが承認した範囲かを確認します。未承認の要件縮小・権限や公開設定の変更は、その内容を示して承認を得るまで適用しません。要求を保存する仕様誤記・型定義の整合や通常の実装修正まで、一律の承認待ちにはしません。

独立レビューでは、契約違反・安全性・誤結果・必須検証不足を現在の変更で解消します。密接に関連する保守性の改善は範囲を確認して取り込み、独立機能・広範囲な再設計・任意の追加検査は別の要求として採否を判断します。同じ原因の再指摘が続く場合は、変更を足す前に契約と期待値を再確認します。待機中に進める作業はレビュー対象を変更せず、未確定の契約に依存しない範囲とします。レビュー担当には固定commitと確認範囲を渡し、試験の出力先を分けます。共有refsの更新は統括するメインagentが行います。

Doc関連の共通値・wire・文書意味APIを固定する前に、[早期inventoryとgap audit](doc-inventory.md) を確認します。`cargo run --locked -p nepl3-tools -- doc-inventory --check` は固定commitの原本と照合し、現在の文書・契約の追加/変更/削除を報告します。通常の `check` とは分離し、baselineの履歴を取得した上で明示的に実行します。現在の網羅性を主張する場合は新しいcommitを監査して `doc-inventory --check-current` を通します。現時点のbaselineと作業treeには差分があり、strict検査が失敗することを未移行/未監査の成功へ読み替えません。再生成方法と履歴要件は監査文書を参照してください。

メインagentが設計具体化、実装、試験、指摘修正と統括を担当し、subagentには独立レビューだけを依頼します。レビュー担当はメインagentの説明だけを根拠にせず、元の契約、実コード、失敗系、期待値の根拠、差分と実行結果を確認します。メインagentが必要な修正・再レビュー・再検査を確認して統合します。専用branchでこまめにcommit・pushし、未レビューのcheckpointと統合可能な変更を区別します。利用上限等で独立レビューが未実行の場合も成功にせず、独立して進められる作業を続けます。具体的な規範は [AGENTS.md](../AGENTS.md) を参照してください。

HTML fragmentの生成adapterは `python tools/generate/markup.py`、schema projectionは `cargo run --locked -p nepl3-tools -- markup --write` を使用する。fragment単体の成功をDoc/asset/KaTeX/Web全体の受入へ拡張しない。

Doc HTMLの値schemaは `cargo run --locked -p nepl3-tools -- doc-html --write`、型付きadapterは `python tools/generate/doc_html.py --write` で生成し、通常checkで照合する。`cargo test --locked -p nepl3-doc-html` とtoolsのDoc試験でnative/実source経路を検査する。local-onlyのfragment成功を、資源解決・配布文書・T23全体の成功へ拡張しない。
# 正本と観測記録の境界

意味の規範は `doc/spec/`、構文・依存・値と操作の機械契約は
`design/forms.json`、`design/dependencies.json`、`interfaces/` が所有する。
これらは優先順位で矛盾を隠すための階層ではなく、異なる責務の正本である。
同じ契約が食い違ったときは仕様・schema・生成物・試験を一緒に訂正する。

`design/tasks.json` は計画、`design/acceptance.json` は受入定義、
`implementation-status.json` は観測状態と証拠参照を所有する。`design/review.json`
は課題台帳であり、correctedはruntime合格を意味しない。`tasks/*.md` は生成表示。
Docページの正本とMarkdown projectionは `doc/canonical.json` の対応に従う。
生成adapterはgeneratorの入力から再生成し、JSONという拡張子だけで全てを正本としない。

`doc/migration/generated/doc-inventory.json` は固定された過去commitの派生監査であり、現在の全文書
一覧ではない。保存によりbaseline改変を差分と再生成検査で検出するため管理を維持する。
script inventoryは必要時に生成する派生監査資料であり、仕様の新しい正本として保存しない。

`design/review.json` のopen課題は対象taskの完了制約として使用し、correctedの説明は
歴史記録として読む。現行制約を解決せず、台帳の移動や削除だけで完了可能にしない。
rootの `languages/` は文法定義、`crates/languages/` はRustの意味処理実装である。
`site/` は公開入力と静的asset、`tools/site/` はhost側の配布処理と試験を所有する。

Grammarのproduction compilerによるbootstrap一致検査は恒久的な回帰検査である。
一方、初回入力用seed adapterは置換可能なhost補助であり、現在はDoc生成も使用する。
代替する検査済みpackage読込経路が成立するまで、移行用という名称だけで削除しない。
Doc移行候補の作成toolは全対象ページの切替・参照更新後に役割を再評価する。

taskの段階着手には利用する前段成果物が必要。`depends_on` の全タスク完了は
当該taskをcompleteにする条件である。部分的な実装・監査をin-progressで記録しても、
依存先や全受入が完成したことにはならない。過去の段階証拠への参照は索引として扱い、
現HEADの実行証拠や正式な受入認定へ読み替えない。

証拠の保存は [共通収集手順](../tools/evidence/README.md) に従う。
実行test、レビュー文章、公開記録は異なる証拠種別である。保存場所の名前に関わらず、
review成功からconformance合格、uploadから公開済み、公開済みからLKGを推定しない。
新しい保存script・sourceコピーを結果配下に作らず、再現に必要なcommit/path、宣言入力、
command、環境、原出力とhashを使用する。

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

### HTML shellの出力buffer再利用

Doc hostは、検証・serialize済みfragmentの所有Stringへ固定のhead/tailを追加する。
最終長をchecked計算し、既存capacityを超える分のAllocationUnitsと、prefix挿入・
追記・必要な再配置のWorkを課金してからfallible reserveする。本文を別の所有bufferへ
再構築しないため追加allocationを減らすが、prefix挿入は本文を移動するためzero-copyや
実行時間短縮を保証しない。既存のHTML検証・shell depth・OutputBytes課金・sticky stopは
維持する。HTML/CSSのbyte同等性と、余剰capacity・exact/one-short上限・既停止Budgetを
検査する。Usageの差は所有storageの再利用によるもので、上限増額や未実行受入の合格を
意味しない。

### Span識別子の一時コピー削減

FoundationCodecのSpan admissionはSnapshotIdを借用してSourceStoreを検索する。
SpanのNDF変換も識別子の各fieldを借用し、最終的に必要な所有NDF Textだけを作る。
既停止Budgetのpoll、完全なsource identityの照合、admission、範囲・UTF-8境界とschemaの
検査は保持する。削除したAllocationUnits課金は、実際に除去した一時SourceIdコピーに
対応する。所有値を使う参照経路とのNDF・CBOR・digest一致、長い非ASCII識別子、
exact/one-short上限とsticky stopを検査し、論理課金の減少と実heap測定は区別する。

### Span expected-type descriptor reuse

FoundationCodec lazily constructs its private expected Span descriptor once per
codec. First use charges the actual 16-byte package and 4-byte name payload;
subsequent validations borrow that descriptor while still validating every NDF
value against the finalized registry. Constructors and child scopes start cold
without allocation, and all paths poll the supplied budget. Source admission,
range validation and mapping-scope invalidation are unchanged. Logical allocation
savings alone do not establish native heap usage or full Doc export acceptance.

### Inline schema-validation frontier

Structural validation retains up to eight pending references in a fixed inline
array. Wider frontiers use budgeted, fallible heap storage for overflow entries,
retaining capacity for reuse. The traversal order and all schema checks remain
unchanged, including per-child Work charging and Nodes/depth accounting. The
removed AllocationUnits correspond to removed heap requests, not waived checks;
inline storage is bounded independently of input depth. Exact/one-short tests
cover both inline and spill boundaries. Full acceptance remains separate from
these local storage checks.

### Staged reply-source process regression

The dedicated process harness includes three fixed terminal-reply cases for
receive_pending_reply/finish_with: explicit admission of a trusted Unicode
source before Report decoding, missing-source failure, and a policy-triggered
sticky cancellation. The successful case verifies exact frame/diagnostic data
and preservation of the following Close frame. Failed cases remain closed and
cannot consume further Usage through receive. All children use the existing
bounded deadline and reap/termination harness. Source fixtures are constructed
before the measured receipt, not with a replacement policy Budget.

This is real-pipe evidence for the staged transport prerequisite. It does not
execute a Reader operation, negotiate this fixture's registry, establish domain
acceptance or remote accounting, or complete a formal acceptance group.

### Standalone builtinName sender

`portable::read::sender::execute_name` checks the complete selected standard
operation identity, admits only authorized entries of the request's explicit
source table, and narrows decoding, checked context, native execution and later
encoding to that owned table. Only the actual native builtinName path can issue
its private result; no arbitrary ReadReply or fabricated ReaderContinuation is
accepted. The issued result preserves execution Usage and its Report. A separate
bounded encoding budget permits serialization/retry even after native execution
stops, without resetting or rerunning execution. Encoding preserves the existing
ReadReply schema and exact inner/outer Report correspondence.

The process fixture compares Unicode Matched, NoMatch, nonfinal NeedMore and a
native WorkLimit outcome with the same standalone native invocation. A fixed
fixture registry and source policy are selected independently on both sides;
this fixed whitelist does not exercise general Grants/dispatch_invoke authorization
or request lifetime management. It is not schema negotiation or a host
ReaderSession resume. Initial caller
identity/depth association, cumulative remote-usage reconciliation and general
Read/Dependent/Transform/Await integration remain unfinished. No formal acceptance
status is promoted by these local tests. The provider's direct Reader dependency
is development-only; Reader production still depends only on core.

### Reader accounting is not settlement alone

The provider Reader regression uses a real saved ReaderSession Await, actual
builtinName execution, and an IssuedInvocation settled from the independently
observed local execution Budget. The unchanged operation-local reply is still
rejected by the saved Reader receiver and resume path, preserving the pending
slot. One case has a larger saved Work baseline; a second uses a longer real
Unicode token so every additive child counter reaches the saved baseline while
its depth remains below the saved absolute depth. No Report or observed counter
is rewritten to satisfy those checks. This protects the missing accounting/depth
bridge rather than implementing it. The delegated negative cases measure builtin execution with separately budgeted
fixture encoding, not end-to-end remote accounting. The positive control charges
real execution, encoding and decoding to the shared parent Budget at the saved
depth, then successfully resumes and consumes the preserved pending slot.

### Native pending builtinName dispatch

`ReaderSession::execute_pending_name` selects the private pending call, including
its actual request cursor and linked standard signature, rather than reconstructing
an invocation from the outer continuation request. It checks saved limits and
monotonic usage, then resolves the checked native context and source table on the
caller's actual shared Budget and source-admission ledger. These objects must be
retained by the host; equal scalar counters alone cannot establish their provenance.
The typed native path does not project the request to NDF or encode/decode it.

Typed preparation is charged before the relative additive grant begins. Checked
addition rejects overflow and grants exceeding the outer limits; depth remains
an absolute ceiling covering historical peak, current depth and saved call depth.
Execution restores outer limits and current depth while retaining usage and sticky
stops. The issued result can be consumed with `into_reply` for ordinary typed
resume; issuance does not consume the pending slot. Repeating execution reruns the
pure builtin and charges again. Serialization retry instead reuses the issued
result with an explicitly bounded encoding budget and preserves its original Report.

Tests distinguish the pending cursor after a Unicode prefix from the outer request
cursor, reuse admitted source bytes with zero new source grant, reject incompatible
linked signatures, closed/missing sessions and invalid budget/depth baselines,
and preserve WorkLimit in an issued stopped reply. This is a same-Budget native
helper. Distributed accounting, authenticated observations, inherited remote source
admission, general operation dispatch and formal acceptance remain separate work.

### Saved Reader and Tokenizer echoes

`ReaderSession` and `TokenizationSession` provide `pending_continuation_value`
and `resume_continuation_value`. Tokenizer reservation waits use
`reserve_continuation_value`. These APIs project the complete canonical saved
continuation, then compare received NDF values against the still-owned private
state. Canonical source ordering and coalescing of identical declarations affect
only the projection, never native rollback storage or saved Usage.

`pending_reply_value` projects the complete pending domain envelope:
`ReadReply::Await`, or `TokenizationReply` with Await/Reserve and all eight outward
fields. `resume_reply_value` and Tokenizer's `reserve_reply_value` compare that
entire envelope, including duplicated call/request/report fields. They do not
create a common `OperationReply::Await`, serialize terminal Tokenizer results,
decode arbitrary continuations, or restore a new session from received state.

The original limits, observed history and session identity are checked before
polling an admitted operation's Budget. Malformed preflight or foreign-session
rejection cannot consume another pending slot even with a stopped unrelated
Budget. Exact comparison and encoding are metered. Failed export retains pending;
an eligible shared-operation stop on resume uses the existing terminal cleanup
and preserves accepted collector artifacts. Saved Report/Usage remain unchanged
by repeated exports, and their diagnostics/events are not emitted again.

`TokenizationSession::pending_read` and `pending_transform` borrow the actual
nested Reader dispatch for existing terminal reply codecs. Read includes Dependent
calls. The tokenizer checks Closed, NoPending and reservation waits before
forwarding; the inner reader rejects the wrong provider kind. The context retains
inner request/state, source closure, Usage, view offset and provider depth, which
may differ from the later outer checkpoint. A codec error or stop alone does not
consume pending. The caller still resumes, discards or closes through the native
lifecycle. A borrowed context cannot remain usable across mutable session calls.

Run the saved-state, envelope, ownership and codec integration fixtures with:

```sh
cargo test --locked -p nepl3-reader --test runtime echo::
cargo test --locked -p nepl3-reader --test builtin echo::
cargo test --locked -p nepl3-reader --doc
```

These include three provider kinds, canonical source tables, mapped nested views,
distinct inner/outer checkpoints, reply-only export exhaustion, retained collectors,
foreign/fresh budget rejection, report mismatch, dispatch rejection without a
fabricated domain partial, and borrowed-context lifetimes. They are scoped tests,
not formal Reader transport acceptance.

### Native child execution through Tokenizer replies

`cargo test --locked -p nepl3-provider --test tokenizer` composes a saved Tokenizer
Await with the actual `builtinName` sender and `IssuedInvocation::execute_local_child`.
The host builds the Invoke from the exact pending call request, after a leading
trivia prefix; it does not substitute the outer tokenizer request. The callback
uses its supplied execution Budget and separate SourceAdmission. The receiver
uses the tokenizer's borrowed inner context and resumes through either complete
continuation or full-reply echo after a terminal OperationReply CBOR roundtrip.

The fixture distinguishes inner saved Usage, later outer saved Usage and the
actual child execution basis. In the separate-budget compatibility case,
successful Grants/issue validation uses a finite separate Budget, whose measured
Usage is recorded once with `run_local` under the
local ceiling before capturing the child basis. This includes context_digest's
own source admission: the parent input, validation and child account for three
explicit admissions in this fixture. Settlement records seven additive differences
and absolute peak depth; the child's cumulative Report is never rebased or rewritten.

The WorkLimit case calibrates the same input independently, then runs the real
builtin with one less Work unit. It requires an actual issued Stopped reply, not a
setup error or fabricated stop. The parent retains capacity for encoding,
validation and resume. Both successful Token and terminal WorkLimit preserve
accepted trivia, report the final parent Usage, consume both pending slots, and
allow a new operation. Tampered reports/echoes are rejected before retrying with
the same retained execution result; serialization retry does not rerun the child.

This is trusted local Read/builtinName evidence. Fixture construction and
adversarial clones are not a proof of complete host cost accounting. For the
separate-budget compatibility path, failed validation/issue and failed retrospective
charging still need their own accounting policy. Reserve, actual Transform/Dependent execution, general dispatch, remote
request/attempt association, authenticated metering, remote source admission,
process cleanup and formal cross-target acceptance remain separate work.

### Parent-budget invocation validation

`IssuedInvocation::issue_with_parent_validation` is an optional host entry that
charges input validation and context hashing directly to the actual parent
Budget before reserving the unchanged relative child grant. Successful charges
remain on malformed-input/context errors and resource stops. A rejected charge
itself is not recorded, and this is not physical CPU or complete host cost
measurement. Validation failure creates no outstanding reservation. If validation
succeeds but the requested grant no longer fits, issuance stops without reducing
the grant or refunding validation work.

The existing `issue(..., validation)` API retains its separate finite validation
Budget and previous error ordering. Both paths preserve Input, Context and
reservation/Stopped error stages. The new path uses current parent depth and
retains historical peak; successful issuance may still be followed by local-child
setup DepthLimit when its grant cannot cover that peak. `context_digest` retains
its separate source-admission ledger even when its counters belong to the parent.

Grants construction/admission and Invoke construction are outside this new method.
The parent-validation Tokenizer fixture passes the actual parent to Grants as well,
then uses an independent validation oracle without charging it again. The older
successful post-charge fixture remains as a compatibility control. Both echo forms
and actual builtin WorkLimit still roundtrip and resume the same saved state.

```sh
cargo test --locked -p nepl3-provider --test transport reply::delegation
cargo test --locked -p nepl3-provider --test tokenizer
```

The parent-validation cases cover direct-cost comparison at nonzero depth,
malformed/unknown input before reservation, input/context quota stops, seven-resource
remaining-capacity boundaries, zero/maximum grants, ancestor limits, child historical
depth rejection, and native outer-scope unwind before/after issuance. The unwind
fixture is excluded on panic-abort targets. Remote observation trust, child-unwind
costs that were never settled, other unmetered host work and formal acceptance
remain outside this entry point.

### Native Article Code/Math composition prerequisite

The selected native guest route accepts Code, InlineMath and DisplayMath only;
local-only and Code-only preparation retain their previous resolution requirements.
The host receives immutable typed embeds and chooses actual renderers. Display
Math imports are checked as block content; inline Math and Code remain phrasing.
Per-occurrence placement and final complete HTML validation remain mandatory.

Structural Code and DisplayMath wrappers are constructed before host callbacks.
Their actual insertion-parent depth therefore also bounds temporary renderer
traversals, even if the returned markup is shallow. This intentionally tightens
near-limit Code callback depth, and wrapper charges may precede a host error.
Sticky resource stops take precedence over callback fallback/error values.

This native composition prerequisite does not add standalone CLI Math selection,
KaTeX execution, pages/SVG Math resolution, browser verification or formal spec17
acceptance. Those later host steps must retain explicit renderer policy and
fallback reasons rather than silently changing structural Math to text.

### Standalone MathML export host

`doc-html export` now selects the native Code/Math Article route. The CLI accepts
`--math-renderer katex-preferred|mathml-only` with either CSS packaging mode; the
default is `katex-preferred`. No qualified KaTeX export adapter is configured in
this host yet. Preferred mode therefore records a typed capability-unavailable
notice for each rendered Math occurrence, charges its diagnostic count, and uses
the independent MathML backend. Explicit `mathml-only` invokes neither KaTeX nor
TeX preparation and records no capability-failure notice. Neither path evaluates
Math. Invalid input, unsafe markup, cancellation and resource stops remain errors;
no generic catch converts them to successful fallback.

The manifest records the preference, actual representation, occurrence/embed
selection and fallback reason alongside source/profile identity and output-file
hashes. These are local host export notices, not authenticated remote provider
Reports. The completed CLI also prints a capability notice when preferred mode
used MathML. Code guests still render retained source through the existing checked
highlighting adapter. HTML remains script-free, with the existing CSP, stylesheet
hashes, render-before-write and no-overwrite rules.

The subsequent SVG and page-set sections describe their separate composition
steps. Standalone generation does not qualify a KaTeX adapter, certify remote
renderer identity, or establish browser/spec17 acceptance. MathML display still depends on the viewing browser; actual browser
coverage must be reported separately from structural generation tests.

### Selected Math with static SVG export

The new opaque SVG+guests preparation admits Code, InlineMath and DisplayMath
under a private three-way guest policy. Existing SVG-only and SVG+Code APIs keep
their old signatures and unsupported-requirement failures. Raw SVG validation,
asset admission, digest checks, alt extraction, and all nonselected requirements
remain unchanged.

`doc-html svg` uses the same default/explicit Math renderer policy and shared
callback as standalone export, with optional `--math-renderer` after `--svg`.
Its manifest retains both typed Math notices and the SVG resource list. Same-byte
assets can have distinct IDs but share one external file; embedded image
occurrences remain separate outputs. All phases retain the same output budget,
CSP and no-overwrite/render-before-write behavior. MathML requires no additional
script or image permissions.

Inline CSS plus embedded SVG makes document content single-file. Fonts remain
unbundled, with the existing documented system fallback offline. This step does
not establish actual-browser rendering, KaTeX qualification, page-set Math
composition or formal acceptance.

### Native page-set Math composition

`doc-html pages` now uses the explicit native `render_pages_with_guests` route.
The input manifest accepts `math_renderer: "katex-preferred" | "mathml-only"`,
with the same preferred default and explicit unavailable-adapter notices as
standalone export. Existing core `render_pages` and `render_pages_with_code`
retain their original rejection of Math; portable replay is not widened.

The resolver and each Math callback share one codec/source-admission ledger and
one output budget. Sentence annotation HTML input validation also uses this
ledger; a sealed, borrowed syntax proof releases the mutable ledger borrow before
nested guest callbacks. Math renders at its actual insertion depth; Code retains its
prepared syntax-only highlighting. The complete page set resolves before any
callback. Unsupported assets/foreign requirements still fail, and imported DOM
IDs cannot manufacture a hidden Doc anchor, including HTML inside MathML.

Reports bind each page index, ID, logical source and output route to its selected
policy and Math occurrences. Guest ordinals restart on each page and count Code
as well as Math, so Code can leave gaps in Math records. The semantic page-set,
output-budget and parse/lower identities keep their existing recipes. A separate
`nepl3.local-doc-pages.mathml-host/1` identity binds the output execution digest,
policy and unavailable KaTeX capability. This is a local host recipe, not an
authenticated remote execution receipt.

File hashes, source/profile identities, aliases, CSP and all-or-error generation
before filesystem writes remain in effect. Capability notices are emitted only
after the completion marker and refer to the Doc export manifest, which site
composition retains as `doc-manifest.json`. This change does not implement
page-set SVG assets, qualify KaTeX, prove browser rendering or complete spec17.


### Sentence annotation admission repair

Sentence HTML exposes native `render_checked_with_foreign` for an immutable
`CheckedSyntax` produced in the same operation. Existing validating entry points
still validate raw Sentence syntax, then use the shared checked builder. The
checked route does not bypass markup, phrasing, DOM identity, callback-depth or
final output checks. It is not a portable proof or authority to skip source
admission in a fresh operation. Validation must also be repeated on entering a
deeper caller context or changing the admission scope; the proof retains neither
validation depth nor budget/admission identity.

Math annotations validate through their existing codec/admission before the
callback borrows that codec again. This removes the former fresh HTML-validation
ledger, which charged an already admitted snapshot again while retaining the
same Budget. No usage is subtracted or reset; newly encountered sources and all
validation work remain charged, and stops retain their original precedence.

### Retained native visual parts

The internal KaTeX finite-tree boundary additionally offers `prepare`, returning
opaque `PreparedVisual` content with owned nodes and an exact copied host class
inventory/scope. It accepts the same bounded JSON shape and rejects malformed
content before returning. It performs no renderer execution or output emission.

Serialization rebuilds a metered borrowed policy view and validates the retained
immutable tree again at the caller's actual depth and budget. Repeated output
remains charged. The old `render` still directly decodes, validates and serializes;
it does not pay the new ownership or duplicate-validation costs.

This is a prerequisite for a future prepared Math artifact, not that artifact.
Policy classes are not authenticated asset bytes, lexical scope validity is not
document-wide uniqueness, and the retained object establishes neither source or
renderer identity, visual fidelity, accessible MathML composition nor Doc/portable
HTML admission. It has no raw-HTML conversion, mutable getter, unbudgeted Clone or
Deserialize constructor. A containing artifact must separately bind its fixed
CSS/fonts/license bytes, independently generated MathML and supervised execution.


### Core-owned visual preparation

The owned native visual implementation now lives in no_std
`nepl3_markup::katex::fragment::PreparedVisual`; the tools JSON boundary delegates
through its existing wrapper and maps stopped errors without changing their kind.
No core dependency on tools, HtmlRequest variant or portable schema is added.

The constructor moves an already admitted native Fragment and meters new policy
copies. Its input buffers may have preexisting spare capacity: this is ownership,
not a fresh retained-memory bound. Each serialization rebuilds the metered policy
view and revalidates at the current depth. Core tests cover boundary/stop behavior
and compile-fail mutation/clone attempts. Asset/class-catalog binding, executing
identity, same-Math pairing and actual Doc insertion remain distinct unfinished
work; this relocation does not mark T24 or browser acceptance complete.

### Host-selected Dependent integration fixture

The provider Tokenizer test now also selects a test-only `readTail` Dependent
signature. A Scalar consumes `a` in ` a変数 ` before Then suspends; the retained
outer start/trivia are distinct from the nested request starting at byte 2.
The fixture authorizes the declared source table, decodes the Dependent input
against that signature, and invokes the production builtin Name reader on the
nested request. It does not pass a Dependent operation to the builtinName sender.

The parent-validation path covers success and a calibrated actual builtin
WorkLimit with both saved continuation and full-reply echoes. Both paths retain
seven-resource settlement, absolute depth, independent source admission,
OperationReply CBOR, tamper rejection/retry and a new operation after consumption.
The helper returns decoding/context errors as errors; a successful test expecting
ReadReply::Stopped therefore cannot silently substitute a pre-execution failure.
The actual result's cumulative usage stays fixed while later serialization and
resume costs accrue on the parent.

This is a test-only host handler coupled to a real lexical reader, not a standard
Dependent executor, common Tokenizer operation, remote metering protocol or
proof that every fixture allocation/assertion is accounted. Generated source,
Transform, Reserve and general handler failure recovery remain separate scopes.

### Exact saved Invoke comparison before settlement

`Invoke::check_saved` compares request ID, operation identity, input, environment,
Limits and ordered source/resource tables against the host's retained Invoke.
It uses the existing budgeted TypedValue and SourceSnapshot comparisons and
charges resource IDs/digests/bytes before comparison. This comparison admits no
source bytes and grants no authority. Even equally malformed values may compare
equal; ordinary schema validation and authorization are still required.

`IssuedInvocation::settle_saved_request` optionally runs that check under the
actual parent's unreserved local ceiling before the existing identity checks and
one-time settlement. Successful comparison costs stay charged on mismatch or
stop. A failed request/identity check or an observation exceeding the grant records
no remote Usage;
the unresolved guard cancels its parent without replacing an earlier stop.
The new entry has a separate `SavedSettlementError`; the existing `Error` enum
is unchanged. The unchanged `settle` entry remains available for an independently verified
observation after a parent stop. The new entry cannot perform fresh comparison
on an already stopped parent, and does not silently bypass that verification.

Exact content equality does not connect that content to a trusted measurement
channel or distinguish repeated identical requests, reconnections or attempts.
The host must establish those associations and terminal cleanup independently.
The context digest retains its existing cycle-detection semantics; no wire
record, remote inherited Budget transport, task status or formal acceptance
result changes in this slice.

### One-budget reply reception under a reserved parent

`Connection::receive_reply_with_budget` uses one Budget sequentially for frame
reception/decoding and semantic reply validation. The original `receive_reply`
keeps separate transport/validation parameters and shares the same frame and
request-ID checks. Transport stops remain `ReplyError::Transport`; semantic
validation stops remain `ReplyError::Validation`. Either failure closes the
connection and retains already charged work; no success is inferred from an
unverified Report.

The reserved-transport fixture sends an actual Invoke, receives and validates
its response inside `IssuedInvocation::run_local`, and finally calls
`settle_saved_request` with independently observed local fixture execution Usage.
A direct send/receive/validate sequence is its full-Usage oracle, and decoding the
sent bytes checks the original Invoke. The returned Report deliberately claims
maximal Work while settlement uses only the separately held actual observation.
Fixture construction and its separately budgeted initial issuance validation are
outside this combined transport scope; the newer parent-validation issuance API
can be selected by a host independently.

The fixture covers Work stopping during send, receive/codec, and semantic
validation, with full failed-prefix Usage compared to a direct same-ceiling
oracle for those stops and non-stopping reply rejections. It also covers OutputLimit before any writer bytes, BrokenPipe, truncated input,
wrong request ID and non-stopping output-contract failure. It checks retained
local costs, parent-limit restoration, suppression of callbacks after a stop,
and cancellation when an unverified outstanding grant is dropped. Terminal/Await
success, invalid Await context, Close and EOF retain the legacy entry's outcomes.
No actual process is launched by these tests: process interruption/reaping,
request-lifetime management, remote measurement authentication and attempt
association remain explicit host responsibilities. The new method does not
perform reservation or settlement by itself.

### Reserved-parent cleanup after a real process deadline

The native process harness keeps the original Silent/Partial blocked-read cases
and adds two reservation cases. A host worker creates an admitted invocation,
registers its lifetime, and sends the actual Invoke under the parent's local
ceiling before signalling readiness. The fixture child consumes stdin without
executing that operation and then stays blocked: this is intentionally not a
successful remote provider execution. The host forces termination and verifies
reaping plus stable repeated exit-status retrieval.

The worker's one-budget receive ends as Closed or Truncated. A counted reader
records every read attempt (including EOF/Interrupted) and received bytes after
the private startup marker. A Cursor over the identical empty/truncated fixture
is the oracle from the same pre-receive Usage and local ceiling. Only the Work
charged once per read call is adjusted by the measured call-count difference;
all other Usage fields and received byte counts must match. This avoids assuming
that pipe reads have Cursor's chunk boundaries.

The tests close request lifetimes exactly once, omit already finished entries,
reject registration after close, and drop the unresolved reservation. They check
Cancelled, unchanged consumed Usage, restored parent Limits, and refusal of an
additional charge or reservation. Forced process exit does not prove unused or
zero remote capacity: no observation is invented and no settlement is attempted.
The combined success of worker checks and the main thread's cleanup establishes
this fixture's exit/reap/cancellation path. It does not establish remote metering,
process-tree containment, inherited-accounting transport or attempt provenance.
These OS-process cases are explicitly skipped on WASI, not counted as WASI passes.

### Doc Profile resolution uses the actual native callback catalog

The standard Doc named-input pipeline constructs its real NativeHost before
resolving the ParseProfile. Package reader signatures still determine the
requirements and allowlist; they no longer manufacture matching host
ProviderImplementation entries. A metered independent catalog copy comes from
NativeHost::provider_catalog, and the same host instance subsequently services
the parser. The copy avoids extending an immutable borrow across mutable dispatch
and charges Work/Allocation before cloning its strings and operation records.

A regression adds a valid but unused reader operation to the package and registry
without registering a callback. Previously that source parsed successfully;
both explicit Await/resume and native-host routes now reject it as MissingProvider during
Profile resolution. Positive Doc/Sentence/Math input, changed implementation
identity, independent catalog storage and exhausted/cancelled preparation budgets
are tested separately. The extra catalog-copy cost belongs to the selected parse
Budget. Existing package/profile preparation still has its own finite host budgets;
this change does not claim complete host-work accounting or full suite Profile
support, nor does it close R009 or any formal acceptance group.

The standalone Sentence source pipeline also uses `provider_catalog` instead of
an uncharged `to_vec` snapshot. Its regression selects an Allocation ceiling
that admits only the existing host preparation, then compares the copy-stage
stop and complete Usage prefix against an independently prepared host/catalog
oracle. Both owned Await and native-host routes must stop before finishing the
parse, while ordinary input still succeeds. This closes that catalog-copy gap;
it is not a claim that every Profile-construction allocation is now metered.

Native reader registration uses the resource-aware registry lookup before
accepting an operation. The selected package/revision must still have the exact
requested schema digest. A regression adds 32 unrelated schemas before the
registered reader and checks their search Work against an independent package-
length formula, then bounds Work before any provider allocation. This change
meters registry search only; it does not alter provider naming or add a new
Read-envelope validation boundary.

Native reader registration also checks the selected operation's nominal
`ReadRequest -> ReadReply` envelope before creating a provider entry. A native
Read callback cannot register Unit, Transform, differently named/revisioned
reader types or an Option/List wrapper as that envelope. The check uses the
already selected descriptor and charges comparison Work before inspecting names.
It leaves operation package/name and purity policy unchanged: an externally named
operation with the canonical Read envelope is accepted, including `pure: false`.
This registration check does not validate callback behavior or remote execution.

`ProviderSignature::check` also meters lookup of its operation schema and retains
exact digest identity before selecting the operation. Its regression keeps the
reader continuation schema ahead of added unrelated schemas, so the independent
Work delta isolates the operation lookup for Read, Transform and Dependent.
A low-Work check stops without allocating, and a mismatched digest remains a
signature error. This does not claim that every lookup inside the preceding
value-type validation is metered; the separate ReaderPlan schema lookup also
remains outside this change.

Named type validation now charges schema and type-name lookup Work before each
comparison. It reuses the selected descriptor instead of repeating an unmetered
lookup through `kind_id`. Primitive and wrapper traversal, Depth, nominal type
existence, and UnknownSchema/UnknownType/Unfinalized errors retain their roles;
validation does not traverse the referenced type's fields. A direct regression
checks complete Usage from package/name lengths, nested wrappers, search-stage
Work stops, cancelled prefix preservation and unknown identities. ReaderPlan's
separate schema lookup remains outside this change.

ReaderPlan's initial schema lookup now also uses metered selection and an exact
SchemaRef comparison. An unfinalized registry retains the existing UnknownSchema
result; a finalized registry must admit search Work before checking the identity.
Consequently, cancellation or exhausted Work on a finalized registry takes
precedence over discovering a missing or mismatched schema. This is an explicit
change from the former unmetered identity-first path.
Search-stage budget failures retain no expression attribution because expression
validation has not begun. A minimal Literal plan isolates unrelated-schema search
cost from provider and Named type validation, and checks preserved cancellation,
low-Work Usage, missing schemas and mismatched digests.

Portable terminal Read/Dependent and Transform reply codecs scope view validation
to the saved checkpoint's accepted SourceMaps plus the reply's mapping delta.
Transform decoding now reads mapping declarations before source-bearing views.
The temporary union is budgeted and never changes the serialized or returned
delta, the checkpoint or ambient codec authority. Native saved-dispatch checks
still validate replies before encoding and after decoding.

Regressions cover a host-parent/generated-child view using a new Transform map,
Read/Dependent/Transform views using an earlier accepted map without re-emitting
it, and independently encoded Foundation views inserted into schema-valid reply
values before CBOR decode. A missing reply map cannot be supplied by ambient codec
state; rejection leaves the pending dispatch available for a valid retry. These
checks establish these mapped-view paths, not all portable/native equivalence or
remote execution authentication.
The accepted-map regression also composes an earlier A-to-B map with a new B-to-C
map, rejects a B-to-A reply cycle without consuming the pending dispatch, and
then accepts the corrected reply. A private scope-copy test checks the existing
logical CopyCost formula and partial Work/Allocation stops before cloning;
immutable Span identity storage may still be shared.

Structural value validation now meters candidate searches for selected schemas,
named type definitions and variant alternatives. This covers `validate` with a
Named expectation and the NdfValue/TypedValue paths that resolve an actual typed
record or variant. Identity comparisons are charged before equality. The existing
error distinction is retained: a Named payload with the wrong identity is
WrongType, while a dynamic payload with an unknown schema identity is
UnknownSchema. Type/variant lookup order and field-count checks are unchanged
when sufficient Work is available.

Independent padding regressions vary schema, type and variant counts separately,
using empty-field values to isolate lookup Work. They cover Record/Variant across
all three expectations, sticky stops and identity/error cases. Existing traversal
retains Work-before-frontier, Nodes and inherited Depth behavior; lookup itself
does not allocate or add Nodes/Depth. The separate borrowed `validate_typed`
lookup and other unmetered low-level accessors are outside this change.

The Head transport fixtures' former 20k/100k Work reserves no longer admitted
these newly metered structural walks. A diagnostic run measured 197608 Work for
packet encoding and 4406 for local CBOR framing. Test-only reserves now allow
210k for that send path and 500k for a round trip; parent caps and the original
500k total-cap regression remain unchanged. The repeated immutable projection
count is checked against remaining Work divided by an isolated per-call cost,
instead of an arbitrary minimum iteration count. Grant and settlement assertions
are retained; no production limit is raised by this fixture adjustment.
The 2048-wrapper symbolic-type roundtrip fixture measured 10,845,179 Work after
lookup accounting, exceeding its former incidental 10m success allowance.
Only that success fixture now allows 12m Work; its depth bound, 100k-wrapper
cleanup case and explicit depth-64 rejection remain unchanged.

Borrowed typed-payload validation now uses the same metered schema/type/variant
selection without cloning the payload. Its root Nodes/Depth and field traversal
remain after successful lookup and field-count checks. Unfinalized still wins
before any Budget access; on a finalized registry, an exhausted/cancelled Budget
now stops before an invalid identity can be discovered. `validate_typed_as`
retains its earlier poll and expected-type admission before actual validation.

Borrowed regressions isolate candidate-count deltas for Record and Variant,
including the extra expected-type lookup in the Named `validate_typed_as` route.
They check zero allocation for empty payloads, parent Depth, sticky lookup stops,
and identity/type/variant/field-count errors with sufficient Work.

Registry type and variant tables are normalized by `register`; lookup now uses
binary search while charging before every comparison. Reversed-input tests cover
all registered positions and missing names before, between and after entries.
This also lets the symbolic roundtrip fixture return to its original 10m Work
allowance. The previously measured linear-search costs above describe that
intermediate implementation, not a guarantee about later lookup algorithms.

The old 100m desktop Doc preparation allowance still stopped adopted chapters
after binary lookup. An explicit measurement run found parse/tree-check Work of
550,404,329 (05-document), 562,624,600 (08-editor), and 764,350,502 (08-completion).
The desktop development host now selects a finite 1b Work allowance before the
operation starts and binds it into the ParseProfile. This shared host preset also
supplies default lower/output phase Work allowances; each becomes 1b, not a claim
of one combined document-wide budget. SourceBytes, Nodes, Depth, AllocationUnits,
OutputBytes, Diagnostics and Events limits are unchanged. Explicit caller/phase
limits, core Budget behavior and Head parent caps are
not increased or retried automatically. This host-policy recalibration is
separate from runtime acceptance.

The canonical desktop batch policy is selected separately in `doc/canonical.json`.
The 34-page Markdown regression measured 3,803,059,631 Work including its final
projections; the complete HTML generation measured 3,417,820,807 Work and
2,306,996,251 AllocationUnits. That checkpoint selected finite 4b Work for both
batches and 2.5b HTML AllocationUnits. The other batch resources remain unchanged.
The former HTML 2.3b AllocationUnits allowance stopped during serialization;
this measurement does not establish that lookup accounting caused allocation
growth. These configured allowances are fixed before a batch starts, not an
automatic retry or an expansion of caller-provided limits. Temporary measurement
prints are not part of production output. Generation success does not establish
browser rendering or formal runtime acceptance.

Reader NoMatch/NeedMore provider expectations now charge schema lookup, exact
identity comparison and each operation-name comparison before inspecting the
candidate. Missing operation and invalid argument/schema errors remain distinct
with sufficient Work. A stopped Budget wins before an unperformed lookup.
Tests derive missing-name search cost from independent candidate lengths and
cover scan-boundary stops, cancellation, exact identities, argument validation
and both externally resumed outcomes. This does not reinterpret an expectation
as an invocation or change its existing argument-type contract.

Diagnostic and Event metadata admission now meters selection of the metadata's
exact schema independently of typed argument/payload validation. The existing
metadata-header charge and empty code/stage/kind priority remain; schema search
and exact identity checks are additionally charged before comparison. A missing
or mismatched metadata schema remains Metadata with sufficient Work. Shared
Report admission tests exercise this same path and preserve remote-usage
non-absorption and local Diagnostics/Events counters. Registry-padding tests put
the argument schema first so that the owner-schema lookup cost is isolated.

Tokenizer mode admission now precharges each skip/take entry, visited rule name,
token-kind schema candidate and exact schema identity. Duplicate-mode
comparisons additionally charge both name byte lengths before comparing them;
the previous index-based overhead remains. These checks are shared by native
session creation and portable mode encoding/decoding. Unknown rule/schema/kind
errors retain their classification with sufficient Work; sticky budget stops
precede searches that cannot be afforded. Tests isolate long common-prefix
comparisons and unrelated registry padding, then exercise valid roundtrips and
native/portable cancellation. This scope does not claim all later tokenizer
runtime lookups have been audited or close formal acceptance findings.

Fresh tokenizer request admission now meters the builtin KindRef check and
requested-mode selection before comparing schema/name bytes. The configuration
and request kind checks share one implementation without changing their fees.
Lookup exhaustion returns the normal Stopped outcome with the accepted prefix;
semantic lookup errors remain recoverable. The existing raw Closed/Busy checks
and accepted-budget provenance checks retain their order. Tests use explicit
scope cost, registry/mode padding, long prefixes and partial-search cutoffs, and
retain a native prefix containing a generated source and two mappings. Pending
portable echo/resume paths are not a fresh TokenizationRequest decoder and are
not claimed as coverage of this entry path.

Tokenizer restoration now meters the foundation-schema selection and saved-mode
search again for each reserve/resume operation. Construction and echo fees do
not prepay these catalog traversals. The existing restoration transaction keeps
pending on hard errors and consumes it on a resource stop while preserving the
accepted collector. Regression tests independently isolate 32 unused modes and
32 schemas preceding foundation. Public reserve/resume cutoff tests distinguish
the new lookup stop from a later stop by exact Work and allocation deltas, retain
generated source/maps/trivia, and reject reuse of the consumed pending slot.
Missing-source and wrong-provider-kind rejections retain retryable pending state.

Reader-context admission now uses a borrowed, budgeted exact-schema descriptor
lookup for context and namespace identities. A checked digest/source-closure
proof does not prove membership in a different operation registry. Runtime
requests additionally retain the explicit selected foundation@1 check and charge
its exact identity comparison. Missing context/foundation and namespace errors
keep their existing classifications; source admission and digest validation
remain in their prior order.

Both retarget APIs charge these lookups before copying. The preserving variant's
old schema-length prepayments are replaced by entry units plus actual lookup
fees, rather than charging identity comparisons twice. Their copy stage charges
the new schema/category/mode strings and the separate foundation package clone;
allocation and source-closure charges remain unchanged. Tests isolate missing
context/namespace scans, prove that a context checked under one registry is not
silently accepted by another, retain digest/source-closure regressions, and check
that lookup exhaustion is a normal reader stop without refunding SourceBytes.

ReaderFact Presentation and Relation admission now uses the shared budgeted
exact-schema lookup after the existing nonempty-name check. Unknown identities
remain ProviderContract; class/relation vocabulary and span/source-map authority
are unchanged. Public resume tests compare empty-name rejection against missing
or wrong-digest schema rejection on the same pending slot, independently deriving
the extra catalog Work. Corrected metadata with arbitrary nonempty vocabulary
remains accepted. A lookup-specific cutoff retains the previously accepted
source/map/diagnostic/event collector and consumes pending without publishing the
rejected facts.

Shared ViewBundle and Token admission now meters exact kind-schema lookup, plus
role and relation schema lookup. Kind finalization and local-kind bounds retain
their order; empty vocabulary and relation-target checks still precede metadata
lookup. No new global finalization requirement is imposed on an empty view.
Core regressions isolate one late-owner lookup in each of the four paths and
check complete Usage deltas, exact-identity failures, lookup-specific stops and
sticky cancellation. Geometry, child cycles, semantic relation cycles and the
unrestricted nonempty presentation vocabulary remain unchanged.

After View/Token lookup accounting, the same 34-page Markdown gate measured
4,089,795,581 Work and exceeded the former 4b allowance. Its configured Work
allowance is now finite 4.5b (the measurement temporarily used 5b); all other
Markdown resource limits are unchanged. The HTML corpus separately completed
under its unchanged 4b Work / 2.5b AllocationUnits policy, using 3,652,336,867 Work
and 2,306,996,251 AllocationUnits. This is an explicit development-host batch
policy update based on the adopted corpus, not an automatic retry or expansion
of caller-supplied/core budgets. Neither generation result is formal acceptance.
