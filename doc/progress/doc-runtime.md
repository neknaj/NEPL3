# Doc runtime の段階実装

T07 は進行中。`doc/spec/05-document.md` と `design/forms.json` を最終契約とし、以下の native API ができたことを T07 全体の完了へ読み替えない。

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

## 残り

- 全 constructor の print → parse → lower、compact printer、guest wrapper 単独の fragment API とその往復。
- 外部 page・asset 解決、foreign Requirementを含む完全なcheck/prepare、plain_text、HTML backend と rendering。schema に表・list・link・code・asset があることだけで、これらの実装済みを主張しない。
- Doc の正式 lower 操作の Report/部分結果包絡と全 suite adapter。native helper の Result を、別実装の操作包絡の完成として扱わない。

設計入力は main `b5295cef655aa59affffd6644f2902268d071953` の文書監査。inventory SHA-256 は `daf94085913930f05c1655d2adbef4f56651864f9a97ac4d261400449c98499e`、61 Markdown / 231 Rust source owner、52 表 / 1283 cell、39 list / 233 item、1134 inline code、12 code block、249 link、1 image。追加 element category はない。この監査は実装中差分の completeness、rustdoc 意味監査、T21 の移行完了とは別である。
