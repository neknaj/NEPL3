# Doc runtime の段階実装

T07 は進行中。`doc/spec/05-document.md` と `design/forms.json` を最終契約とし、以下の native API ができたことを T07 全体の完了へ読み替えない。

## 現在状態を確認する入口

正本の一覧は `doc/canonical.json`、実装・受入の状態は `implementation-status.json`、
到達条件は `design/tasks.json` を参照する。以下の履歴にある「未完了」「停止」は、
その段階の結果であり、現在の残件を列挙したものではない。

第05章は通常のpage予算でparse/lowerと文書集合のHTML生成が成立し、正本登録済みである。
内容比較、旧見出しalias、17章・23章へのリンクを検査した移行結果はregistryから辿る。
Markdown集合のWork上限はページ追加に対応する別の実行設定であり、単体の性能保証ではない。
source集合の増大については、readerの競合検査とadmissionだけを実行する限定測定を追加した。
parser全体の実時間・実メモリは未測定であり、単体文書の予算適合やこの限定測定から推定しない。
次の性能改善では、以下の測定を基に既存集合の再走査と検証scopeの寿命を調べる。
受入済みcollectorの再admissionは、台帳の一致を確認できる同期readで再利用する。
同期hostもcallback境界ごとの交換検出を通す。SourceStoreを明示準備した経路では、
環境集合の所有付きscopeも再利用する。正常な同期readでは、collectorが保持する
環境検査済みprefixを引き継ぎ、追加sourceだけを検査する。resume経路は残件である。

Sentence consumerの所有移行を実装中である。Docの文章slotは独立Sentenceの
ForeignClosureを保持し、Articleの選択guestとnamespaceを明示して描画する。
Docのportable表現は共有provenanceをDocOwner表へ一度記録し、各DocClosureが
内容digestで参照する。同値の独立storageも同じportable byte列へ正規化する。
Foundationの単独ForeignClosure交換形式は維持する。

`16f6118` の128段落・512文・18,848 bytesの注釈付き入力は、既存の資源上限でHTML生成まで成功した。
parse/lower/prepare/renderの段階測定を `tools/tests/doc_capacity.rs` で行い、
本文とRubyの512件を確認する。prepareのWorkは90,419,873、描画までの累積Workは
92,233,883、累積AllocationUnitsは276,016,281である。AllocationUnitsは論理的な
累積使用量であり、ピーク物理メモリは未測定である。

PageNamespacePlanはページ・member・文書digest・リンク・残る要求を交換する。
受信側で再構築したnamespace proofから全fieldを照合し、出現順序・所属・添付ファイルの
変更や要求の欠落を拒否する。Doc coreの全46試験はnative・WASIで成功した。
旧ページ試験の型エラーは解消し、root検査と選択guestのnamespace検査へ責務を移した。
toolsのArticle・namespace・印字等の22試験と通常容量試験3件もnativeで成功した。
workspace全体のcompileはDoc HTMLの旧 `tests/local.rs` にある28件の型エラーで停止した。
正式Markdown生成は旧Sentence構文の `doc/tutorial/miniexpr.nepld` で停止している。
原稿・fixture・生成物の移行、portableページ描画、Mathを含む全体namespaceの接続を
継続する。T07/T21と正式受入は引き続き未完了である。

## 段階別の履歴

### 2026-09-21: collectorの環境検査済みprefixを保持

正常な同期readが返すcollectorに、開始時のsource長までの競合検査scopeを保存する。
新規生成sourceは次のreadで検査する。公開appendは追加分をこの証明へ含めず、
checkpointとrollbackは元の検査範囲を実source列と一緒に保持する。
別store・未準備store・raw復元・resume・停止ではこの省略を推定しない。
SourceChecksの補助cacheは検査済みの値列であり、collector内のindexではない。

