<!-- Generated from doc/spec/03&#45;reader.nepld; renderer nepl3-tools.markdown-annotated-pages/1; page reader; source SHA-256 e64b079cc4b4a5d28aa420d378557340bdf986ad3d4374672257b8e833071c85; alias input SHA-256 7b0de26b733079af4d75d3bf3190b6fcfd22d378a02de4b108f9c6d59e72ac4b; document digest 06f60d545f098eefa93447943b7d66687f70b030b56b68cc43d230eba369340e; input PageSet digest 943ad743b12614300239fe63cb49f04dc425ce932763acae94dab8be258e8a89; input context SHA-256 3aee2f9d1af4170ff68e2561f995e52d2e115755069880d4eb57776b9385cbef. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="03-reader--tokenizer"></a>

# 03\. Reader \/ tokenizer

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## 方針\[ほうしん\]

Readerは、通常\[つうじょう\]の字句解析\[じくかいせき\]を超\[こ\]える再帰\[さいき\]・状態\[じょうたい\]・外部実装\[がいぶじっそう\]を許\[ゆる\]す。共通\[きょうつう\]prefix parserに対\[たい\]しては、一\[ひと\]つのtokenを返\[かえ\]す境界\[きょうかい\]を保\[たも\]つ。正規表現\[せいきひょうげん\]だけを、表現能力\[ひょうげんのうりょく\]の上限\[じょうげん\]にしない。

<a name="n-72657175657374"></a>

<a name="1-呼出し契約"></a>

## 1\. 呼出\[よびだ\]し契約\[けいやく\]

`ReadRequest = {snapshot, start, limit, finalInput, context, state, limits}`。`ReadReply = Matched(value, end, newState, view, facts, diagnostics) | NoMatch(expected, furthest) | NeedMore(expected) | Failed(diagnostic, recovery) | Stopped(reason) | Await(externalRequest, continuation)`。limitは、このreaderが参照\[さんしょう\]できる入力末尾\[にゅうりょくまつび\]を表\[あらわ\]す。start\/end\/limitは、同一\[どういつ\]snapshotのUTF\-8境界\[きょうかい\]に置\[お\]く。通常\[つうじょう\]tokenの成功\[せいこう\]には `start < end <= limit` を要求\[ようきゅう\]する。combinator内部\[ないぶ\]のlook\/optionalなどでは空\[から\]の成功\[せいこう\]を許\[ゆる\]すが、token化\[か\]・skip・反復\[はんぷく\]の反復単位\[はんぷくたんい\]では進捗\[しんちょく\]を要求\[ようきゅう\]する。

NoMatchとNeedMoreは、state\/facts\/確定診断\[かくていしんだん\]を変更\[へんこう\]しない。Failedは、形式\[けいしき\]を確定\[かくてい\]してから見\[み\]つかった不正入力\[ふせいにゅうりょく\]を表\[あらわ\]す。finalInput\=trueで入力\[にゅうりょく\]が終\[お\]わる場合\[ばあい\]は、NeedMoreではなくNoMatchまたはFailedを返\[かえ\]す。StoppedをNoMatchへ変\[か\]えて、別候補\[べつこうほ\]へ進\[すす\]んではならない。

`Await` は、プログラム製\[せい\]reader\/transformの呼出\[よびだ\]し要求\[ようきゅう\]である。engine\/suiteがmanifestで許可\[きょか\]したproviderだけを呼\[よ\]び、同\[おな\]じ予算\[よさん\]で再開\[さいかい\]する。continuationはproviderとsnapshotに束縛\[そくばく\]する。対象\[たいしょう\]ソースに書\[か\]かれた任意\[にんい\]コードを、自動実行\[じどうじっこう\]しない。

<a name="n-636f6d62696e61746f7273"></a>

<a name="2-combinatorの意味"></a>

## 2\. combinatorの意味\[いみ\]

ReaderExprの全\[ぜん\]signatureは、grammar\-signaturesを参照\[さんしょう\]する。

- literalは指定\[してい\]のUTF\-8文字列\[もじれつ\]に完全一致\[かんぜんいっち\]し、Unitを返\[かえ\]す。空\[くう\]literalも認\[みと\]めるが、進捗検査\[しんちょくけんさ\]の対象\[たいしょう\]とする。
- scalarは、一\[ひと\]つのUnicode scalarが指定\[してい\]classに属\[ぞく\]すれば、Textとして返\[かえ\]す。
- seqは順\[じゅん\]に実行\[じっこう\]し、結果\[けっか\]のListを返\[かえ\]す。失敗時\[しっぱいじ\]には、状態\[じょうたい\]とfactsをtransaction開始位置\[かいしいち\]へ戻\[もど\]す。
- choiceは順\[じゅん\]に試\[ため\]し、最初\[さいしょ\]の成功\[せいこう\]を返\[かえ\]す。NoMatchのときだけ次\[つぎ\]の候補\[こうほ\]を試\[ため\]し、NeedMore\/Failed\/Stoppedはその場\[ば\]で伝播\[でんぱ\]する。全枝\[ぜんえだ\]の成功値\[せいこうち\]は、同\[おな\]じ宣言型\[せんげんがた\]を持\[も\]つ。境界認識\[きょうかいにんしき\]だけが必要\[ひつよう\]で枝\[えだ\]ごとの値\[あたい\]が異\[こと\]なる場合\[ばあい\]は、各枝\[かくえだ\]へ明示的\[めいじてき\]なdiscardを適用\[てきよう\]してUnitに揃\[そろ\]える。seqのList値\[ち\]を、暗黙\[あんもく\]にTextへ連結\[れんけつ\]しない。
- many\/someは、それぞれ0回以上\[かいいじょう\]と1回以上\[かいいじょう\]の反復\[はんぷく\]であり、NoMatchで終了\[しゅうりょう\]する。成功\[せいこう\]しても入力\[にゅうりょく\]を消費\[しょうひ\]しなければNonProgressとする。NeedMoreを終了扱\[しゅうりょうあつか\]いにしない。
- repeat min max pは、有限回\[ゆうげんかい\]のmanyである。0 \<\= min \<\= maxを要求\[ようきゅう\]し、maxへ到達\[とうたつ\]した後\[あと\]はそれ以上読\[いじょうよ\]まない。
- optionalはNoMatchをNoneへ変\[か\]え、成功\[せいこう\]はSomeとする。その他\[ほか\]の失敗\[しっぱい\]は伝播\[でんぱ\]する。
- look\/notは、元\[もと\]のcursor\/state\/factsを変更\[へんこう\]しない。lookは、成功\[せいこう\]または不一致\[ふいっち\]という結果\[けっか\]だけを保持\[ほじ\]する。notはMatchedとNoMatchを交換\[こうかん\]してUnitを返\[かえ\]し、NeedMore\/Failed\/Stoppedは伝播\[でんぱ\]する。
- commit pはpのNoMatchをFailedへ変換\[へんかん\]し、prefixを認識\[にんしき\]した後\[あと\]の残\[のこ\]りを包\[つつ\]む。例\[たと\]えば `seq [literal quote, commit bodyAndClose]` とする。
- captureは子\[こ\]の成功範囲\[せいこうはんい\]にfield名\[めい\]を付\[つ\]け、regionは表示\[ひょうじ\]roleを付\[つ\]ける。両者\[りょうしゃ\]とも意味値\[いみち\]を変更\[へんこう\]しない。
- nodeは、子\[こ\]のmatch treeを指定\[してい\]kindのViewElementで包\[つつ\]む。kindは、現在\[げんざい\]packageのschemaへ登録\[とうろく\]する。
- discardは成功値\[せいこうち\]をUnitへ変\[か\]えるが、sourceと確定診断\[かくていしんだん\]は失\[うしな\]わない。
- refは、同\[おな\]じpackageのreaderを呼\[よ\]ぶ。進捗\[しんちょく\]のない再帰\[さいき\]は静的検査\[せいてきけんさ\]ができる範囲\[はんい\]で拒否\[きょひ\]し、実行時\[じっこうじ\]にもcallsite\+cursor\+contextの再入\[さいにゅう\]を検出\[けんしゅつ\]する。
- decodeは成功後\[せいこうご\]にnamed pure decoderを適用\[てきよう\]し、元\[もと\]からdecode後\[ご\]へのSourceMapも返\[かえ\]す。
- mapは、named Transform providerを成功値\[せいこうち\]とsource\/viewへ適用\[てきよう\]する。結果\[けっか\]が宣言済\[せんげんず\]み型\[かた\]と一致\[いっち\]しなければ、ProviderContractViolationとする。
- thenは、最初\[さいしょ\]のparserの値\[あたい\]とendを依存\[いぞん\]parser providerへ渡\[わた\]し、そのproviderが後続\[こうぞく\]を読\[よ\]み取\[と\]る。返\[かえ\]す値\[あたい\]は後続\[こうぞく\]parserの値\[あたい\]とし、全体範囲\[ぜんたいはんい\]は開始\[かいし\]から後続\[こうぞく\]endまでとする。前半\[ぜんはん\]のsource\/factsは保持\[ほじ\]する。
- callは、named Reader providerへ要求全体\[ようきゅうぜんたい\]を渡\[わた\]す。reader全体\[ぜんたい\]をRustなどで実装\[じっそう\]できる。
- eofはlimitで成功\[せいこう\]するが、finalInput\=falseならNeedMoreとする。
- takecount nはn個\[こ\]のUnicode scalarを読\[よ\]み、byte数\[すう\]と混同\[こんどう\]しない。
- until dは、最初\[さいしょ\]の非空\[ひくう\]delimiter dの直前\[ちょくぜん\]までをTextとして読\[よ\]み、delimiterは消費\[しょうひ\]しない。delimiterが無\[な\]ければ、未確定入力\[みかくていにゅうりょく\]ではNeedMore、確定入力\[かくていにゅうりょく\]ではNoMatchとする。

