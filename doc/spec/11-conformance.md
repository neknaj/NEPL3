# 11. 受入条件と検証

## 方針

仕様・データ・実装・実行結果を区別する。schemaの形式整合だけで言語が完成したことにしない。正例、誤例、境界、変換前後、異なる実装の比較を受入条件とする。

## 1. 必須の試験群

G01: Grammar自身をseedで読み、compileしたpackageとseedの意味正規形が一致。
G02: declarative readerとRust direct readerの結果・消費範囲・viewが一致。
G03: 空反復、進捗なし再帰、未定義reader、shape衝突、provider署名違反を正しいcodeで拒否。
G04: quoted属性内の `>` を含むAngleTag、動的delimiter、部分入力のNeedMore、commit/no-matchの差を検査。
G05: native provider / NDF loopback / 別process providerから同じ結果。

P01: prefixのserialize/parseで木が同じ。cons/nilを含む全constructorを網羅。Numberの有限十進制約、任意有理数からの式構築、著者のFrac保存も検査する。
P02: Doc→Math→DocとDoc→Circuitの復帰直後のhost tokenを読み過ぎない。
P03: headのarityを既読の子の値で変更できない。局所schema更新providerは既読情報だけ利用。
P04: Recover parseがMissing/Unparsedを保持し、未知arityを0としない。

D01: sentence literalとprefix構築が意味正規形で等しい。
D02: ネストしたruby/anno、多段note、全escape、空part、閉じ忘れを検査。
D03: paragraphの深さとparallelのsentence対応が独立。重複languageとparagraph variantは拒否。
D04: 前方label、重複label、未定義ref、renameの捕捉検査。
D05: HTMLにユーザー由来scriptが出ず、Doc paragraphのネストが正しい構造で出力される。Markup文字集合違反の拒否、]]>のescape、CRと属性TAB/LF/CRの保存を検査する。

M01: 1/2+1/3=5/6、0.1+0.2=3/10、大整数を丸めず計算。
M02: matrix形状違反、次元不一致、0除算、非整数指数等を仕様通り分類。
M03: let/sumのscope、外側と内側の同名、free symbol、notation-onlyのSymbolic。
M04: すべてのMath constructorをMathMLへ描画。弱い子の括弧、sub/powの結合、source式保存。mspaceの単位付き非負em長さと無効値、Markupの文字制約とXML escapeを検査する。

C01: half-adderを全4入力、adderは小幅の全入力で確認。
C02: 複数stateの同時更新、testごとのreset、pre-edge出力を検査。initialはPreparedNetlistを受けてslot順の初期値を返す。
C03: 未定義signal、重複driver、幅違反、組合せloop、再帰instantiationを拒否。
C04: child moduleのstateを介したfeedbackを組合せloopとして誤拒否しない。
C05: instanceを2個作ればstateが独立。wire参照は共有。
C06: vector evaluatorと独立したNOR evaluatorの複数tick一致。4種のnodeとnextBits/outputBitsのsink参照、長さ・bit順を検査する。

E01: 日本語/補助平面文字/CRLFにおけるUTF-8/16/32の位置変換。
E02: SentenceLiteralの内部を正確にハイライトし、外側では1tokenを維持。
E03: definitionの全範囲とname範囲、対応文関係とdefinitionの区別。
E04: 更新後に古いdiagnostic/renameを適用しない。
E05: 増分解析と全解析で意味・診断・参照・位置が一致。
E06: code表示が不正/非停止のguestをlower・意味check・evaluateしない。Doc自身のCodeもForeignSyntaxのまま表示する。

W01: NDFの全variant roundtrip、未知tag、非canonical整数、負zero、分母0、壊れた参照の拒否。field arrayの順序交換でschema digestが変わり、名前付きvariant mapのkey順だけの交換では変わらないことを検査する。
W02: nativeとwire経路で一致。Complete/Invalid/Stopped/Awaitを網羅。
W03: continuationの誤用、未知operation、schema mismatch、過大frameを拒否。