限定測定`accepted_prefix_growth_measurement`はsourceを1件ずつ増やして競合検査だけを
128/256/512回行う。source構築・parser・admissionは含めない。
従来のWorkは8,384/33,152/131,840、prefix保持時は10,797/21,677/43,437だった。
128件では定数費用によりWorkが増えるが、増大入力の反復比較は線形に変わる。
Windows debugの参考時間は従来0.26/0.78/3.02 ms、prefix保持時0.046/0.073/0.147 ms。
これは全parserの実時間や実heapの改善率ではない。別分岐が同じ長さの場合、rollback後の
別suffix、環境競合、検査停止、実host生成直後の未検査suffixを回帰試験で確認した。

### 2026-09-21: 変更されていないSourceStoreの環境比較を再利用

SourceStoreのscopeは集合を構築した後に明示準備する。新規挿入と非空編集のcommitで
失効し、同値の重複挿入・空編集・途中停止では維持する。再準備しない場合と非atomic
targetは従来の内容比較を使う。台帳のscopeとは別であり、schemaや資源入場を証明しない。
共通開発hostのparse入口で準備し、tokenizerは同じ集合の反復比較だけを省く。

限定測定`environment_scope_growth_measurement`は128/256/512件の環境をそれぞれ
127/255/511回再比較する。初回の保存とfixture構築、parser、accepted sourceは含めない。
未準備時のWorkは16,256/65,280/261,632、準備時は0であり、各回のBudget pollは維持する。
Windows debugの参考実時間は未準備0.38/1.42/5.85 ms、準備0.011/0.016/0.033 msだった。
この測定は環境比較だけのもので、parser全体の性能・実heap削減・全source経路の線形化を
示さない。AllocationUnitsは実heap計測ではない。独立レビューで求められた編集commit
直前のAllocation停止も、集合とscopeの不変性を回帰試験で確認した。

### 2026-09-21: source集合の反復走査の基準測定

`tokenizer::source_checks::tests::source_scope_growth_measurement` は明示実行するignored testである。
固定32 sourceと、読取りごとに1 source増える入力を128/256/512回処理する。
測定対象のaccepted sourceは各1 KiB、固定environmentの32 sourceは各4 bytesである。
fixture構築・parser・I/Oを測定に含めない。
同じscope内で実際のSourceChecksとSourceAdmissionを再利用する。

| 増大入力の読取り回数 | admission走査回数 | 競合検査Work | admission Work |
| ---: | ---: | ---: | ---: |
| 128 | 8,256 | 32,960 | 144,462 |
| 256 | 32,896 | 82,304 | 477,190 |
| 512 | 131,328 | 230,144 | 1,714,046 |

Windows native releaseの一回の実測では、この二段階の合計時間は約0.17/0.57/2.25 msだった。
細粒度timerの費用を含む参考値であり、安定した性能閾値や処理系全体の二次時間を主張しない。
固定入力のadmission走査は4,096/8,192/16,384回である。
AllocationUnitsも出力するが契約上の課金値であり、実heap使用量ではない。
測定testはsource bytesの二重課金がないことも検査する。

この経路には増大する既存集合の二次的な走査が残る。検証省略の実装はまだ行っていない。
AcceptedTokenizationReportのscopeと長さだけでは、現在のSourceAdmissionでの受入済みを証明できない。
集合の分岐・rollback・別admission・環境変更を扱う失効条件を保った上で改善する。

### 2026-09-21: 同期collectorのadmission再利用

上記測定後、atomic pointerを持つtargetで、hostなし同期readが返す所有されたcollectorに
SourceAdmissionのopaque scopeを保持する経路を追加した。同じ台帳なら入場済みsourceの
再admissionを走査せず、別台帳では全sourceを検査する。source環境との競合検査は維持する。
checkpointとrollbackは実際のsource集合とscopeを一緒に保持する。
公開append/diagnosticへ別台帳を渡す場合は以前の証明を失効させる。

host callback中の台帳交換をまだ追跡しないため、host付きread・Await・Reserve・resume・
Stopped・raw復元には証明を付けない。非atomic targetも全検査を維持する。
したがって全source処理の線形化、Doc host全体の高速化、実heap削減が完了したとはしない。
次段階はcallback/resumeを含む台帳寿命と、残るSourceChecksの集合比較である。

