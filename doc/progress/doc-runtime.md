# Doc runtime の段階実装

T07 は進行中。`doc/spec/05-document.md` と `design/forms.json` を最終契約とし、以下の native API ができたことを T07 全体の完了へ読み替えない。

## 同期host接続と文書移行までの残り

`nepl3_tools::doc::host::NativeHost` は、明示Profileの実装identityと操作登録を
照合してDoc sentence・Name・Number・Trivia providerを実行する。
呼出しに宣言されたsource閉包だけを使用し、同じBudget/SourceAdmissionを保持する。
予約IDは操作内で単調に発行し、停止した予約ではIDを消費しない。
`ParseSession::read_with_host`へ接続することで、各同期callで成長中のparse arenaを
外向けcontinuationへ複写する処理を避ける。providerのreply検査は省略しない。
catalogのVec全体とreply Boxは、実際の確保・provider実行より先に予算計上する。

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

## Tokenizer????????

prefix engine??tokenizer???host???????call???tokenizer????????
??????????????????????Stopped?????????????Budget?
????????????????????source/map????????
HTML???????????Work??100,000,000????????1412??1897??????
13KB??????????????????????????????HTML??????????
????source??????????echo????????????????????????
????????????????
