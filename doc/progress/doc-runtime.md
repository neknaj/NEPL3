# Doc runtime の段階実装

T07 は進行中。`doc/spec/05-document.md` と `design/forms.json` を最終契約とし、以下の native API ができたことを T07 全体の完了へ読み替えない。

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
次は残るsource admission・診断source・Origin graphの繰り返し検索を調べる。