### 2026-09-21: 同期hostの台帳交換を追跡

同期hostのprovider/reservation callbackを共通wrapperで囲み、各返却時点の台帳を
読取り開始時のscopeと照合する。失効は累積し、複数callbackでAからBへ交換した後に
Aへ戻っても証明を再発行しない。正常終端かつ台帳交換・host errorがない場合だけ、
host付きreadも受入済みcollectorの証明を保持する。

callback内部だけで台帳を交換して元へ戻した場合、返却sourceはruntimeによって
現在の台帳へ再検査・入場される。callback内部の任意状態を証明しているわけではない。
試験は二つのprovider呼出しでsourceを実際に生成し、A→B→Aで失効した後に
最初のsourceの再admissionが必要になることをSourceBytesで確認する。
reservationの交換、None、エラー返却でも失効を累積する。
resume/portableからは引き続き証明を発行しない。

実文書6件のparse/lower/labels試験も成功した。第05章はそれぞれ73,092,384 /
66,285,115 / 8,413,256 Workで、各段階の通常100,000,000 Work枠内だった。
未採用の第21章草案はparseが113,044,768 Workであり、明示した草案用枠での成功を
通常ページ枠への適合や正本移行完了とは扱わない。実時間の比較実験・実heap測定は別途必要である。

## #158を優先するSentence・注釈の回復

今後の是正順序は[23章](../spec/23-sentence-annotation.md)に従う。Doc固有機能を広げる前に、
Sentenceの独立言語化からDoc本文の文章境界を移行し、実文書性能とT19 docs-only Pagesを優先する。
NEPL3a完成・旧comment-as-triviaの全面移行でD本文・公開を止めない。これらは後続で完了させる。
以下の既存API・性能調査はその時点の実装記録であり、旧所有関係を維持する決定ではない。

最初の実装として`nepl3-sentence-core`を追加した。foundationだけに依存する`no_std + alloc`の
順序付き有限arena、category/参照/循環/到達性・非空文章部の検査と独立schemaを持つ。
descriptorは既存生成器で`interfaces/sentence.json`から生成し、実descriptorからidentityを計算する。
通常のURL構造とforeign参照を保持するが、guest実行やDの名前解決、安全なHTMLの認証はしない。

新規10試験は成功した。共有DAGの最長深さ、10万段の非再帰検査、資源停止、foreign参照共有・
未使用拒否、root category、schema所有者・revision・field順の境界を含む。
独立レビューで不足を指摘されたforeign/root試験を追加し、再レビューで指摘なしを確認した。
ARMv6-M、wasm32-unknown-unknown、wasm32-wasip2はbuild確認であり、target上の実行証拠ではない。

続いてSentenceValueのNDF codecを実装した。実descriptorの完全identity、受信前schema検査と
受信後arena検査、foundationによるforeign閉包検査を行う。手組みwire fixtureと実CBOR往復、
共有参照、Unicode、1万段の意味上の深さ、処理途中の非ゼロ予算停止、不正owner source/hashを検査する。
この値codecはSentence局所syntaxの位置情報・reader payloadを完成させるものではない。
codec追加後のSentence全19試験はnativeとWasmtime 44.0.1上のwasm32-wasip2で成功した。
ARMv6-Mとwasm32-unknown-unknownは引き続きbuild検査であり、browser実行は未検証である。
これは同じRust実装のtarget別検証で、独立した第2provider実装の適合を意味しない。
Source/Origin境界の準備中に、値codecで文章とforeignの深さを別々に検査していた欠陥を確認した。
最深の所有位置に親Budget深さを加えてguestを検査するよう修正した。文章42段・guest30段は
単独では上限60に収まるが、合成時は送受信とも拒否する。短経路から先に到達する共有guestでも
最長経路を使い、呼出元の深さを戻す。独立レビューと回帰試験で確認した。