choice途中\[とちゅう\]の失敗診断\[しっぱいしんだん\]は、最終的\[さいしゅうてき\]に全候補\[ぜんこうほ\]がNoMatchになった場合\[ばあい\]だけ、最遠到達位置\[さいえんとうたついち\]のexpected集合\[しゅうごう\]へ統合\[とうごう\]する。成功\[せいこう\]した別候補\[べつこうほ\]へ、誤\[あやま\]ったエラーを残\[のこ\]さない。work予算\[よさん\]は巻\[ま\]き戻\[もど\]さない。

native readerの内部\[ないぶ\]checkpointは、追記済\[ついきず\]みcollectorの不変\[ふへん\]なprefixに対\[たい\]する長\[なが\]さ、cursor、state、trace overflowを保存\[ほぞん\]してよい。viewの要素\[ようそ\]とroot、facts、診断\[しんだん\]、event、source、SourceMapは、それぞれ開始時\[かいしじ\]のprefixを保\[たも\]つ。失敗\[しっぱい\]やlookの復帰\[ふっき\]では追加部分\[ついかぶぶん\]を除去\[じょきょ\]し、保存\[ほぞん\]したstateへ戻\[もど\]す。Nodeによるrootの束\[たば\]ね直\[なお\]しやtransformによる置換\[ちかん\]は、そのframeが開始\[かいし\]してから追加\[ついか\]した範囲\[はんい\]に限定\[げんてい\]する。外部\[がいぶ\]へAwaitを公開\[こうかい\]するときは、各\[かく\]frameの完全\[かんぜん\]な所有\[しょゆう\]checkpointを予算内\[よさんない\]で具体化\[ぐたいか\]し、既存\[きそん\]のReaderContinuation schemaを使用\[しよう\]する。privateな長\[なが\]さやpointerだけを交換形式\[こうかんけいしき\]へ渡\[わた\]さず、再開時\[さいかいじ\]には従来\[じゅうらい\]どおり保存済\[ほぞんず\]みcontinuationと応答\[おうとう\]を検査\[けんさ\]する。これは実際\[じっさい\]のcollectorコピーを省\[はぶ\]く内部表現\[ないぶひょうげん\]の変更\[へんこう\]であり、実施\[じっし\]したコピーの課金\[かきん\]を取\[と\]り除\[のぞ\]く理由\[りゆう\]にはしない。

<a name="n-6c65786963616c"></a>

<a name="3-文字集合と基礎reader"></a>

## 3\. 文字集合\[もじしゅうごう\]と基礎\[きそ\]reader

配布\[はいふ\]profileではUnicode 16\.0\.0のXID\_Start \/ XID\_Continueを使\[つか\]い、underscoreは開始\[かいし\]にも許\[ゆる\]す。識別子\[しきべつし\]を勝手\[かって\]にNFC変換\[へんかん\]せず、scalar列\[れつ\]の完全一致\[かんぜんいっち\]で比較\[ひかく\]する。将来\[しょうらい\]のUnicode版変更\[ばんへんこう\]は、package revisionに含\[ふく\]める。

Nameの文字判定\[もじはんてい\]と全綴\[ぜんつづ\]り検査\[けんさ\]、Langの全綴\[ぜんつづ\]りABNF検査\[けんさ\]は、共通\[きょうつう\]coreの `lexical` 契約\[けいやく\]としてreaderとdomain printerが共有\[きょうゆう\]する。現\[げん\]native実装\[じっそう\]のXID表\[ひょう\]は、pinned `unicode-ident = 1.0.18` の [Unicode 16\.0\.0生成表\[せいせいひょう\]](<https\:\/\/github\.com\/dtolnay\/unicode\-ident\/blob\/1\.0\.18\/src\/tables\.rs>) に従\[したが\]う。全綴\[ぜんつづ\]り検査\[けんさ\]は予算付\[よさんつ\]きで、区切\[くぎ\]り・triviaを含\[ふく\]まない一\[ひと\]つの語\[ご\]だけを判定\[はんてい\]する。選択言語\[せんたくげんご\]の予約\[よやく\]head判定\[はんてい\]や正規化\[せいきか\]を代行\[だいこう\]しない。readerの最大一致\[さいだいいっち\]、非\[ひ\]final入力\[にゅうりょく\]のNeedMore、境界診断\[きょうかいしんだん\]はreader側\[がわ\]に残\[のこ\]し、この共通化\[きょうつうか\]で認識集合\[にんしきしゅうごう\]を変更\[へんこう\]しない。printerは不適合\[ふてきごう\]な意味値\[いみち\]を型付\[かたつ\]き失敗\[しっぱい\]として扱\[あつか\]い、reader依存\[いぞん\]や別\[べつ\]のUnicode\/BCP47規則\[きそく\]の複製\[ふくせい\]を導入\[どうにゅう\]しない。

基礎\[きそ\] `Name` は、上記\[じょうき\]識別子\[しきべつし\] 一\[ひと\]つである。`Nat` は `0` または `[1-9][0-9]*` とする。`Number` はoptional `-`、Nat、optional `.` と1桁以上\[けたいじょう\]の数字\[すうじ\]から成\[な\]る。このsurfaceには、指数表記\[しすうひょうき\]を設\[もう\]けない。`Text` は、通常\[つうじょう\]の二重引用符文字列\[にじゅういんようふもじれつ\]である。`Lang` は、ASCIIのwell\-formed BCP47 tagである。BCP47 tagの比較\[ひかく\]はASCII case\-insensitiveとし、元\[もと\]の綴\[つづ\]りは保存\[ほぞん\]する。登録状況\[とうろくじょうきょう\]をnetworkで照会\[しょうかい\]しない。

通常\[つうじょう\]Textのescapeは `\\`、`\"`、`\n`、`\r`、`\t`、`\u{1〜6 hex}` とする。surrogateとU\+10FFFF超\[ちょう\]は拒否\[きょひ\]し、未知\[みち\]escapeはエラーとする。Textではruby\/annoを認識\[にんしき\]しない。Textも直接\[ちょくせつ\]CR\/LFを拒否\[きょひ\]し、開始引用符後\[かいしいんようふご\]はcommitする。閉\[と\]じ引用符\[いんようふ\]・escape・Unicode escapeの途中\[とちゅう\]で入力\[にゅうりょく\]が途切\[とぎ\]れた場合\[ばあい\]は、finalInput\=falseならNeedMore、trueならFailedとする。既\[すで\]に不正\[ふせい\]と確定\[かくてい\]したescapeやscalarは、追加入力\[ついかにゅうりょく\]を待\[ま\]たずFailedとする。Numberの小数点後\[しょうすうてんご\]には最低\[さいてい\]1桁\[けた\]を要求\[ようきゅう\]する。入力末尾\[にゅうりょくまつび\]で桁\[けた\]が欠\[か\]ける場合\[ばあい\]も、非\[ひ\]finalではNeedMore、finalではFailedとなる。