A01: workspace DAG、依存許可集合、coreのno_stdを検査。
A02: native x86_64/aarch64、wasm32-wasip2、wasm32-unknown-unknownでbuildし、利用可能なrunnerで同一goldenを実行。
A03: runner不在を「テスト成功」にしない。CIの対応runnerを設定して完了にする。
A04: typed errorsのcatalog、診断stage/位置、resource上限とcancelを検査。sourceBytes超過はUTF-8 byte数で判定しStopped(SourceLimit)を返す。

U01: TEAのinit/update/view/subscriptionsが決定的で作用を直接実行せず、native/WASI/browserおよび別実装replayでModel/Cmd/Viewの意味が一致する。
U02: 全要求identityの照合により応答逆転、close/reopen、profile/provider/options/resources変更後の旧結果を拒否する。
U03: cancel、停止期限、Worker terminate/recreate、epoch更新と再準備を検査。終了後の完了応答を採用しない。
U04: document閉鎖・再読込、worker/session epoch、listener/timer再登録と解除で旧処理や二重購読が漏れない。
U05: 日本語IME、補助平面文字、CRLF、選択、undo/redo、format/rename transaction、programmatic edit feedbackとsnapshot競合で入力を失わない。
U06: Grammarと対象DSLの2editorを提供し、追加の文法・束縛・表示定義だけで共通highlight/definition等を得る。TypeScript再実装を不要にし、外部provider不足は明示する。
U07: preview隔離、未承認provider拒否、source表示からの非評価、保存・読込・downloadの成功/拒否/容量超過、未保存編集の保護を検査する。
U08: UI/Worker型のschema closure、native/Wasm/codecと別実装replayの対応、core/domain/engineへのUI依存逆流がないことを検査する。

S01: SiteConfigの /NEPL3/ と別の非root baseでトップ/docs/Playground/assets/Worker/Wasm/rustdoc/例manifestを生成・ロードし、deep linkと再読込を検査する。
S02: docs・例manifest・runtime・profile・assetsの版を照合する。cache混在、欠けた例、digest不一致、版違いを検出し、同一入力buildの決定性を検査する。
S03: 配信予定artifactを実browserで開き、4言語の代表操作、入出力、診断選択、editor query、停止を実行する。大きなsourceの応答性とkeyboard/focusも確認する。
S04: docs本文をJSなしで読み、4言語tutorial/reference/例から同じsourceをPlaygroundで開く。page/anchor/検索/linkとaccessibilityを検査し、自動検査と手動確認を区別する。
S05: 外部backend・localhost・CDNなしで基本4言語操作を行い、sourceの外部送信をせず、sandbox previewがscriptを実行しないことを検査する。
S06: 最小権限、同じ検査済みSHA/artifact、配信直列化とfreshness、失敗log保存を検査。実Pages公開後のHTTPS smokeとasset/build identityを照合して記録する。

J01: 全対象文書のinventoryとDoc表現gapを独立レビューし、表/list/link/汎用code/図の必要なschema・文法・backend・wire・conformanceを完成させる。
J02: 全移行ページを元の固定snapshotと比較し、意味・表・参照・数式・図・コードbyte列の同等性を独立に確認する。一つの正本と生成Markdownの差分検査を行う。
J03: Doc正本からの公開物で安定page ID/URL/anchor、検索、例revision、リンク、accessibilityを維持し、非root配信とbrowserで確認する。
J04: 旧版の検証済みrendererによる明示的な文書buildと現行runtime受入を区別し、bootstrap循環がないこと、全ページのDoc正本への切替、決定的生成、欠落・未対応時の停止を検査する。

必須群と必須targetの正本は `design/acceptance.json`。本文のID、catalog、implementation-statusの群集合を一致させ、T16は全required群を要求する。r3は55群だが、この数をcheckerの完了条件へ固定しない。nativeはLinux x86_64、Windows x86_64、macOS aarch64、WASIはwasm32-wasip2をWasmtimeで実行、browserは実Chromium/Firefox/WebKitを対象とし、正確な版をlogへ記録する。native process固有群はnative targetで検査する。WebKitの成功から実Safari device QAを推定しない。

## 2. propertyとfuzz

任意のvalidな有限prefix treeからprint→parse→lowerの意味一致。任意のUTF-8入力でpanicしない。case分割した入力streamと一括入力が同じ最終token列。消費範囲の単調性。Origin DAGの閉路なし。可逆なSourceMapだけがrenameを許す。小幅回路の原式とNORの一致。