続いてSentenceSyntaxの局所Source/Origin/View境界を追加した。意味arenaと同順・同長の位置表、
必須Origin、任意head/cover、Viewのowner、payload自身のsource/mapを両codec方向で検査する。
架空Spanを要求せず、生成値はSynthetic/Generated Originを使う。これは位置の閉包・関連付けの
検査であり、まだ未実装のreaderによる原文と意味値の一致を証明しない。

独立レビューで、明示SourceMapを持つViewの親子をnativeでは受理しportableでは拒否する欠陥を
確認し、foundation codecとSentence/Doc/Mathのsyntax境界を一緒に修正した。mapを各Viewで
再検査する初版は実14ページ文書でWorkLimitとなった。不変scopeに閉じた検査結果を再利用し、
既存上限のまま生成成功を確認した。各Viewは再検査し、scopeの再束縛とSourceAdmissionへの
可変アクセスでは親子双方のcacheを失効させる。scope越しのadmission置換を含む負例を維持する。
SentenceはCIのWASI実行・Wasm/ARM build対象にも追加した。
最終差分の全workspace試験は730成功・失敗0・既定ignore 2（所有continuation性能比較、
Node/npm依存KaTeX corpus）。Sentence/Wire/Doc/MathはWasmtime 44.0.1で134成功・失敗0・
ignore 0、ARMv6-Mとbrowser向けWasmのbuildも成功した。独立レビューの追加指摘を修正し、
再レビューで指摘なしを確認した。Sentenceのreader/LanguagePackageと注釈・consumer移行、
旧コメントの撤去は引き続き未完了である。

Sentence literalの解析本体と専用印字を独立coreへ移した。Doc型へ依存せず、Ruby/InlineAnno、
escape前のView、元Source/Originとdense位置表を保つ。Unicode escapeの途中、空注釈部、
不正delimiter、1,000段の入れ子、共有DAGの出力膨張、明示windowとCBORの往復を検査した。
literalで表現できないBreak等はNotLiteralとして返し、Textへ黙って変えない。
この追加後のSentence全33試験はnative/WASIで成功し、独立レビューで指摘なしを確認した。
LanguagePackage、reader/provider包絡、prefixと汎用印字は次の実装であり、D本文への接続を
NEPL3a完成より先行する。#165はCIと独立最終レビュー後にmainへ統合済みである。

局所syntaxのSource/Origin境界、literal/prefixと独立LanguagePackage、annotation adapter、
Doc/Math consumerおよび公式sourceの移行は未完了である。旧コメント処理だけは先に削除せず、
移行後にschema・wire・生成器まで撤去する。T07や受入状態をこのモデル検査でpassedにしない。

## 同期host接続と文書移行までの残り

`nepl3_tools::doc::host::NativeHost` は、明示Profileの実装identityと操作登録を
照合してDoc sentence・Name・Number・Trivia providerを実行する。
呼出しに宣言されたsource閉包だけを使用し、同じBudget/SourceAdmissionを保持する。
予約IDは操作内で単調に発行し、停止した予約ではIDを消費しない。
`ParseSession::read_with_host`へ接続することで、各同期callで成長中のparse arenaを
外向けcontinuationへ複写する処理を避ける。providerのreply検査は省略しない。
catalogのVec全体とreply Boxは、実際の確保・provider実行より先に予算計上する。

#158に基づく境界是正では、source hostの環境・reader stateを固定4言語の列挙から
解決済みProfileの登録へ変更した。default categoryとreader modeも解決済みentryを使う。
portable Await経路も同じ登録実装の照合を通し、schemaのpackage文字列によるdispatchを除いた。
追加alias・登録順変更・非root categoryでnative/portableの構文木一致を確認し、
同じ操作schemaでも別implementation digestなら両経路で拒否する。
開発fixtureの言語構成は依然として明示選択であり、一般suite完成やSentence/A抽出、
comment-as-trivia撤去の完了を意味しない。

この接続だけをHTML backendの未保存変更から分離して検証し、nativeのDoc試験32件、
WASI31件が成功した。差はhost processを使用するnative専用seed検査である。
独立レビューでも通常経路・同期経路・fallbackの構文木一致、登録/sourceの不正入力、
予約停止とconstructorの全Allocation上限を確認した。