基礎\[きそ\]readerの値\[あたい\]は、Name\/Text\/LangがText、Natが非負\[ひふ\]の任意精度\[にんいせいど\]Integer、Numberが正確\[せいかく\]なRational、TriviaがUnitである。Natからarity\/revisionなどのU64へ変換\[へんかん\]するlowerは、範囲\[はんい\]を明示検査\[めいじけんさ\]する。Langのwell\-formedは、[RFC 5646 §2\.1\/§2\.2\.9](<https\:\/\/www\.rfc\-editor\.org\/rfc\/rfc5646\.html\#section\-2\.2\.9>)のABNFを意味\[いみ\]し、26個\[こ\]のgrandfathered tagとprivate\-useを含\[ふく\]む。登録状態\[とうろくじょうたい\]やvariant\/singleton重複\[じゅうふく\]を含\[ふく\]むvalid判定\[はんてい\]を、この操作\[そうさ\]の追加要件\[ついかようけん\]にしない。

BuiltinRequestはkind、ReadRequest、`Option<SourceReservation>`を持\[も\]つ。SourceReservationは、hostが一\[ひと\]つの生成\[せいせい\]snapshotへ予約\[よやく\]するsourceId\/revision\/絶対\[ぜったい\]URIであり、digestは生成\[せいせい\]bytesから算出\[さんしゅつ\]する。Textでのみ予約\[よやく\]を必須\[ひっす\]とし、他\[ほか\]のbuiltinへ渡\[わた\]した予約\[よやく\]は契約違反\[けいやくいはん\]とする。入力\[にゅうりょく\]と同\[おな\]じsourceId\/revisionの予約\[よやく\]は拒否\[きょひ\]する。成功\[せいこう\]するまで生成\[せいせい\]sourceをadmitせず、NoMatch\/NeedMore\/Failedでは生成物\[せいせいぶつ\]を確定\[かくてい\]しない。成功後\[せいこうご\]のdecoded sourceと、元\[もと\]からdecodedへのSourceMapをReadReplyへ保持\[ほじ\]する。coreが乱数\[らんすう\]・時計\[とけい\]・独自\[どくじ\]namespaceからIDを発明\[はつめい\]するのではなく、tokenizer\/hostが各呼出\[かくよびだ\]しの予約\[よやく\]を供給\[きょうきゅう\]する。再試行\[さいしこう\]は同\[おな\]じ予約\[よやく\]・同\[おな\]じ結果\[けっか\]bytesなら同一\[どういつ\]snapshotとして扱\[あつか\]い、異\[こと\]なるbytesへの使\[つか\]い回\[まわ\]しはIdentityConflictとする。

standard triviaはASCII space\/tab\/CR\/LFと、`#`から行末直前\[ぎょうまつちょくぜん\]までのcommentである。literal内\[ない\]ではtrivia処理\[しょり\]を行\[おこな\]わない。readerが処理\[しょり\]するのは前側\[まえがわ\]のtriviaだけであり、子\[こ\]の最終\[さいしゅう\]tokenの後\[あと\]で子言語\[こげんご\]のtriviaを勝手\[かって\]に消費\[しょうひ\]しない。これらは配布\[はいふ\]4言語\[げんご\]のprofileであり、全\[ぜん\]NEPL3言語\[げんご\]へ同\[おな\]じ字句規則\[じくきそく\]を強制\[きょうせい\]しない。新\[あたら\]しいpackageは、別\[べつ\]modeとreaderを定義\[ていぎ\]できる。

<a name="n-6d6f646573"></a>

<a name="4-modeとtoken決定"></a>

## 4\. modeとtoken決定\[けってい\]

modeのskip規則\[きそく\]を宣言順\[せんげんじゅん\]に試\[ため\]し、成功\[せいこう\]して進捗\[しんちょく\]したら最初\[さいしょ\]から反復\[はんぷく\]する。どのskipも一致\[いっち\]しなくなった位置\[いち\]で、take規則\[きそく\]を宣言順\[せんげんじゅん\]に試\[ため\]す。NoMatch以外\[いがい\]は決定\[けってい\]を妨\[さまた\]げるため、上記\[じょうき\]に従\[したが\]って伝播\[でんぱ\]する。Word reader自身\[じしん\]が、一\[ひと\]つの識別子\[しきべつし\]を最長\[さいちょう\]で読\[よ\]む。`letx`を `let` と `x` に分割\[ぶんかつ\]しない。自然数\[しぜんすう\]などの基本\[きほん\]readerは、後続\[こうぞく\]がidentifierContinueならBoundaryMismatchとし、数値\[すうち\]の途中\[とちゅう\]でtokenを確定\[かくてい\]しない。headのspellingは元\[もと\]のlexemeで比較\[ひかく\]し、payloadの表示文字列\[ひょうじもじれつ\]から逆算\[ぎゃくさん\]しない。

構文\[こうぶん\]カテゴリごとのform照合\[しょうごう\]は、tokenを読\[よ\]んだ後\[あと\]で行\[おこな\]う。同一\[どういつ\]category・同一\[どういつ\]spellingでshapeが異\[こと\]なる場合\[ばあい\]は、package compile時\[じ\]に拒否\[きょひ\]する。modeのordered choiceによる優先順位\[ゆうせんじゅんい\]は、明示的\[めいじてき\]な仕様\[しよう\]である。優先順位\[ゆうせんじゅんい\]が必要\[ひつよう\]なケースで、暗黙\[あんもく\]の最長一致\[さいちょういっち\]へ切\[き\]り替\[か\]えない。

<a name="n-73656e74656e636573"></a>

<a name="5-sentence-reader"></a>

## 5\. Sentence reader

Doc章\[しょう\]の完全\[かんぜん\]な再帰規則\[さいききそく\]を参照\[さんしょう\]する。最初\[さいしょ\]の二重引用符\[にじゅういんようふ\]を認識\[にんしき\]したらcommitし、終了引用符\[しゅうりょういんようふ\]は未\[み\]escapeのものだけとする。LF\/CRをliteral内\[ない\]へ直接置\[ちょくせつお\]くことは許可\[きょか\]しない。明示的\[めいじてき\]な `\n` は内容\[ないよう\]として許可\[きょか\]する。これにより、閉\[と\]じられていないliteralが後続\[こうぞく\]の文書全体\[ぶんしょぜんたい\]を飲\[の\]み込\[こ\]み続\[つづ\]けることを防\[ふせ\]ぐ。

sentence decoderは、quoted範囲\[はんい\]の生\[なま\]のescapeを保持\[ほじ\]して注釈\[ちゅうしゃく\]を認識\[にんしき\]する。先\[さき\]に全\[ぜん\]escapeを展開\[てんかい\]してから `[` の意味\[いみ\]を決\[き\]めない。例\[たと\]えば `\u{5B}` は内容\[ないよう\]の `[` であり、ruby開始\[かいし\]ではない。prefix側\[がわ\]のarityは常\[つね\]に0とする。内部\[ないぶ\]viewには、本文\[ほんぶん\]・delimiter・base・reading・noteの位置\[いち\]を保持\[ほじ\]する。

<a name="n-636f6d706c65785f746f6b656e73"></a>

<a name="6-html相当の複雑なtoken"></a>

## 6\. HTML相当\[そうとう\]の複雑\[ふくざつ\]なtoken

conformance用\[よう\]のAngleTag readerは `<`、ASCIIの名前\[なまえ\]、引用符付\[いんようふつ\]き範囲\[はんい\]を含\[ふく\]む任意\[にんい\]のtag内部\[ないぶ\]、`>` の境界\[きょうかい\]を一\[ひと\]つのtokenとして認識\[にんしき\]する。引用符内\[いんようふない\]の `>` で終了\[しゅうりょう\]してはならない。これは開始\[かいし\]tagの境界\[きょうかい\]readerであり、属性\[ぞくせい\]の文法\[ぶんぽう\]、HTMLのtree構築\[こうちく\]、full HTML適合性\[てきごうせい\]までは検査\[けんさ\]しない。

任意\[にんい\]HTML fragmentを扱\[あつか\]うproviderは、明示的\[めいじてき\]な入力上限\[にゅうりょくじょうげん\]またはheredoc delimiterを受\[う\]け取\[と\]り、その範囲内\[はんいない\]で独自\[どくじ\]parserを実行\[じっこう\]できる。外側\[そとがわ\]prefix parserへ、DOMの子\[こ\]を渡\[わた\]す必要\[ひつよう\]はない。HTMLの安全性検査\[あんぜんせいけんさ\]と内容\[ないよう\]の解釈\[かいしゃく\]はprovider\/domain側\[がわ\]の操作\[そうさ\]であり、token化\[か\]しただけでsafeとしない。

<a name="n-70726f70657274696573"></a>

<a name="7-解析器の性質"></a>

## 7\. 解析器\[かいせきき\]の性質\[せいしつ\]

ReaderPlanの空成功\[くうせいこう\]・再帰\[さいき\]・出力型\[しゅつりょくがた\]・参照先\[さんしょうさき\]を検査\[けんさ\]する。手書\[てが\]きproviderの停止性\[ていしせい\]を一般\[いっぱん\]に証明\[しょうめい\]できるとは扱\[あつか\]わない。trusted native providerは協調的\[きょうちょうてき\]なbudget pollを契約\[けいやく\]とし、untrusted providerはhostの隔離\[かくり\]runnerへ限定\[げんてい\]する。隔離\[かくり\]runnerがない環境\[かんきょう\]ではTrustRequiredで拒否\[きょひ\]し、workspaceから勝手\[かって\]にnativeコードをbuild\/loadしない。

memoizationを使\[つか\]う場合\[ばあい\]は、snapshot、start、limit\/finalInput、reader revision、context\/state digest、provider digestをキーに含\[ふく\]める。cacheを使\[つか\]うことで、現在\[げんざい\]のsourceに属\[ぞく\]さないspanを返\[かえ\]してはならない。性能最適化\[せいのうさいてきか\]は、結果\[けっか\]とdiagnostic codeを変\[か\]えない。

<a name="n-636f6e74696e756174696f6e"></a>

<a name="8-型付き包絡と継続状態"></a>

## 8\. 型付\[かたつ\]き包絡\[ほうらく\]と継続状態\[けいぞくじょうたい\]

`interfaces/reader.json` は `nepl3.reader` revision 1の実\[じつ\]descriptorであり、`reader --write` でproductionの型付\[かたつ\]き登録\[とうろく\]コードを明示生成\[めいじせいせい\]する。ReadRequest、ReadReply、TransformRequest\/Reply、DependentRequest、ReaderPlan、ReaderContinuationを含\[ふく\]む。nativeの借用\[しゃくよう\]ReadRequestと所有\[しょゆう\]OwnedReadRequestは、wire上\[じょう\]の同\[おな\]じReadRequestへ対応\[たいおう\]する。schema登録\[とうろく\]だけで、VMの実行\[じっこう\]・resume・providerの意味検査\[いみけんさ\]が完成\[かんせい\]したことにはならない。

ProviderSignatureのvalueInput\/valueOutput\/stateTypeはreaderが運\[はこ\]ぶ値\[あたい\]の型\[かた\]であり、operationのinput\/outputとは別\[べつ\]である。operation descriptorのinput\/outputは、ReadでReadRequest\/ReadReply、TransformでTransformRequest\/TransformReply、DependentでDependentRequest\/ReadReplyとなる。operationの存在\[そんざい\]、包絡型\[ほうらくがた\]、pureを署名\[しょめい\]と照合\[しょうごう\]する。ReadのvalueInputはUnitとし、包絡中\[ほうらくちゅう\]のNdfValueはさらに署名\[しょめい\]の具体型\[ぐたいがた\]で検査\[けんさ\]する。decode providerにはpureを要求\[ようきゅう\]し、その他\[ほか\]の外部効果\[がいぶこうか\]はhostが明示\[めいじ\]した権限\[けんげん\]と操作契約\[そうさけいやく\]で扱\[あつか\]う。

TransformReplyは、outcome、sources、sourceMaps、reportを共通\[きょうつう\]fieldとして持\[も\]つ。outcomeはComplete\(value\, view\, facts\)、Failed\(diagnostic\, recovery\)、Stopped\(reason\)である。Completeだけを署名\[しょめい\]のvalueOutputへ照合\[しょうごう\]する。Failedのprimary診断\[しんだん\]は、Report内\[ない\]に一度存在\[いちどそんざい\]する診断\[しんだん\]の再掲\[さいけい\]とする。MapとDecodeのどちらも、Failed\/StoppedをそのままReaderの正式\[せいしき\]な失敗\[しっぱい\]・停止\[ていし\]へ伝播\[でんぱ\]し、生成\[せいせい\]source上\[じょう\]の診断\[しんだん\]・関連位置\[かんれんいち\]・mapを共通閉包\[きょうつうへいほう\]に保存\[ほぞん\]する。成功\[せいこう\]Unitへ置換\[ちかん\]しない。traceOverflowを持\[も\]つ非\[ひ\]Stopped返信\[へんしん\]は拒否\[きょひ\]する。Stoppedでは、理由\[りゆう\]がEventLimit以外\[いがい\]でも終了\[しゅうりょう\]eventのoverflowを保持\[ほじ\]できる。

provider返信\[へんしん\]は、保存要求\[ほぞんようきゅう\]・署名\[しょめい\]・入力範囲\[にゅうりょくはんい\]・state・view・source\/map・Reportを借用\[しゃくよう\]したまま先\[さき\]に検査\[けんさ\]し、受理後\[じゅりご\]に正式\[せいしき\]collectorへ移動\[いどう\]する。不正\[ふせい\]な非停止返信\[ひていしへんしん\]は待機要求\[たいきようきゅう\]を消費\[しょうひ\]せず、同\[おな\]じcontinuationへ訂正返信\[ていせいへんしん\]を返\[かえ\]せる。Readerを包\[つつ\]むTokenizerとparserも、同\[おな\]じ待機状態\[たいきじょうたい\]を保\[たも\]つ。検査\[けんさ\]で使\[つか\]ったWorkやSourceAdmissionは返却\[へんきゃく\]しない。検査中\[けんさちゅう\]に資源停止\[しげんていし\]した場合\[ばあい\]は、返信\[へんしん\]の未受理\[みじゅり\]artifactを混\[ま\]ぜず、それまでの正式\[せいしき\]collectorをStoppedへ保持\[ほじ\]する。

現在\[げんざい\]のTransform portable返信\[へんしん\]adapterは、ReaderSessionが発行\[はっこう\]する保存待機要求\[ほぞんたいきようきゅう\]の借用\[しゃくよう\]proofに結\[むす\]び付\[つ\]く。nativeとNDFの両経路\[りょうけいろ\]で同\[おな\]じ返信検査\[へんしんけんさ\]を使\[つか\]い、sourceの解決\[かいけつ\]を保存要求\[ほぞんようきゅう\]・正式\[せいしき\]checkpoint・返信\[へんしん\]の明示\[めいじ\]tableへ閉\[と\]じる。このproofは初回\[しょかい\]TransformRequestの独立\[どくりつ\]provider受信\[じゅしん\]decoderではなく、Report\.usageにも同\[おな\]じ操作\[そうさ\]で観測\[かんそく\]した累積\[るいせき\]Usageを要求\[ようきゅう\]する。freshな別操作\[べつそうさ\]へ任意\[にんい\]のReportを再\[さい\]serializeする機能\[きのう\]、通信\[つうしん\]の認証\[にんしょう\]、remote Usageの吸収\[きゅうしゅう\]、Reader\/Tokenizer全継続\[ぜんけいぞく\]codecの完成\[かんせい\]とは区別\[くべつ\]する。

ReaderPlanは、ReaderIdで参照\[さんしょう\]するarenaと名前付\[なまえつ\]きruleを持\[も\]つ。seqは `List<NdfValue>`、lookは成功時\[せいこうじ\]Unitを返\[かえ\]す。Expectationは、Literal、CharClassを直接持\[ちょくせつも\]つScalarClass、EndOfInput、TokenBoundary、型付\[かたつ\]きProvider要求\[ようきゅう\]を区別\[くべつ\]する。CharClass\.Rangeの両端\[りょうたん\]TextはUnicode scalarを一\[ひと\]つだけ持\[も\]ち、その順序\[じゅんじょ\]を検査\[けんさ\]する。Name\/Natなどは、finalInput\=falseの境界\[きょうかい\]で後続判定\[こうぞくはんてい\]が未確定\[みかくてい\]ならNeedMoreとし、短\[みじか\]いtokenを先\[さき\]に確定\[かくてい\]しない。

expected集合\[しゅうごう\]の列\[れつ\]は、最初\[さいしょ\]の宣言\[せんげん\]・試行\[しこう\]で現\[あらわ\]れた順\[じゅん\]とし、同\[おな\]じtyped値\[ち\]の重複\[じゅうふく\]を除\[のぞ\]く。同一実装\[どういつじっそう\]では列順\[れつじゅん\]まで決定的\[けっていてき\]にし、別実装\[べつじっそう\]の意味比較\[いみひかく\]ではtyped集合\[しゅうごう\]を比較\[ひかく\]する。provider argumentsを表示文字列\[ひょうじもじれつ\]へ変換\[へんかん\]して、同一性\[どういつせい\]を推測\[すいそく\]しない。

checkpointはcursor\/state\/view\/facts\/diagnostics\/events\/source\/sourceMapとtraceOverflowを保存\[ほぞん\]する。失敗候補\[しっぱいこうほ\]の結果\[けっか\]は巻\[ま\]き戻\[もど\]すが、既\[すで\]に使\[つか\]ったwork・diagnostic\/event counter・source admissionは返却\[へんきゃく\]しない。decodeの生成\[せいせい\]sourceと対応\[たいおう\]mapにも、同\[おな\]じtransaction規則\[きそく\]を適用\[てきよう\]する。Failed\.diagnosticはreport\.diagnosticsに含\[ふく\]まれる同\[おな\]じprimary診断\[しんだん\]の再掲\[さいけい\]であり、診断件数\[しんだんけんすう\]は一度\[いちど\]だけ計上\[けいじょう\]する。

ReadReplyの全\[ぜん\]terminal（Matched\/NoMatch\/NeedMore\/Failed\/Stopped）はsources\/sourceMapsを所有\[しょゆう\]し、巻戻\[まきもど\]し後\[ご\]の正式\[せいしき\]ReportのSpanを、入力宣言\[にゅうりょくせんげん\]と返却\[へんきゃく\]artifactから解決可能\[かいけつかのう\]にする。生成\[せいせい\]sourceの受理後\[じゅりご\]に後段\[こうだん\]が失敗\[しっぱい\]しても、診断\[しんだん\]だけを残\[のこ\]してsourceを捨\[す\]てない。Awaitでは、continuation\.request\.sourcesとcontinuation\.current\.sources\/sourceMapsを閉包\[へいほう\]の正本\[せいほん\]とする。内部\[ないぶ\]でReportを別\[べつ\]collectorへ引\[ひ\]き継\[つ\]ぐ際\[さい\]も、対応\[たいおう\]する正式\[せいしき\]source\/mapsを一緒\[いっしょ\]に扱\[あつか\]う。hostのglobal SourceStoreへの先行\[せんこう\]commitで、欠損\[けっそん\]を補\[おぎな\]わない。

Reply\.sourcesは生成\[せいせい\]・追加\[ついか\]snapshotの差分\[さぶん\]tableであり、対応\[たいおう\]するReadRequest\.sourcesと合成\[ごうせい\]して閉\[と\]じる。受信側\[じゅしんがわ\]は必\[かなら\]ず対応\[たいおう\]するrequest\/session slotへreplyを結\[むす\]び付\[つ\]け、異\[こと\]なるrequestのtableを混用\[こんよう\]しない。重複\[じゅうふく\]identityには同\[おな\]じbytes\/digest\/locatorだけを許\[ゆる\]し、Span・map端点\[たんてん\]・Report関連位置\[かんれんいち\]は合成\[ごうせい\]した宣言\[せんげん\]tableだけで解決\[かいけつ\]する。追加\[ついか\]sourceだけを単独保存\[たんどくほぞん\]して自己完結\[じこかんけつ\]したreplyと扱\[あつか\]わず、wire\/providerの記録\[きろく\]にも対応\[たいおう\]requestを保持\[ほじ\]する。providerのNoMatch\/NeedMoreは、その候補\[こうほ\]の追加\[ついか\]artifactを巻\[ま\]き戻\[もど\]して空\[から\]にする。VM全体\[ぜんたい\]の結果\[けっか\]は、既\[すで\]に正式\[せいしき\]なprefixから引\[ひ\]き継\[つ\]いだ閉包\[へいほう\]を保持\[ほじ\]できる。

TokenizationReplyはoutcomeのほか、cursor、newState、trivia、facts、sources、sourceMaps、reportを持\[も\]つ。先行\[せんこう\]して成功\[せいこう\]したskipの成果物\[せいかぶつ\]も、End\/NoMatch\/NeedMore\/Failed\/Stoppedへ保持\[ほじ\]する。newState\=Noneは、開始\[かいし\]checkpointを複製\[ふくせい\]する予算\[よさん\]がなくStoppedとなった場合\[ばあい\]だけであり、通常\[つうじょう\]はSome（実際\[じっさい\]の状態\[じょうたい\]）とする。Token\.leadingTriviaと共通\[きょうつう\]triviaは同\[おな\]じ内容\[ないよう\]の再掲\[さいけい\]であり、複製\[ふくせい\]storageだけを計上\[けいじょう\]する。各\[かく\]skip成功\[せいこう\]lexemeは、BOM、ASCII whitespace、単一\[たんいつ\]の\#commentとして全体一致\[ぜんたいいっち\]する場合\[ばあい\]に、その種別\[しゅべつ\]を持\[も\]つ。それ以外\[いがい\]はSkippedとして、無解釈\[むかいしゃく\]の元範囲\[もとはんい\]を保存\[ほぞん\]する。既存\[きそん\]Grammarのskip宣言\[せんげん\]にない種別\[しゅべつ\]を捏造\[ねつぞう\]しない。

Text予約\[よやく\]が必要\[ひつよう\]なtokenizerはReserveを返\[かえ\]し、sessionId\/requestId\/snapshot\/start\/limitの要求\[ようきゅう\]と、privateなmode\/context\/candidateを固定\[こてい\]する。hostから戻\[もど\]る予約\[よやく\]をそのslotと照合\[しょうごう\]し、入力\[にゅうりょく\]がquoteに一致\[いっち\]しない候補\[こうほ\]では予約\[よやく\]を要求\[ようきゅう\]しない。native TokenizationRequestはsnapshot\/contextとstateの借用期間\[しゃくようきかん\]を分\[わ\]けるが、所有\[しょゆう\]ReadRequestと同\[おな\]じデータ契約\[けいやく\]である。tokenizer固有\[こゆう\]の継続全体\[けいぞくぜんたい\]のportable codecが完成\[かんせい\]するまでは、nativeのprivate session状態\[じょうたい\]を汎用\[はんよう\]ReaderContinuationと同\[おな\]じものとして広告\[こうこく\]しない。

TokenizationContinuationは、所有\[しょゆう\]ReadRequest、mode、TokenTarget、Skip\/Takeの候補位置\[こうほいち\]、checkpoint、trivia、expected\/furthest、pending予約\[よやく\]または内側\[うちがわ\]ReaderContinuation、Usage\/Reportを保持\[ほじ\]する。configurationDigestには、名前順\[なまえじゅん\]のmode宣言\[せんげん\]と順序\[じゅんじょ\]を保\[たも\]つskip\/take、および具体\[ぐたい\]ReaderPlan identityを含\[ふく\]める。TokenTargetは要求\[ようきゅう\]ごとに保存\[ほぞん\]し、configurationだけから推測\[すいそく\]しない。再開\[さいかい\]では、外側継続全体\[そとがわけいぞくぜんたい\]をprivate slotと照合\[しょうごう\]してから、内側\[うちがわ\]readerを再開\[さいかい\]する。schema上\[じょう\]の状態定義\[じょうたいていぎ\]、nativeの所有継続\[しょゆうけいぞく\]、portable codecの実装範囲\[じっそうはんい\]は区別\[くべつ\]する。

TokenizationScopeは、hostが割\[わ\]り当\[あ\]てるoperationId、profileDigest、入力\[にゅうりょく\]snapshotを固定\[こてい\]する。同\[おな\]じparse operationが複数\[ふくすう\]aliasのtokenizerを呼\[よ\]ぶ場合\[ばあい\]も、共通\[きょうつう\]scopeと共有\[きょうゆう\]Budget\/SourceAdmissionを使\[つか\]う。AcceptedTokenizationReportは、既受理\[きじゅり\]のReportとsource\/mapsを移動\[いどう\]して引\[ひ\]き継\[つ\]ぐnativeのprivate proofであり、任意\[にんい\]のraw Reportからは構築\[こうちく\]できない。scope、Limits、単調\[たんちょう\]なUsageを照合\[しょうごう\]し、異\[こと\]なる操作\[そうさ\]のcollectorを混用\[こんよう\]しない。これらの値\[あたい\]をコピーしただけで、同一\[どういつ\]のBudget実体\[じったい\]が証明\[しょうめい\]されるとは扱\[あつか\]わない。operationIdの一意性\[いちいせい\]と共有\[きょうゆう\]ledgerの維持\[いじ\]は、host契約\[けいやく\]とする。停止時\[ていしじ\]も既受理\[きじゅり\]collectorを保持\[ほじ\]し、停止後\[ていしご\]のflattenや複製\[ふくせい\]に失敗\[しっぱい\]して診断\[しんだん\]だけを消\[け\]さない。

このproofのdiagnostic入口\[いりぐち\]は、明示\[めいじ\]された宣言\[せんげん\]SourceStoreからprimary\/related\/fixの全\[ぜん\]snapshotを検査\[けんさ\]する。それを共有\[きょうゆう\]SourceAdmissionへ一度\[いちど\]だけ受\[う\]け入\[い\]れ、予算内\[よさんない\]で所有\[しょゆう\]source閉包\[へいほう\]と診断\[しんだん\]を同時追加\[どうじついか\]する。単\[たん\]なるraw Reportのproof化\[か\]ではない。未宣言\[みせんげん\]source、identity\/locatorの不一致\[ふいっち\]、schemaの不一致\[ふいっち\]を拒否\[きょひ\]する。追加準備\[ついかじゅんび\]が停止\[ていし\]した場合\[ばあい\]は既受理\[きじゅり\]collectorを保存\[ほぞん\]し、失敗\[しっぱい\]した準備\[じゅんび\]のwork\/admissionは返却\[へんきゃく\]しない。

Repeat frameは元\[もと\]のstart\/checkpointを維持\[いじ\]したまま、最後\[さいご\]の反復開始位置\[はんぷくかいしいち\]をiterationStartへ別\[べつ\]に保存\[ほぞん\]する。NeedMoreやmin未満\[みまん\]の失敗\[しっぱい\]で、最後\[さいご\]の成功反復\[せいこうはんぷく\]までを部分成功\[ぶぶんせいこう\]として確定\[かくてい\]しない。

Awaitでは、pending ProviderCall、plan identity、snapshot、frame列\[れつ\]、checkpoint、累積\[るいせき\]Usage\/Reportを保存\[ほぞん\]する。Await\.reportとcontinuation\.report、およびcontinuation\.usageとreport\.usageは一致\[いっち\]させる。callのdepthBaseを絶対的\[ぜったいてき\]な停止中深\[ていしちゅうふか\]さとして保持\[ほじ\]する。hostは現在\[げんざい\]の深\[ふか\]さを下\[さ\]げず、そのbase以上\[いじょう\]の共有\[きょうゆう\]budgetでproviderを実行\[じっこう\]する。wireからの自己申告\[じこしんこく\]baseだけを信用\[しんよう\]せず、保存\[ほぞん\]したpending slot\/frameと照合\[しょうごう\]する。providerがさらにAwaitした場合\[ばあい\]は、hostがその依存呼出\[いぞんよびだ\]しを解決\[かいけつ\]してから、終端\[しゅうたん\]replyを待機\[たいき\]frameへ戻\[もど\]す。

ProviderCallとReaderContinuationは、hostが割\[わ\]り当\[あ\]てる空\[から\]でないopaque sessionIdを持\[も\]つ。callIdはそのsession内\[ない\]で一意\[いちい\]とし、coreは乱数\[らんすう\]や時計\[とけい\]でIDを生成\[せいせい\]しない。plan\/source\/stateが偶然同一\[ぐうぜんどういつ\]の別\[べつ\]sessionから届\[とど\]いた継続\[けいぞく\]でも、保存\[ほぞん\]されたsessionId\/callIdと違\[ちが\]えば拒否\[きょひ\]する。close済\[ず\]みsessionはresumeできない。これらは輸送上\[ゆそうじょう\]の要求\[ようきゅう\]identityであり、意味値\[いみち\]の比較\[ひかく\]runnerは別実装間\[べつじっそうかん\]のsession対応\[たいおう\]を明示\[めいじ\]する。

native providerは同\[おな\]じBudgetを使\[つか\]う。外部\[がいぶ\]providerの累積\[るいせき\]Usageが保存時\[ほぞんじ\]の使用量以上\[しようりょういじょう\]であることを検査\[けんさ\]し、その増分\[ぞうぶん\]をhostの共有\[きょうゆう\]budgetへ計上\[けいじょう\]してからresumeする。上限\[じょうげん\]のresetや、自己申告値\[じこしんこくち\]の無検査代入\[むけんさだいにゅう\]をしない。VMは、報告使用量\[ほうこくしようりょう\]がhostの現在使用量\[げんざいしようりょう\]を超\[こ\]えず、正式\[せいしき\]diagnostic\/event件数\[けんすう\]が計上済\[けいじょうず\]みcounterに収\[おさ\]まることを確認\[かくにん\]する。転送\[てんそう\]・複製\[ふくせい\]の割当\[わりあて\]は別途計上\[べっとけいじょう\]し、同\[おな\]じ診断\[しんだん\]\/eventの件数\[けんすう\]を二重計上\[にじゅうけいじょう\]しない。

現在\[げんざい\]のReaderPlan digestはarena indexを含\[ふく\]む実行\[じっこう\]plan identityであり、continuationの別\[べつ\]planへの誤適用\[ごてきよう\]を防\[ふせ\]ぐ。Grammar packageの意味\[いみ\]digestとは別\[べつ\]である。package比較\[ひかく\]では、ruleの名前順\[なまえじゅん\]、reader参照\[さんしょう\]の解決\[かいけつ\]、意味上\[いみじょう\]の順序\[じゅんじょ\]を正規化\[せいきか\]する。arena配置\[はいち\]だけの差\[さ\]を、意味\[いみ\]の違\[ちが\]いとして扱\[あつか\]わない。context cacheにはEnvironmentEntryの内容\[ないよう\]digestだけでなく、13章\[しょう\]に従\[したが\]うOrigin\/source bundle identityを含\[ふく\]める。

nativeのReadRequestは、CheckedReaderContextを要求\[ようきゅう\]する。raw ReaderContextは、実際\[じっさい\]のEnvironment内容\[ないよう\]digest、選択\[せんたく\]schema、Origin、binding、resourceを検査\[けんさ\]してからprivate proofを得\[え\]る。coreのFoundationValueCodec境界\[きょうかい\]はwire実装\[じっそう\]が提供\[ていきょう\]し、readerからwireへのproduction依存\[いぞん\]を作\[つく\]らない。環境\[かんきょう\]の交換表現\[こうかんひょうげん\]はcontextの準備時\[じゅんびじ\]に検査\[けんさ\]し、通常\[つうじょう\]のVMがtokenごとにserializeする構成\[こうせい\]にはしない。

検査済\[けんさず\]みcontextは、OriginのDirect\/callsite\/anchorが参照\[さんしょう\]するsnapshotの正確\[せいかく\]な閉包\[へいほう\]を持\[も\]つ。portable ReadRequest\.sourcesには、入力\[にゅうりょく\]snapshot、この閉包\[へいほう\]、現在\[げんざい\]の正式\[せいしき\]な生成\[せいせい\]sourceを重複\[じゅうふく\]なく含\[ふく\]め、無関係\[むかんけい\]なglobal SourceStore全件\[ぜんけん\]を転送\[てんそう\]しない。Span\/Originの解決\[かいけつ\]は、要求\[ようきゅう\]が宣言\[せんげん\]したsource tableへ閉\[と\]じる。proofを別\[べつ\]operationで再利用\[さいりよう\]するときも、そのoperationのSourceAdmissionへ必要\[ひつよう\]snapshotを改\[あらた\]めて受\[う\]け入\[い\]れる。前\[まえ\]のBudgetの計上\[けいじょう\]を、引\[ひ\]き継\[つ\]いだとみなしてはならない。resumeのprivate constructorは保存\[ほぞん\]したsession slotの完全照合後\[かんぜんしょうごうご\]にのみ使\[つか\]え、未検査\[みけんさ\]の外部\[がいぶ\]contextへproofを付\[つ\]けない。

Local\/WithModeのretargetは検査済\[けんさず\]みenvironmentとOriginを明示的\[めいじてき\]に再利用\[さいりよう\]し、schema\/category\/modeだけを変更\[へんこう\]する所有\[しょゆう\]proofを予算付\[よさんつ\]きで構成\[こうせい\]できる。利用先\[りようさき\]registryと正確\[せいかく\]なsource閉包\[へいほう\]を再照合\[さいしょうごう\]し、元\[もと\]のraw contextの借用期間\[しゃくようきかん\]を延長\[えんちょう\]する必要\[ひつよう\]はない。Foreignの環境\[かんきょう\]は、Profileで指定\[してい\]した検査済\[けんさず\]みprojectionか、明示的\[めいじてき\]な空環境\[くうかんきょう\]を境界\[きょうかい\]で準備\[じゅんび\]する。hostの環境\[かんきょう\]を暗黙\[あんもく\]に継承\[けいしょう\]しない。

<a name="n-73796e6368726f6e6f7573"></a>

<a name="同期tokenizer-host境界"></a>

### 同期\[どうき\]tokenizer host境界\[きょうかい\]

`read_accepted_with_host` は、操作内\[そうさない\]の明示\[めいじ\]hostへproviderとText予約\[よやく\]を依頼\[いらい\]するnative接続\[せつぞく\]である。成功\[せいこう\]した同期\[どうき\]callでは外向\[そとむ\]けTokenizationContinuationを作\[つく\]らず、既存\[きそん\]readerのreply検査\[けんさ\]・source閉包\[へいほう\]・候補巻戻\[こうほまきもど\]しを通\[とお\]す。外部\[がいぶ\]から継続\[けいぞく\]を受\[う\]け取\[と\]る経路\[けいろ\]では、ReaderContinuationのecho検査\[けんさ\]を維持\[いじ\]する。同期\[どうき\]callbackはProviderCallを借用\[しゃくよう\]するだけで継続\[けいぞく\]を供給\[きょうきゅう\]しないため、排他的\[はいたてき\]に保持\[ほじ\]したreaderのprivate slotを再開\[さいかい\]する。callのsignature・範囲\[はんい\]・source・返却値\[へんきゃくち\]・reportの検査\[けんさ\]は、共通\[きょうつう\]check\_providerで行\[おこな\]う。Read\/Dependentの同期返信\[どうきへんしん\]は元\[もと\]Machineの所有中\[しょゆうちゅう\]に検査\[けんさ\]・適用\[てきよう\]し、外部待機\[がいぶたいき\]か不正返信\[ふせいへんしん\]の場合\[ばあい\]だけ完全\[かんぜん\]なReaderContinuationを生成\[せいせい\]する。Transformは既存\[きそん\]の保存\[ほぞん\]context経路\[けいろ\]を使\[つか\]う。callbackの前後\[ぜんご\]で元\[もと\]LimitsとUsageの単調性\[たんちょうせい\]を照合\[しょうごう\]し、hostによるBudgetの交換\[こうかん\]を正常\[せいじょう\]な応答\[おうとう\]として採用\[さいよう\]しない。

同\[おな\]じsignatureの合法値\[ごうほうち\]であっても、別\[べつ\]call由来\[ゆらい\]のreplyをpayloadだけで識別\[しきべつ\]できるとは保証\[ほしょう\]しない。呼出\[よびだ\]しの対応付\[たいおうづ\]けは、hostの責務\[せきむ\]である。Noneは通常\[つうじょう\]の所有\[しょゆう\]Await\/Reserveへ戻\[もど\]り、callbackの非停止\[ひていし\]エラーもその境界\[きょうかい\]と元\[もと\]エラーを返\[かえ\]す。不正\[ふせい\]なprovider replyはreaderで拒否\[きょひ\]し、tokenizer入口\[いりぐち\]では所有\[しょゆう\]Awaitを保持\[ほじ\]する。不正\[ふせい\]な予約値\[よやくち\]は通常\[つうじょう\]のreserveと同\[おな\]じ検査\[けんさ\]エラーであり、callbackの輸送失敗\[ゆそうしっぱい\]とは区別\[くべつ\]する。停止原因\[ていしげんいん\]を持\[も\]つcallbackエラーは、Budgetへ同\[おな\]じ理由\[りゆう\]で固定\[こてい\]し、正常\[せいじょう\]な待機\[たいき\]へ置換\[ちかん\]しない。

prefix engineのnative接続\[せつぞく\]は、Profileの選択済\[せんたくず\]みProviderRequirementと実装\[じっそう\]catalogを照合\[しょうごう\]して、この経路\[けいろ\]を使\[つか\]う。未対応\[みたいおう\]callをその場\[ば\]で二重\[にじゅう\]dispatchせず、所有境界\[しょゆうきょうかい\]へ戻\[もど\]す。engineでの不正\[ふせい\]provider replyの扱\[あつか\]いは、従来\[じゅうらい\]どおり解析\[かいせき\]エラーである。公開\[こうかい\]wire型\[かた\]や別\[べつ\]processの照合\[しょうごう\]を省略\[しょうりゃく\]する許可\[きょか\]ではなく、同期呼出\[どうきよびだ\]しで不要\[ふよう\]なコピーだけを避\[さ\]ける。

providerの追加\[ついか\]map、viewの要素\[ようそ\]とroot、factsがすべて空\[から\]なら、新\[あら\]たな位置対応\[いちたいおう\]の検査対象\[けんさたいしょう\]はない。この場合\[ばあい\]だけ、private checkpointで保持\[ほじ\]する既存\[きそん\]map列\[れつ\]の複製\[ふくせい\]とgraph再検査\[さいけんさ\]を省\[はぶ\]く。既存\[きそん\]mapは受理済\[じゅりず\]みprovider結果\[けっか\]か、予約\[よやく\]した新\[しん\]snapshotへ正\[ただ\]しい対応\[たいおう\]を構成\[こうせい\]する組込\[くみこ\]みreaderに由来\[ゆらい\]する。返却\[へんきゃく\]source、値\[あたい\]・state、report、停止\[ていし\]・失敗条件\[しっぱいじょうけん\]は従来\[じゅうらい\]どおり検査\[けんさ\]する。追加\[ついか\]mapまたはview\/factがあれば既存\[きそん\]mapとの和集合\[わしゅうごう\]を検査\[けんさ\]し、後\[あと\]から循環\[じゅんかん\]を作\[つく\]る追加\[ついか\]も拒否\[きょひ\]する。外部\[がいぶ\]から受\[う\]け取\[と\]ったraw mapへ、この省略条件\[しょうりゃくじょうけん\]だけで検査済\[けんさず\]みproofを付\[つ\]けることはない。

さらに返却\[へんきゃく\]source・map・view・facts・診断\[しんだん\]・eventがすべて空\[から\]で、Failedでもない返信\[へんしん\]では、参照位置\[さんしょういち\]の解決\[かいけつ\]に使\[つか\]うSourceStoreを再構築\[さいこうちく\]しない。reportのUsage・overflow、値\[あたい\]・state、終端範囲\[しゅうたんはんい\]・期待値\[きたいち\]などの検査\[けんさ\]は維持\[いじ\]する。NoMatch\/NeedMoreには、従来\[じゅうらい\]どおり空\[から\]の返却\[へんきゃく\]artifactを要求\[ようきゅう\]する。これは検査済\[けんさず\]みのprivate request\/checkpointに対\[たい\]する処理\[しょり\]であり、外部\[がいぶ\]source宣言\[せんげん\]の検査省略\[けんさしょうりゃく\]ではない。

tokenizerが受理済\[じゅりず\]みprefixを次\[つぎ\]のreaderへ渡\[わた\]すときは、まず全\[ぜん\]sourceの競合\[きょうごう\]と操作内\[そうさない\]admissionを検査\[けんさ\]する。診断\[しんだん\]・eventのないreportでは、この同\[おな\]じsource索引\[さくいん\]をreport検査用\[けんさよう\]に再構築\[さいこうちく\]せず、Usage\/overflowを検査\[けんさ\]する。providerの適用\[てきよう\]へ渡\[わた\]すframeはprivate状態\[じょうたい\]から取\[と\]り出\[だ\]した所有値\[しょゆうち\]なので、NoMatch\/NeedMoreの巻戻\[まきもど\]しではcheckpointを再複製\[さいふくせい\]せず、所有権\[しょゆうけん\]を戻\[もど\]す。正式\[せいしき\]artifactの喪失\[そうしつ\]や、予算\[よさん\]の払戻\[はらいもど\]しを伴\[ともな\]わない。

native tokenizerの`read_with_accepted_recover`と`read_accepted_with_host_recover`は、hard errorでも呼出\[よびだ\]し開始時\[かいしじ\]の受理済\[じゅりず\]みcollectorを所有値\[しょゆうち\]として返\[かえ\]す。診断\[しんだん\]・event・source・SourceMapのprivate append\-only prefixを四\[よっ\]つの長\[なが\]さで記録\[きろく\]し、hard error時\[じ\]だけ全長\[ぜんちょう\]を検査\[けんさ\]して追加分\[ついかぶん\]を取\[と\]り除\[のぞ\]き、開始時\[かいしじ\]のtrace overflowへ戻\[もど\]す。scopeとLimitsは元\[もと\]の値\[あたい\]を保持\[ほじ\]し、消費済\[しょうひず\]みUsageは払\[はら\]い戻\[もど\]さない。不正\[ふせい\]なfresh Budgetや別\[べつ\]Limitsの拒否\[きょひ\]では、そのBudgetのUsageで既受理\[きじゅり\]の記録\[きろく\]を上書\[うわが\]きしない。正常\[せいじょう\]なStoppedはこの巻戻\[まきもど\]しを行\[おこな\]わずliveの正式\[せいしき\]collectorを返\[かえ\]す。Await\/Reserveは引\[ひ\]き続\[つづ\]き所有\[しょゆう\]continuationを返\[かえ\]し、wire境界\[きょうかい\]の検査\[けんさ\]も維持\[いじ\]する。

private prefixより実際\[じっさい\]の長\[なが\]さが短\[みじか\]い場合\[ばあい\]は内部整合性\[ないぶせいごうせい\]の破損\[はそん\]であり、Recoverableと扱\[あつか\]わない。この場合\[ばあい\]はBrokenPrefixとして元\[もと\]のerrorと観測済\[かんそくず\]み停止理由\[ていしりゆう\]を返\[かえ\]し、tokenizerを閉\[と\]じる。不完全\[ふかんぜん\]なcollectorにAccepted proofを付\[つ\]けず、呼出\[よびだ\]し側\[がわ\]もその操作\[そうさ\]を終了\[しゅうりょう\]する。この回収\[かいしゅう\]APIはnative所有権\[しょゆうけん\]の補助契約\[ほじょけいやく\]であり、公開\[こうかい\]wire型\[かた\]・言語\[げんご\]の意味\[いみ\]・外部\[がいぶ\]providerの検査済\[けんさず\]み条件\[じょうけん\]を変更\[へんこう\]するものではない。既存\[きそん\]のerrorのみを返\[かえ\]すAPIも維持\[いじ\]する。

native hostの返信\[へんしん\]は、同\[おな\]じ呼出\[よびだ\]しのprivate prefix markを保持\[ほじ\]するRecoverableHostReplyとして一時的\[いちじてき\]に受\[う\]け取\[と\]れる。parserは従来\[じゅうらい\]の条件\[じょうけん\]でcallback輸送\[ゆそう\]エラーとreader側\[がわ\]の拒否\[きょひ\]を区別\[くべつ\]し、前者\[ぜんしゃ\]はowned Await\/Reserveとして受理\[じゅり\]し、後者\[こうしゃ\]は元\[もと\]prefixへ戻\[もど\]して解析\[かいせき\]エラーにする。正常\[せいじょう\]なStoppedのcollectorは巻\[ま\]き戻\[もど\]さず保持\[ほじ\]する。BudgetのLimits\/Usage改変\[かいへん\]は別\[べつ\]の整合性違反\[せいごうせいいはん\]である。各\[かく\]provider\/reservation callbackの直前\[ちょくぜん\]と直後\[ちょくご\]でLimits・Usage・現在\[げんざい\]depth・既存停止\[きそんていし\]を照合\[しょうごう\]し、改変\[かいへん\]の検出\[けんしゅつ\]は返信生成\[へんしんせいせい\]やcallback輸送\[ゆそう\]エラーとは別\[べつ\]に保持\[ほじ\]する。後\[あと\]からCancelledを付\[つ\]けたりErrを返\[かえ\]したりしても正常\[せいじょう\]な停止\[ていし\]や再試行可能\[さいしこうかのう\]なAwaitに戻\[もど\]さない。拒否\[きょひ\]した呼出\[よびだ\]しのpendingは破棄\[はき\]する。復元不能\[ふくげんふのう\]なBrokenCollectorはparserを閉\[と\]じ、同時\[どうじ\]に予算\[よさん\]が停止\[ていし\]していても不完全\[ふかんぜん\]なprogress付\[つ\]きStoppedへ変換\[へんかん\]しない。

この経路\[けいろ\]ではtokenを読\[よ\]むたびのcollector退避\[たいひ\]コピーは不要\[ふよう\]となる。深\[ふか\]さの事前検査\[じぜんけんさ\]が通\[とお\]るまでcollectorをparserから移動\[いどう\]せず、回収可能\[かいしゅうかのう\]なエラーでは同\[おな\]じ所有値\[しょゆうち\]をparserへ戻\[もど\]す。外部\[がいぶ\]へ公開\[こうかい\]するAwait\/Reserve、再開可能\[さいかいかのう\]なNeedMoreの所有\[しょゆう\]checkpointは引\[ひ\]き続\[つづ\]き必要\[ひつよう\]である。

parser内部\[ないぶ\]で完成木\[かんせいき\]を検査\[けんさ\]するときは、検査用\[けんさよう\]に解析\[かいせき\]progress全体\[ぜんたい\]を複製\[ふくせい\]しない。最後\[さいご\]のarena、対応\[たいおう\]するcontext、recoveryを一時的\[いちじてき\]に検査対象\[けんさたいしょう\]の木\[き\]へ移\[うつ\]し、同\[おな\]じ木検査\[きけんさ\]を実行\[じっこう\]する。追加\[ついか\]contextの領域\[りょういき\]とrootの存在\[そんざい\]は移動前\[いどうまえ\]に確認\[かくにん\]し、検査\[けんさ\]が成功\[せいこう\]・不正\[ふせい\]・資源停止\[しげんていし\]のいずれで終\[お\]わっても、source・Origin・token・SourceMap・environmentを含\[ふく\]む所有値\[しょゆうち\]を元\[もと\]のprogressへ戻\[もど\]す。停止時\[ていしじ\]の正式\[せいしき\]progressや診断\[しんだん\]のsource参照\[さんしょう\]を欠落\[けつらく\]させず、消費\[しょうひ\]した予算\[よさん\]も払\[はら\]い戻\[もど\]さない。これはnative内部\[ないぶ\]の所有権移動\[しょゆうけんいどう\]であり、外部\[がいぶ\]から受\[う\]け取\[と\]った木\[き\]の再検査\[さいけんさ\]を省略\[しょうりゃく\]する根拠\[こんきょ\]にはしない。