fuzz入力でも上限を共有する。大量diagnosticを発生させる入力で無制限allocしない。OOMを完全に回避できると虚偽の保証をしない。

## 3. 不変条件のCI

`cargo fmt --check`、clippyの対象warningのdeny、workspace tests、doc tests、package生成の差分検査、dependency検査、target別check、conformance runner、fuzz smoke、license/asset/unsafe監査。

一つのarchitecture boundaryごとにtestがあり、再設計時に未更新の依存を検出できる。新しいconstructorを追加した際はparser、lower、check、print、render、wire、editorのcoverage表に行が増える。None/unsupportedで黙って網羅扱いにしない。

## 4. 開発の段階

順序はtasksで管理する。前段を先に完成させることは許可するが、未完の後段をその時点の「完成仕様」と呼ばない。最終受入は全必須試験に対する実行証拠が揃った時点。

`design/tasks.json` の各task.acceptanceは、そのタスクが寄与する試験群のcoverage参照である。試験群には後続タスクの責務も含むため、参照した群全体のpassedを前段タスクの完了条件にしない。T16以外のタスクのcompleteには、当該deliverableの実装、scopeを限定した検証証拠、依存タスクのcomplete、および関連する未解消の設計blockerがないことを要求する。証拠は `conformance/results/` 以下のJSONとし、`task_id`、`checks`（空でない文字列の非空list）、`commands`（同）、`targets`（同）、`result`（passed）、`excluded_acceptance_portions`（未検証範囲の文字列list、明示的な空listを許可）を持つ。implementation-statusの当該タスクからファイルを参照する。形だけの証拠ファイルではなく、記載したコマンドの実行結果をレビューする。

例えばT01のsource契約試験を実行してT01をcompleteにしても、E03/E04のエディタ操作を実装・検査していなければ当該群はnot-runのままとする。群全体のpassedは全要件の実行証拠がそろってから記録する。T16のcompleteには依存タスクの完了に加えて、task.acceptanceに記載した一部の群だけでなく登録された全必須群のpassedとその証拠を必須とする。

## 5. 群全体の型付き証拠

scope付きTaskEvidenceと群全体のAcceptanceEvidenceを分ける。群の証拠schemaは `interfaces/acceptance-evidence.schema.json`、配置は `conformance/results/`。`schema`、`acceptance_id`、`design_revision`、`identity`、`result`、`runs` を必須とする。identityはprofile `nepl3.repository-inputs/1` とsource_sha256/spec_sha256を持ち、現在の検査入力へ照合する。source identityの収集・除外規則は開発toolsのidentity操作と開発手順で固定する。

runはcatalogのtarget.kindに一致するtag付き型とする。`kind: command` はcommand、target、result、exit_code、非空checks、environment（runnerのname/versionとtoolsのname/version一覧）、log、log_sha256を持つ。`kind: review` はtarget、reviewer識別子、independent=true、decision（approved/rejected）、非空scope、log、log_sha256を持つ。意味同等性レビューのために架空のshell commandや終了コードを作らない。

logは `conformance/results/` 内の非空.txtまたは.logファイルとし、証拠JSON自身を実行logにしない。実在logとdigestを検査し、別ID、古いsource/spec/design、未登録target、target種別不一致、改変logを拒否する。passedには全command runがpassedかつexit_code=0、全reviewがapproved、全required targetの実行・レビューを要求する。failedは少なくとも一つの非zero終了の失敗runまたはrejected reviewを含み、早期停止による未実行targetを許すがpassedへ変更できない。未実行を空logやbuild成功で置き換えない。

証拠の形式・hash検査は記述された挙動の正しさそのものを証明しない。期待値の根拠、実行コマンドとlog、対象source、coverageを独立レビューする。source/doc仕様を変更した後は古い証拠でcurrent passedを維持せず再実行する。T16は全registered required群の型付き証拠を動的に確認する。

この設計パッケージの検査は別扱い。`doc/history/design-validation.json` に、構文例の構造検査、依存DAG、task ID、JSON、例の独立算術/回路検算などの実施範囲を記録する。Rust compiler/editor/browserが完成しているという証拠に使わない。