`linear-combination.nepld`の約13KB全文は、同期接続後もWork上限100,000,000で
停止する。これはHTML出力や文書移行の完成証拠ではない。reader/tokenizerの
継続状態コピーを削減し、同じ入力・予算・位置・診断を用いた回帰検査を進める。
その後、文書間リンクとasset解決、HTML artifact、意味同等性・安定URLの検証を
接続して、準備できたページからnepld正本とPages配布へ進める。

## 現在の実行経路

`nepl3-doc-core` は `no_std` + `alloc`、production 依存は `nepl3-core` だけ。DocValue は型付き arena であり、構造検査・正規化・source/Origin/View 閉包検査と明示 NDF adapter を提供する。深い入力は平坦な参照と反復処理を使い、共有 DAG の最大経路と guest 内部検査の Depth を合成する。

sentence recognizer は元 SourceSnapshot を借用し、Matched / NoMatch / NeedMore / 位置付き Failed を返す。tools の明示 provider がこれを ReadRequest → ReadReply の正式包絡へ接続する。失敗は型付き Diagnostic と Report、停止は元 StopReason を保持する。literal の token payload は `nepl3.doc.DocumentSyntax`、内部 View は外側 prefix parser の子 arity にならない。

正式な Doc/Math/Circuit/Grammar の文法 source から保存した seed を実 compiler へ渡し、4 alias と各 checked environment を明示した Profile で ParseSession を実行する。runtime provider 登録は実 Name/Trivia/Number と Doc sentence のみ。Facts 拡張は compile 時の署名登録であり、未実装の Doc Facts callback を runtime 実装として広告しない。

`lower::prefix` は host が選択を検査した SyntaxBundle と明示 surface SchemaRef/category を受け、現在の Budget/SourceAdmission で再検査して Doc arena へ変換する。構造 proof は PreparedArticle や guest の意味 proof ではない。prefix constructor と sentence recognizer は同じ正規化を使う。実 ParseSession の literal payload と prefix lower の正常例は同じ意味正規形となる。

`lower::document` は同じ変換へ実FoundationValueCodecを渡し、prefixと受理済みSentenceLiteralを一括lowerする。payload自身のsource閉包、外tokenのhead/View完全一致、意味Span/Originのtoken内包含（明示SourceMapを含む）を検査する。元hostのOrigin列と各token-local Viewを維持し、payload Originのみ末尾へ再配置する。各snapshotのSourceBytesを操作内で一度だけ計上し、停止時は入力構文木を変更しない。従来のcodec不要な`lower::prefix`はliteral leafに対して明示Unsupportedを維持する。

補助 constructor は独立 fragment として lower/codec でき、親 operand では型付き enum/Option へ取り込む。取り込んだ wrapper を意味的な子に残さず、元 View/Origin/source 宣言を保存する。Code の DocGuest は `foreign Doc Article` で、同 alias でも独立 bundle/environment を保持する。構文は正しいが注釈が意味的に不正な guest も Code の表示準備のために意味 lower しない。host への復帰を実 parse で検査する。

## 検証の境界

`labels::check` はArticle内のSection/Anchorを先に収集し、前方Referenceを同一DocLabelIdへ解決する。重複名・未解決名と共有DAG上の複数表示経路を型付き失敗にし、名前selectionと定義全体rangeを分離して保持する。source-less位置はNoneのまま、foreign guestのlabelは集計しない。`LabelError::diagnostic` は意味失敗を通常のschema-validated Diagnosticへ変換し、名前と複数経路を構造化引数へ、先行定義をrelated位置へ保持する。構築自身の停止時も元の借用エラーは残る。CheckedLabelsはlabel検査だけのproofで、foreign Requirement解決やPreparedArticleではない。

Doc arena の node/root、Origin、局所 View と source map は portable 往復で保持する。共通 codec は source 表を identity 順、guest syntax graph を正準 NodeRef 座標へ整えるため、未正準な native 表の列順や guest NodeRef 数値そのものの Rust Eq は wire の要件ではない。source の identity/URI/content、owner Origin ID と環境 digest、および実 CBOR の正準再 encode を照合する。

管理対象は Doc core の構造・literal・正規化・source/ForeignClosure・初回 CBOR と、tools の実 compiler/parse/prefix lower・helper 範囲・DocGuest・resource 停止。host seed と元 source/adapter の一致だけは Python process を使う native 専用試験で、同じ保存 seed を使う実処理は WASI でも実行する。browser target は compile 検査であり実描画の成功ではない。

`text::plain_text` は正式PlainTextRequestを受け、document/guestの正準digestへ束縛した明示host textを用いてSentenceを投影する。BaseOnly / WithReadings / WithAllNotesの固定規則と、表示しないnote/readingには未提供embed textを要求しない規則を実行する。全提供entryはpolicyにかかわらず検査する。型付きrequest/replyの初回CBOR、実prefix→lower→投影、元source変更・guest差替え・owner環境変更、準備中と出力中の停止を管理対象で検査する。公開prepareはidentity取得だけで、公開実行が再検査と資源計上を省略するproofではない。これはguest意味checkやPreparedArticleの完成を表さない。

`print::print` はPrintRequest/Replyの正式操作としてPrefix/Compactを出力する。64 form・20 entryは元の表層入力を実parse/lowerし、印字後に同じ実経路へ戻す。MathGuest / CircuitGuest / Guestの独立rootと4 wrapperを保持する。明示host guest-printはdocumentとForeignClosureのdigestへ束縛し、原文取得だけを意味一致proofとして扱わない。名前と言語は共通reader語彙で印字可能性を検査し、source-less不適合は元値を変えず型付き失敗にする。Doc coreはguest意味処理もsnapshot発行も行わない。

## 残り

`prepare::inspect` はArticleの全variant・labelを検査し、Link/Assetの意味node別要求とForeignClosure別要求を列挙する。正式DocPreparationPlanはdocument/guestのcanonical digestへ束縛され、独立受信時には明示documentから全要求を再導出する。実sourceのCode/InlineMathは構文のまま保持し、外部資源を読まず評価しない。これは準備入力の発見であり、下記の資源解決・HTML出力の完成ではない。

- 公開suiteでのhost guest-printの供給と、対象guest parserによる構造正規形・source対応の検査。現在の管理対象は実generic engine parser/checked tree/printerを使う構文往復であり、raw PrintedGuestのtextだけで一致proofを得ない。この残りは全guestの意味lower/checkを要求するものではなく、意味不正・回復構文を表示用Codeとして保持する契約を維持する。Math/Circuit等の意味check・評価の完成でもない。
- 外部 page・asset 解決、foreign Requirementを含む完全なcheck/prepare、HTML backend と rendering。schema に表・list・link・code・asset があることだけで、これらの実装済みを主張しない。
- Doc の正式 lower 操作の Report/部分結果包絡と全 suite adapter。native helper の Result を、別実装の操作包絡の完成として扱わない。

設計入力は main `b5295cef655aa59affffd6644f2902268d071953` の文書監査。inventory SHA-256 は `daf94085913930f05c1655d2adbef4f56651864f9a97ac4d261400449c98499e`、61 Markdown / 231 Rust source owner、52 表 / 1283 cell、39 list / 233 item、1134 inline code、12 code block、249 link、1 image。追加 element category はない。この監査は実装中差分の completeness、rustdoc 意味監査、T21 の移行完了とは別である。

## Tokenizer同期接続の次段階

prefix engineからtokenizerの同期hostを使用し、成功callで外側tokenizer継続を発行しない
経路を追加した。独立レビューで見つかった明示Stoppedエラーと入れ子の停止原因もBudgetへ
保持する。既存所有経路との構文木・診断・source/map比較を維持する。
HTMLの全文再現試験では同じWork上限100,000,000のまま停止位置が1412から1897へ進んだが、
13KB文書の完走には至っていない。これは部分改善であり、文書移行やHTML全体の成功ではない。
次は共有sourceの実コピー費用と外部echo比較費用の分離を検討し、外部入力の検査を維持して
本文の反復コピー課金を解消する。

続く実装では、実コピーと比較前の走査費用を分離し、source storeの重複検索を
予算付きの二分探索へ変更した。公開snapshot列の順序、外部echoの全文比較上限、
source編集の原子的な反映は維持する。同期reader callbackは外部継続を受け取らず、
排他的なprivate slotを既存の返却値検査経路へ渡す。
同じ全文試験は停止位置3399まで進んだが、なおWork上限で停止する。
途中の線形検索版ではGrammar bootstrapも既存上限で停止したため、その版を完成扱いにせず、
二分探索版でbootstrapを再実行して成功を確認した。workspace試験とClippyも成功した。
当時の残件はsource admission・診断source・Origin graphの繰り返し検索だった。

2026-09-13、`272228c`で同じ約13KB全文を`doc-html export`へ再入力したところ、
既存の各操作Work上限100,000,000のままHTMLまで成功した。使用Workはparse/validateが
20,362,601、lowerが2,987,790、prepare/render/serializeが6,986,518だった。
これらは別々の予算を持つ操作であり、一つのend-to-end予算の値ではない。
入力SHA-256は`9099bbbe48ed75a0768997789229188f53e43348fe930522b119908dfdb7a889`。
上記3399での停止は過去の途中版の記録で、現行の再現結果ではない。
この全文HTML試験はnative/WASIで成功している。多数ページ、全foreign、Pages配信の
性能・完成までを一例の成功から推定しない。

## Grammar仕様原稿の検査と割当課金

第04章の未公開NEPL3d原稿を、既存の原稿用parse/lower/labels試験へ追加した。
前段の修正時点では、通常ページのAllocation上限500,000,000で完成木の検査中に停止した。
その時点ではcanonical registryへ登録せず、Markdown正本を維持した。
未公開原稿用の既存の有限予算で処理できることと、正本切替の準備完了は区別する。

調査で、SyntaxBundle検査が共有SourceSnapshotにも本文コピー分を課金し、
借用するOrigin配列にも所有配列分を課金していたことが分かった。
SourceStoreの予算付き参照挿入を使い、Origin検査自身が計上するscratchだけを課金する。
Source入場、重複拒否、全Originの参照・cycle検査、非atomic targetの実コピー費用は維持する。
長短sourceの共有費用、source挿入時の停止、未使用Originの不正参照を回帰試験にした。
この課金修正のみでは第04章は通常上限で停止し、性能課題は未解消だった。

続く計測で、schema検査のpending stackがpop後の容量を再利用していても、
各pushで新規slot分を課金していたことが分かった。論理的な予約slot数を保持し、
倍増時の追加容量を確保前に課金する。allocatorの余剰capacityは課金判定に使わない。
各子のWorkはqueue投入前に課金し、広い入力の無制限な展開を防ぐ。
全値の型検査・Nodes・深さ・左から右の検査順は維持する。

この修正で第04章は通常予算内のparse/lower/labelsを通過した。
parseのWorkは94,666,857、Allocationは341,191,768、lowerのWorkは85,689,406だった。
SourceMapの一意な直接対応を範囲として検査する改善も加えたが、
この原稿の停止解消に寄与したのはschema stackの課金修正である。
一文書の成功を全仕様移行や全受入群の達成とは扱わない。

## Doc仕様原稿の次の性能境界

第05章の原稿で欠けていたguest printerの深さ合成とDoc/Math adapterの停止契約を補い、
独立Sentence readerから現Doc consumerへ変換する段階を正文と原稿に明記した。
修正後のCLIによる通常Work上限100,000,000でのHTML出力はcursor 47,445でWorkLimitとなり、
正本切替は行っていない。

Work計測からSourceMapのsnapshot検索を調べ、直前の検索位置に隣接するidentityを
二分探索の前に照合する改善を加えた。完全なSnapshotIdの比較、cycle検査、重複edge、
各比較前の課金を維持する。順序付きedgeを反復する257頂点のstarでは、修正前の
WorkLimitに対し修正後は435,257 Workで検査できた。第05章は通常予算で停止しており、
この小さい回帰試験を同原稿の性能課題の解消とは扱わない。

続いてreaderのprovider境界で、新規mappingがなく全包含関係を同一snapshot内で
直接確認できる場合に限り、private checkpointで検査済みのmapping集合の再検査を省いた。
viewのschema・参照・cycle、factのmetadataとsource位置の検査は引き続き実行する。
間接的な包含関係または新規mappingがある場合は従来通り集合全体を検査する。
不正KindRef、空Capture名、未登録Presentation/Relation schema、追加mapによるcycle、
停止予算を負例として検査した。readerのnative試験とWASI runtime試験、
追加負例を含むlibrary試験は成功し、独立レビューの指摘を修正した。

同じ第05章の通常HTML出力はcursor 54,300まで進んだが、Work上限100,000,000で
停止した。予算を変更しておらず、第05章の正本切替および性能課題の解消は未達である。

次のWork計測ではtokenizerの受入済みsourceに対する競合検索が約30,804,689を占めた。
単発検索のhintとbatch検索は実文書で改善せず撤回した。
tokenizer session内で、照合した宣言集合と受入済みprefixのimmutable snapshotを保持し、
次回も完全一致する部分の競合検索だけを再利用する。宣言集合の変更・prefix変更は
再検査し、source admission・report検査・継続scope検査は従来通り実行する。
保持するsnapshotは所有し、allocatorのaddress再利用を同一性の根拠にしない。
外部decode値は内容比較を省かず、closeで保持情報を解放する。
検査の順序・Usageは変わり得るが、停止を成功へ変換したり新しい予算へ移したりしない。

この変更の通常HTML出力はcursor 70,193でWorkLimitとなった。上限は同じ100,000,000で、
第05章の完走と正本化はまだ未達である。環境変更、変更されたsuffix、同一内容の
別storage、部分的なcache確保後の停止と再検査を回帰試験で確認する。

さらに、新しいsource/mapと診断/eventがなく、全artifactの位置を消費範囲内で
直接確認できるprovider応答では、そのsnapshotだけで通常の応答検査を行う。
範囲外の合法なPresentation/Relationは従来のresolverへ戻し、不正なschema・参照・
report、新規sourceの競合を省略しない。private checkpointの無関係なsource集合を
tokenごとに再構築していた費用を減らした。

第05章は通常予算でparse 88,653,715 Work、lower 66,285,115 Work、
labels 8,413,256 Workに収まり、この予算を原稿の回帰試験にも適用した。
単独HTML出力はWorkLimitではなく17章・23章への2リンクのNeedsResolutionまで進む。
これはHTML生成・リンク解決・文書の意味比較・正本切替の完了を意味しない。

第05章の正本移行では、原稿を `doc/spec/05-document.nepld` へ移し、旧見出し16件を
aliasとして登録した。第23章は未移行Markdownの原文参照であり、Doc正本へ昇格させない。
本文の独立比較でinline code92個、RawCode2個、15節、リンク2件の保存を確認した。
18ページのHTML集合は通常のpage予算で生成でき、第05章のparseは87,582,341 Work、
lowerは65,610,740 Workとなった。JavaScript無効のChromium/Firefox/WebKitで本文と
17章HTML・23章原文へのリンクを確認した。混在siteでは23章を既生成HTMLへ接続する。

Markdown集合の旧Work上限1,600,000,000では停止し、途中出力を採用しなかった。
18ページ用に集合Workだけを1,900,000,000へ変更した別実行は1,615,489,130 Work、
63,205,674 Nodesで完了した。Nodes上限64,000,000と各pageの通常予算は維持する。
既存17ページの本文は不変で、集合入力digestに伴うmetadataだけが更新される。
停止診断にはpage・段階・Usageを加え、文書集合の停止と単体解析の停止を区別する。
この移行はT07/T21全体や未移行の残り6章の完成を意味しない。
