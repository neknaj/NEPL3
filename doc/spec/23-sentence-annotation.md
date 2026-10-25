<!-- Generated from doc/spec/23&#45;sentence&#45;annotation.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page sentence&#45;annotation; source SHA-256 b74d6d4efece258a70884cb70054975cff1eb3517ab79b57cd93fb8ca160b2fb; alias input SHA-256 800c58010fe3ca5fb3c10997a3b7908fefb312ba31edec1c0c8cb409f1bb0b29; page input SHA-256 cad8756a033470a5bed111b37dffa9f2f146f1bf7e511e6219f8672f12b26814. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="23-構造化文章と対象付き注釈への是正"></a>

# 23\. <ruby>構造化文章<rt>こうぞうかぶんしょう</rt></ruby>と<ruby>対象付<rt>たいしょうつ</rt></ruby>き<ruby>注釈<rt>ちゅうしゃく</rt></ruby>への<ruby>是正<rt>ぜせい</rt></ruby>

[正本（NEPL3d）](<23-sentence-annotation.nepld>)

この章は[\#158の統合設計](<\.\.\/decisions\/multilanguage\-hca\.md>)を具体化した、Sentenceと対象付きannotationの規範契約である。契約の採用と実装完了は別であり、実装・正式受入の状態は [実装状態の正本](<\.\.\/\.\.\/implementation\-status\.json>) を参照する。

入出力・意味・失敗条件・source閉包・資源停止はこの章で定義する。`literal::read`等のRust名は現在の実装入口を指し、他言語の実装にRust ABIや内部module構成を要求しない。[Doc runtimeの記録](<\.\.\/progress\/doc\-runtime\.md>) は現在の接続範囲と履歴を説明し、[T07\/T19\/T21のタスク定義](<\.\.\/\.\.\/design\/tasks\.json>) が移行順と成果物を所有する。

<a name="n-6f776e657273"></a>

<a name="所有者と実装順"></a>

## <ruby>所有者<rt>しょゆうしゃ</rt></ruby>と<ruby>実装順<rt>じっそうじゅん</rt></ruby>

NEPL3sentenceは独自LanguagePackage、Sentence root、sentence literalと前置構築、 単独parse\/check\/printを持つ文章言語である。NEPL3aはSentenceと対象syntaxの付与関係、 NEPL3dは文書構造を所有する。Doc本文をA経由にしない。 Sentence coreはfoundationだけへ依存し、Doc\/Math\/A\/engine\/hostへ依存しない。 reader adapter、language package compile、各hostとの橋渡しは外側へ置く。

この節の見出しは既存参照のため維持する。実装順の正本はT07であり、本文は所有境界を定める。 Doc本文とannotationはそれぞれSentenceを使用し、Aの完成をD本文や公開の前提にしない。 途中のモデル検査成功を独立言語の完成としない。schema digestは実descriptorから計算する。 実装が期待するschema identityは、同じdescriptorのcanonical hashから生成できる。 生成したidentityの利用時も、registryのfinalize、登録の存在、package・revision・digestの 完全一致を確認する。生成物検査とruntime descriptorのhash照合によって更新漏れを検出する。 入力値・位置・source閉包・意味構造の検査は、各操作の契約に従って実行する。

<a name="n-6d6f64656c"></a>

<a name="独立した文章モデル"></a>

## <ruby>独立<rt>どくりつ</rt></ruby>した<ruby>文章<rt>ぶんしょう</rt></ruby>モデル

rootはSentenceまたはInlineの型付き参照とし、公開LanguagePackageの既定rootはSentenceとする。 有限arenaの各nodeはSentence、Text、Concat、Ruby、InlineAnno、Code、Emphasis、Strong、 Break、ExternalLink、ForeignInlineのいずれかである。子は順序付きの型付きindexで表す。 Sentence\/ConcatはInline列、Rubyはbaseとreading、InlineAnnoはbaseと順序付きnotesを持つ。 Code\/Textは文字列、Emphasis\/StrongはInline一つ、Breakは子なし、ExternalLinkはURIとlabelを持つ。 ForeignInlineは明示的なForeignClosureGroupのentryを参照し、言語名の閉じたenumを持たない。 複数のforeign値の所有情報を共有する場合は[Foundationの共有所有契約](<24\-shared\-foreign\-owners\.md>)に従う。共有groupの検査だけで文章の出現深さやDoc consumer移行の成立を証明しない。

文章内の外部URL構造はSentenceが所有する。page\/section\/anchorの名前解決、画像asset、 数式等は明示foreign\-inline adapterの契約で扱い、D固有の名前空間をSentence coreへ移さない。 URIの存在確認・network accessやguestの意味解析・実行は文章の構造検査では行わない。 安全な出力と外部参照の解決には後段の独立した検査を要求する。

arenaは型の一致、参照範囲、非循環性、全nodeと全foreign closureの到達性を検査する。 共有部分木を許すが、深さは最初の訪問経路だけでなく最長経路で制限する。 空Sentence、空Text、空Concatを許し、Rubyのbase\/readingとInlineAnnoのbase\/各noteは非空とする。 InlineAnnoのnotesは一つ以上とし、空Textをwrapperで包んでも非空にしない。 Breakは明示した文章内容として非空、ForeignInlineは構造上の内容であり、その表示可能性は adapter準備で検査する。Text内の改行とBreakは別の意味値として保持する。

型・意味モデルはsource位置と独立させる。syntax層は各nodeのSource\/Origin、literalのView、 foreign閉包、field位置を明示して保持する。source\-less値に架空Spanを作らない。 構造proofはsource閉包・binding・表示・hostへの挿入proofを兼ねない。 全検査は共有BudgetでWork\/Nodes\/AllocationUnits\/Depthを計上し、Stoppedから成功へ戻さない。

<a name="n-706f727461626c65"></a>

<a name="sentence値のportable境界"></a>

## Sentence<ruby>値<rt>ち</rt></ruby>のportable<ruby>境界<rt>きょうかい</rt></ruby>

`portable::to_value/from_value`は`nepl3.sentence`のSentenceValueを既存NDF値へ対応させる。 finalize済みregistryと実descriptorの完全identityを照合し、同package・revisionでも異なるdigestを 採用しない。型付きrecord\/variantと順序付きfieldを用い、Rust enumの既定serialize形式へ依存しない。 foreignの深さを文章arenaと別々に判定しない。共有部分木と共有closureを含む最深の所有位置を求め、 呼出元の深さに文章側の経路を加えたBudgetでguest閉包を検査する。各arenaが単独で上限内でも、 合成後の深さが上限を超えれば両codec方向でStoppedを返す。 送信前と受信後にnativeと同じarena検査を行う。schemaだけに適合する循環・不正参照・空Ruby\/Annoを 成功値にしない。各foreign closureはhostが選んだFoundationValueCodecでsource・Origin・environmentを 検査する。ambientなsourceによる欠落閉包の補完は認めず、guestの意味評価は行わない。

SentenceValueの局所node順・共有参照・notes順・文字列・Breakは保持する。foreign bundleの内部は foundationの正準化規則に従うため、任意の送信元NodeRef番号を永久identityとはしない。 owner Originとguest Originを混ぜず、正準CBORの再encodeと意味上の参照対応を検査する。 この値境界は局所文章のSource\/Viewを持つsyntax boundaryとは別であり、印字可能性や安全なHTMLのproofでもない。

<a name="n-6c6f636174696f6e73"></a>

<a name="局所syntaxの位置情報"></a>

## <ruby>局所<rt>きょくしょ</rt></ruby>syntaxの<ruby>位置情報<rt>いちじょうほう</rt></ruby>

`SentenceSyntax`はSentenceValueと独立したlocations、sources、origins、views、sourceMapsを持つ。 locationsはvalue\.nodesと同じ順序・長さで、各nodeへOriginId、任意head、任意coverを対応させる。 OriginIdは必須とし、source\-less生成はSynthetic\/Generated Originを使う。架空Spanで穴を埋めない。 headがある場合はcoverを要求し、同snapshotまたは明示SourceMap経由で包含を検査する。 意味arenaの共有edgeは構文の包含関係と同一ではないため、すべての意味上の子にsource順を強制しない。

SentenceViewはownerの意味node index、元head、ローカルViewBundleを持つ。owner範囲、宣言済みsource、 ownerにcoverがある場合のhead包含、およびhead内の全ViewElementを検査する。複数の原表記を一つの 意味nodeへ正規化する場合はownerを明示的に再対応させ、元のViewRefを別ViewBundleへ流用しない。 coverがない生成nodeでも、明示Source\/Originと対応させた元表記のViewは保持できる。

owner\.coverからView\.headへの包含が成立しない場合、両者の完全なsnapshot identityが異なるときに限り、 View\.headからowner\.coverへの明示SourceMapによる包含も認める。全byte・anchorの全incoming pathを 検査し、一経路だけの一致や未被覆部分は認めない。同snapshotの過大なView\.headはこの例外で許可しない。 これは生成nodeと原表記の来歴対応であり、最小の原文範囲や意味・文字列の同値性を保証しない。 NodeLocation自身のhead\/cover包含、ViewElementのView\.head内包含、source閉包検査は変更しない。 Doc等のhostは、さらに自身のtokenとView\.headとの対応を検査する。

native検査とportable\:\:syntaxの両方向で、重複snapshot revision、未宣言source、古いsnapshotへのSpan、 Origin参照、SourceMap、View閉包、foreignの合成深さを検査する。portable受信はpayload自身のsourceで codecをscopeし、ambient sourceを欠落閉包の代わりにしない。UTF\-8 byte位置を保持し、editorで必要な UTF\-16等への対応は共通Source\/LineIndexに委譲する。 Viewの親子が変換前後のsourceへ分かれる場合も、nativeとportableで同じ明示SourceMapを使う。 FoundationValueCodecの`scoped_with_mappings`はpayloadのsourceとmappingを同時に限定し、 View検査時に全mappingの端点・geometry・source admissionを検査する。通常の`scoped`はmapを 引き継がない。受信側はmappingを検査してからViewをdecodeする。SentenceだけでなくDoc\/Mathの syntax境界もこの契約を使い、ambient mapや同じ本文で包含を補完しない。wire形状の追加ではなく、 既存SourceMapをtyped codecへ正しく渡す共通境界の修正である。 scope内で不変に借用したsource\/mappingに対するadmission・geometry・cycle proofは再利用できる。 各Viewの検査とそのSpanのadmissionは省かず、別scopeへproofを持ち越さない。 SourceAdmissionを可変参照で公開する前にもproofを失効させる。全mappingの検査を 各Viewで反復して文書全体の既存Budgetを浪費しない。 これは関連付けの構造検査であり、原ソースを再parseしたこと、任意の文章値が原文と意味一致すること、 binding、HTMLの安全性を証明しない。それらはlanguage\/adapterの操作で別に検査する。

<a name="n-6c69746572616c"></a>

<a name="sentence-literalの読取りと印字"></a>

## Sentence literalの<ruby>読取<rt>よみと</rt></ruby>りと<ruby>印字<rt>いんじ</rt></ruby>

Sentence coreの`literal::read`は、明示SourceSnapshotのUTF\-8 windowからちょうど一つの 引用符付きliteralを読む。NoMatch\/NeedMore\/Failedでは構文や途中の文章を返さない。 成功時はSentenceSyntaxに元source、denseな位置表、Direct Origin、元綴りのViewを残す。 このnative helperはLanguagePackageやreader\/provider包絡そのものではなく、その解析本体である。 Doc coreへの依存なしで動作する。Doc本文もこのreaderを明示adapter経由で使用し、 Doc coreの重複したSentence parsing APIは撤去した。Doc consumerの意味arenaの所有移行は別の残件である。

`[base/reading]`はRuby、`{base/note/...}`は文章内部のInlineAnnoへ対応する。入れ子を許し、 空のbase\/reading\/note、余分なRuby区切り、不正な括弧対応を型付き失敗とする。literal内の 直接CR\/LFは拒否し、escapeで生成するTextの改行とは区別する。Unicode escapeの結果を再び 括弧や区切りとして解釈しない。失敗位置とopening位置も元snapshotへ束縛する。

`literal::print`はliteralで表現できるSentenceを印字する専用操作である。Text\/Concat\/Ruby\/ InlineAnnoの文章内容を保持するが、元のescape綴りやarena共有を保存する操作ではない。 Break、code、強調、link、foreignはprefix\/adapter出力が必要なためNotLiteralを返し、 Textや改行へ黙って変換しない。独立LanguagePackageの汎用printerはprefixも扱う別の入口とする。 readerとprinterは非再帰で処理し、共有値を展開した実出力にもBudgetを適用する。

reader\/providerのliteral受渡しには`SentenceLiteralPayload`を用意する。SentenceValue、 denseなlocations、Direct Origin列、一つのroot所有Viewを順序付きfieldとして持ち、 SourceContentをliteralごとにwireへ複製しない。`portable::literal`の受信は呼出側が明示した 一つのSourceSnapshotだけでcodecをscopeする。同じID\/revisionでも本文digestが異なるsource、 head外の位置、root以外が所有するView、prefix専用node、foreign、mappingを拒否する。 一般の生成・変換済みsyntaxは閉包付きSentenceSyntaxを使い、このliteral契約へ縮小しない。 受渡しの構造検査は本文の再parseによる意味一致のproofではない。包むTokenのhead\/Viewとの 照合はconsumerのlower操作が行う。新schemaのdigestはdescriptorから算出し、旧Doc payloadの identityやdecoderをSentenceの契約として使い回さない。

開発hostのSentence reader adapterは`nepl3.sentence.reader`の`literal`操作を明示登録する。 ReadRequest\/ReadReply、Unit state、SentenceLiteralPayloadを使用し、ReaderSessionの通常の 停止・再開・reply検査を通す。操作schemaの正確なidentityを検査し、別operationや不正stateを 受理しない。NoMatch、NeedMore、位置付きtyped診断、Stoppedを区別する。これはDoc専用readerの aliasではなく独立したadapterであり、Sentence coreからreader\/engine\/toolsへ逆依存しない。 開発hostのnative adapter検証と、独立processのportable provider比較、LanguagePackage経由の prefix parse\/check\/printは別の受入である。

<a name="n-73757266616365"></a>

<a name="独立languagepackageの表層"></a>

## <ruby>独立<rt>どくりつ</rt></ruby>LanguagePackageの<ruby>表層<rt>ひょうそう</rt></ruby>

`languages/Sentence/syntax.neplg`をGrammar処理系で解析し、`nepl3.syntax.sentence`へcompileする。 意味schemaの`nepl3.sentence`とは別identityである。公開rootはSentence、内部categoryはInline、 列は共通cons\/nilを使う。各formの順序付きfieldは`design/forms.json`と対応させる。

| category | head | 順序付き引数 | arity |
| --- | --- | --- | ---: |
| Sentence | sentence | inlines\: Inline列 | 1 |
| Sentence | 引用符付きliteral | — | 0 |
| Inline | text \/ code | text\: Text | 1 |
| Inline | concat | inlines\: Inline列 | 1 |
| Inline | ruby | base\: Inline\, reading\: Inline | 2 |
| Inline | anno | base\: Inline\, notes\: Inline列 | 2 |
| Inline | em \/ strong | inline\: Inline | 1 |
| Inline | break | — | 0 |
| Inline | link | uri\: Text\, label\: Inline | 2 |

このlinkは外部URIの文章内構造であり、Docのpage\/section\/anchorを暗黙解決しない。 foreign\-inlineは登録されたadapterが別の具体formで導入する。全guestを列挙するformや、 任意の文字列を未検査foreign値へ変換する入口は標準Sentence packageへ追加しない。

`lower::prefix_with_foreign`は、hostが選択した追加Inline formを受け取る。 選択にはformのkind、guestの完全なschema identity、categoryを指定する。 追加formは単一のforeign fieldを持ち、照合後に元の所有者から一括captureしたForeignClosureGroupへ保持する。 標準constructorの上書き、重複した選択、未選択のform、guest identityとcategoryの不一致を拒否する。 `lower::presentation::sentence_with_foreign`は同じ選択を使い、意味nodeとSource\/Origin\/Viewの対応を保持する。 この変換が行う操作は構造の取込みであり、guestの実行は別途認可された操作が担当する。

SentenceのCode modeは空白（space\/tab\/CR\/LF）だけをskipする。独立comment・annotationや directiveをskipに入れない。旧Docのcomment readerをSentenceへ再利用せず、旧`#`入力、 arity不足、未消費の末尾入力を成功文書にしない。既存Docの旧comment撤去は別の移行境界である。

開発hostは具体OperationRefと実装identityへreader関数を登録し、ResolvedProfileの要求に 照合してdispatchする。DocとSentenceが同じsource driverを使っても、SentenceへDoc schemaや Doc readerを要求しない。native呼出しと所有Await経路は同じ検査済みParseTreeを返す。 source driverは引き続き開発hostの入口であり、完全なsuite\/runtimeや独立process providerの完成を 意味しない。

`lower::prefix`は選択済みsurface identityとValidatedSyntaxBundleを受け、現在のBudgetで source・schema・Origin・参照・循環・共有経路の深さを再検査してから標準10formを意味arenaへ投影する。 BuiltinTextのpayload、Inline子、cons\/nil列、constructorのfield数を言語側でも検査し、空Rubyや 空Anno等の意味制約を出力arenaで検査する。先行する構文検査成功だけでは意味適合としない。 非再帰で処理し、停止・不正入力時に部分的な意味値を返さない。

ProjectionはSentenceValueと入力syntax nodeから意味nodeへの局所対応を返す。 list constructorやBuiltinText operandは独立した意味nodeではないため対応値はNoneとする。 Source\/Origin\/Viewを持つ入力構文木は保持し、この対応表を別bundleへ持ち越さない。 source\-less生成も元のSynthetic Originを持つ構文木と対応し、架空Spanを作らない。 この入口はprefix専用でありSentenceLiteralはUnsupportedとする。

`lower::literal::sentence`はrootのSentenceLiteralを専用payload codecへ渡す。選択surface、 leaf shape、実tokenの存在、head\/cover一致を検査し、tokenの正確なsnapshotをownerとしてdecodeする。 payloadとtokenのView\/headがそれぞれ妥当なだけでは受理せず、両者の一致も検査する。 syntax leaf内の意味arenaという所有関係の深さを加え、停止時も呼出元のdepthへ戻す。 旧Doc payloadや原文の再parseによるfallbackは使わない。

`lower::presentation::sentence`は標準literal\/prefixをSentenceSyntaxへ返す共通入口である。 prefixでは投影mapからdenseな位置表を作り、元のSource\/Origin\/SourceMapを保持する。 form headのViewは対応する意味nodeへ、list\/BuiltinText等の補助tokenのViewはrootの表記へ付与し、 全Viewを位置閉包とともに再検査する。これは編集用の元SyntaxBundleを置き換えない。 literalのSentenceSyntaxはreader payload自身の文章内位置とOriginを保持する。包んでいたleafの Originやbindingをpayload内のOriginへ同一化せず、呼出側は元SyntaxBundleも保持する。 source\-less prefixは元Synthetic Originを使用し、架空Spanを作らない。 literalで表現可能なRuby\/Annoの例では、両経路の意味値から得るliteral印字が一致することを検査する。 任意foreign adapterの意味比較、表示の安全性、Doc本文移行の完了をこの一致から推定しない。

`print::prefix`は標準Sentence表層の全constructorを印字する。Sentence rootと明示Inline入口に 対応し、Concat、Ruby、InlineAnnoの境界、notes順、Code、強調、Break、外部URIとlabelを保持する。 TextはBuiltinTextとしてescapeし、文章literalのRuby\/Anno区切りとして再解釈しない。 共有arenaをsourceへ展開した各出現と出力byteにもBudgetを適用し、失敗・停止時は部分文字列を返さない。 foreign\-inlineは対応する表層adapterを別途選択する必要があるため、標準printerでは AdapterRequiredを返す。印字は元の綴り・Source\/Origin・共有indexを再現する操作ではない。 本番packageで標準prefixのparse\/lower\/print一致とText payload復元を検査する。 foreign adapterを含む全意味往復とDoc consumer移行の完了とは区別する。

`print::prepare`はSentenceの構造とforeign closureを検証し、不変借用する。 hostは選択したguest printerから、foreign Inline form全体のsourceを取得し、 `PreparedPrint::resolve`で入力とembedへ対応付ける。`render`はそのsourceを出現順に組み込む。 別入力の結果、同じembedの重複指定、未解決のembedを拒否する。 sourceの文法と意味対応はhost側adapterの検証対象である。 出力時にもWork・Allocation・OutputBytes・Depthを計上し、停止時は部分文字列を返さない。

<a name="n-706c61696e5f74657874"></a>

<a name="sentenceのplain-text生成"></a>

## Sentenceのplain\-text<ruby>生成<rt>せいせい</rt></ruby>

Sentence coreのnative API `text::prepare`は`SentenceValue`の構造とforeign closureを検証し、 入力とregistryの不変借用を保持する。`PreparedText::render`は標準Inlineの順序に従って文章を生成する。 TextとCodeは内容を保持し、強調と外部リンクは本文を、BreakはLFを出力する。 共有nodeは各出現位置で出力する。

`BaseOnly`は注釈のbase、`WithReadings`は`base[reading]`、`WithAllNotes`はさらに `base{note/note}`を出力する。RubyとInlineAnnoの内部にも同じ方針を適用する。 hostはforeign\-inlineの文章を明示的に供給する。`resolve`が供給値を対象入力のembedへ結び付け、 生成時に所属と重複を検査する。出力対象に必要な供給値が欠けた場合は`Unresolved`を返す。 注釈方針によって出力から除外される部分も、準備時の構造・foreign closure検査の対象とする。 供給文章の意味的な正しさとproviderの認証は、hostのadapter契約に従う。

生成はWork・AllocationUnits・OutputBytes・Depthを計上し、失敗・停止時にはErrorを返す。 成功時の返り値は全文のStringである。元sourceの表記と位置情報は入力側で保持する。 このAPIの対象はnativeの意味値である。portable operationの追加とDoc consumerの所有移行は後続作業とする。

<a name="n-68746d6c"></a>

<a name="sentenceのhtml出力adapter"></a>

## SentenceのHTML<ruby>出力<rt>しゅつりょく</rt></ruby>adapter

suiteの`sentence-html` featureは、`adapters::sentence::html::render`を公開する。 SentenceSyntaxを検証し、標準Inlineを共通markupのphrasing fragmentへ変換する純粋な処理である。 このfeatureのdomain依存はSentence coreであり、HTMLの構造検査とserializeはmarkupが担当する。 Text、Code、強調、Break、外部リンク、Ruby、InlineAnnoの内容・順序を保持する。 RubyとAnnoには既存の`nepl-ruby`・`nepl-anno`等の表示classを用いる。 hostは対応するstylesheetの出自と配置を管理する。外部URIと出力文字はmarkupの安全性規則に従う。

返り値は元SentenceSyntaxの不変借用、検査済みの出力構造、各出力要素と意味nodeの対応を保持する。 共有nodeの各出現は個別の出力要素を持つ。元のSource・Origin・Viewは入力を参照して取得する。 出力構造を変更するconsumerは、変更後の構造と対応情報を再検証する。 Work・Nodes・AllocationUnits・Depthを生成と検証に計上し、失敗・停止時は部分fragmentを返さない。 serializeのOutputBytesはmarkup側で計上する。

標準入口はForeignInlineへ`ForeignAdapterRequired`を返す。`render_with_foreign`では、 入力closureの検証後に、hostが選択したcallbackを各出現について実行する。 callbackのowned markupは呼出元のDepthを保持してphrasingとして検査し、arenaを移動して結合する。 `ForeignPlacement`はembedと出力arena内の連続範囲を対応付ける。guest内部の意味nodeとの対応は、 その言語の出力metadataを保持するhostが、この配置情報に従って更新する。 callback後にBudgetの停止を確認し、停止後に返された成功値と後続callbackを公開・実行しない。 Mathの文章注釈では、内側Mathのnode対応と注釈対応をSentenceの配置へ移し、 外側MathMLからHTMLへの変換でも同じ対応を更新する。

<a name="n-646f635f726561646572"></a>

<a name="doc本文readerへの接続"></a>

## Doc<ruby>本文<rt>ほんぶん</rt></ruby>readerへの<ruby>接続<rt>せつぞく</rt></ruby>

この節はconsumer所有移行中のadapter契約である。Sentenceの恒久的な意味モデルと、現在のDoc consumer arenaへの変換を区別する。移行完了後も必要なforeign adapterと、旧所有を除去するまでの互換変換を同じ完成条件にしない。

Docの標準文法は独立Sentenceのliteral readerを使用し、suiteの`doc-sentence` featureで公開する `nepl3_suite::adapters::sentence::document`を通して現在のDoc consumerへ渡す。 adapterは`no_std + alloc`で動作し、DocとSentenceの公開型・検査を接続する。 suiteの既定featureはこのdomain依存を有効にしない。開発hostはfeatureを明示して利用する。 Doc\/Sentence core間の直接依存と、coreからtoolsへの依存を禁止する。 adapterはSentenceSyntaxを検査し、全標準Inlineを同じarena index・順序でDoc値へ変換した後、 Doc側でも構造を再検査する。CodeはDocのInlineCode、外部linkはLinkTarget\:\:Externalへ対応し、 foreign CodeやDoc固有のpage参照へ読み替えない。foreign\-inlineは個別adapterを要求する。 Source\/Origin\/SourceMapとViewを保持し、元SentenceSyntaxのdense位置・View ownerも呼出側で保持できる。 入力の共有・source\-less Syntheticを展開や架空Spanで置き換えず、失敗・停止時は部分Doc値を返さない。

Doc catalogは独立Sentenceのdescriptorと標準literal readerを登録する。readerは SentenceLiteralPayloadを返し、Doc固有のreader descriptor・SentencePayload codecを持たない。 suiteのliteral adapterがSentenceLiteralPayloadまたはSentenceSyntaxを一度だけ検査し、 既存の型付きSentence→Doc bridgeへ渡す。旧Doc payloadへのfallbackは行わない。 codec経路は、検査済みの所有SentenceSyntaxと検査に用いた不変registryを私有の型付きownerへ束縛する。 このownerには公開constructorと可変syntax参照を与えず、bridgeは同じ値を再検査せずに変換できる。 未検査SentenceSyntaxを受け取る公開bridgeは従来どおり検査する。Doc構造と外tokenの検査は省かず、 codec・変換・後段検査は同じ累積BudgetとSourceAdmissionを使用し、停止後に成果物を返さない。

Doc consumerの正規化では、一つのsequence内の隣接Text列をまとめ、完成した文字列だけを新しいnodeとして保持する。 結合途中のprefix全体を繰り返し複製せず、文字列のコピー・確保はその列の合計byte数に比例させる。 共有される入力nodeは変更せず、元の順序と既存のOrigin合成順を保持する。source\-less部分を既知のsourceで補完しない。 参照の走査・出力確保・Origin合成は引き続き累積Budgetへ計上する。これは局所的な隣接Text結合の条件であり、 入れ子や共有を含む文書全体の正規化を線形時間と保証するものでも、consumer所有移行の完了を意味するものでもない。

このownerは操作内の検査結果の受渡しであり、portable proofやDoc consumer所有移行の完了証拠ではない。 Doc lowerの公開decoder境界はcallbackの停止を優先して伝播し、返却された局所Doc構造を 参照の再配置前に検査する。その後で外tokenのhead\/Viewと全位置の所属を検査する。 生成されたrootのcoverと原Viewは、前述の全経路SourceMap条件で保持できる。 文書の内部identityは生成manifestへ記録し、Markdownの出自は実際のsourceと参照入力に対応する。 この段階はreader\/payloadの所有移行であり、Doc consumerの意味arenaにあるSentence\/Inline表現は残る。 DocのInlineMath・DisplayMathをMath表示へ接続するadapterも引き続き必要である。 Mathの文章注釈は独立Sentenceを直接使用する。Doc本文の二重所有は後続のconsumer所有移行で整理する。

<a name="n-666f726569676e"></a>

<a name="hostが選択するforeign-inlineの構築"></a>

## hostが<ruby>選択<rt>せんたく</rt></ruby>するforeign Inlineの<ruby>構築<rt>こうちく</rt></ruby>

hostは、Grammar compilerの型付き`ForeignForm`を通じて、既存categoryへ単一のforeign fieldを持つformを追加できる。 宣言にはkind、category、spelling、field、foreign alias、guest category、生成理由を指定する。 compilerは追加宣言を含めてsurface descriptorを生成し、そのidentityでreaderと全formを構築する。 既存kindとの衝突、重複kind、空の宣言情報、未定義category、同一読取りcontextでのspelling衝突を拒否する。 元の文法sourceと由来を保持し、hostが生成した宣言には独立したSynthetic Originを付ける。 foreign aliasの実schemaとcategoryはParseProfileの解決時に検査する。

Doc hostは、この経路でSentenceのInlineへ`math Expr`を追加し、MathのExprを明示的に選択する。 標準Sentence単独のpackageは従来の宣言集合を保持する。選択後のsurfaceは追加宣言に対応するschema identityを持つ。 Math注釈のprinterはSentenceを構造としてlowerし、foreign MathをMath printerへ渡す。 共有guestの呼出しには最深の出現位置を使用し、呼出元のDepthと累積Budgetを保持する。 HTMLのforeign adapterはMathの出力操作を選択し、内側Sentenceの本文・読みの対応を最終HTMLへ保持する。 再解析後の構造比較、NDF受信後の表示、source対応、旧Doc注釈headの拒否をconsumer試験で検査する。 Mathの数値評価は注釈のSentence内部へ進入せず、注釈の表示と印字は選択したhost操作が担当する。

hostはSentenceのInlineへDocのInlineを選択するformも登録できる。 printerは選択したLanguagePackageのformから綴りを取得し、Doc InlineをDoc printerへ渡す。 Doc内のMathと、その注釈のSentenceへ再入する場合も、各言語のschema・categoryと累積Budgetを検査する。

Doc Inlineの局所HTML表示は`prepare_local_inline`と`render_inline`を使用する。 準備段階でfragment内のlabel、前方参照、重複出現を検査し、link・asset・foreign guestの依存要求を返す。 外部の依存要求を持たない入力から、専用の準備済み型とPhrasingのHTMLを生成する。 HTML要素は元のDoc nodeへの対応を保持する。外側の文書のnamespaceと複数fragment間の参照はcomposition側で解決する。 Sentenceのforeign HTML adapterは、hostが選択したDoc Inlineを専用の準備・表示入口へ渡す。 `prepare_inline_with_foreign`は、未解決要求がforeign guestだけであることを検査する。 `render_inline_with_foreign`は、準備済み文書が所有するclosureをhostへ渡し、返されたHTMLのPhrasing適合と合成後の構造を検査する。 hostはDoc内のInlineMathを選択したMath出力へ接続し、その注釈のSentenceとDocへの再入を処理する。 各guestの表示位置は出現ごとに記録し、共有embedの複数出現も独立したHTML要素へ対応付ける。 合成結果はMathとDocの由来を型で区別し、保持した構文とHTML要素の対応を各合成段階で更新する。 未解決labelはsource付き診断、link・assetは依存要求として返す。未選択のguest操作は明示的に拒否する。 複数fragmentのHTML IDが衝突した場合は合成を拒否し、停止時は部分HTMLを成功結果として返さない。 guestのHTMLもDoc出力の深さ上限256と、呼出元の累積Budgetに従う。 文書namespaceの名前解決は`labels::namespace`が担当する。hostは構造検査済みのArticle・Sentence・Inline断片を、表示出現順のmember列として明示する。 検査結果は各定義・参照のmember、Doc node、元source位置を保持し、選択した全memberの定義を収集してから前方参照を解決する。 同じ定義を持つmemberの複数出現とmember間の名前重複を拒否する。未選択の断片やforeign closure内部の定義は自動導入しない。 名前索引は宣言順のidentityを保持し、比較・確保を共通Budgetへ計上する。名前解決のproofは選択した不変のmember列へ束縛する。 native APIの`resolve_interleaved`は、定義と参照を別々の型付きindex列で指定し、member間の順序を交互に構成できる。 各indexは元member列の出現IDと、そのmemberが収集した局所site indexを持つ。同じ文書を複数回選択しても出現IDを統合しない。 各member内部のconstructor順を保ち、定義・参照それぞれについて全siteを一度ずつ列挙しなければならない。 欠落・重複・範囲外・局所順の反転を型付きorder errorとして拒否してから名前解決する。order errorを未解決labelの診断へ読み替えない。 このproofは選択したmemberと明示site順での名前解決だけを保証する。hostが実際のArticle走査順を供給した証明でも、既存rendererがその順に出力する保証でもない。 従来の`resolve`はmemberごとの連続順を維持する。両入口は同じ名前解決処理を使い、空memberの走査とschedule検査も累積Budgetへ計上する。 Doc HTMLの`namespace::prepare`はこのproofから全memberを取得し、共通codec・registry・source admissionで再検査する。 `namespace::render`はmember順にHTMLを合成し、全体のID・参照・構造と深さ上限を検査してから出力する。 出力はmemberごとの文書digestとDoc node・HTML要素の対応を保持する。合成用wrapperへ架空のDoc ownerを割り当てない。 表示言語の選択によって参照先が出力されない場合は最終検査で拒否する。この入口ではlink・asset・foreign操作の未解決要求を返す。 hostがSentence内の位置へ挿入する場合は、`namespace::render_part`から構造検査済みの`PendingPart`を取得する。 この値はmember・文書digest・Doc node対応を保持し、文書内参照を未確定のまま合成先へ渡す。単体のserialize用proofは付与しない。 共通HTMLの`check_part`は構造・属性・挿入slotを検査し、Sentence rendererは全guestの合成後にID重複と参照解決を確定する。 参照先のguestが未配置の場合はSentenceの表示全体を拒否し、部分HTMLを完成結果として返さない。 InlineMathを含むnamespaceは`namespace::prepare_with_foreign`で準備する。対応するguest操作はInlineMathに限定し、他のforeign要素とlink・assetは未解決の依存要求として返す。 `namespace::render_part_with_foreign`は指定memberのguest操作を実行し、Doc参照を保持した`PendingPart`とguestの表示位置を返す。 guest出力は局所的に有効なPhrasingを要求し、Doc参照の最終解決は合成先が担当する。停止時はpending出力も返さない。 Sentence annotation hostは、構造検査済みSentenceの子順からDoc断片の表示出現を事前収集する。 Rubyの読み・Annoの注釈と共有nodeの反復を含め、出現順に文書namespaceへ登録する。 同じembedのDoc意味モデルは不変の共有値として保持し、描画結果と由来は表示出現ごとに記録する。 型付きSentenceを直接渡す`render_syntax`も、文書namespaceの準備前にSentenceの構造・source境界を検査し、同じ累積Budgetと深さ上限を適用する。 各Doc断片内のInlineMathも既存のMath出力へ接続し、Sentence全体の合成後にHTML参照を検査する。 未解決参照は元sourceの診断を返し、member間の重複定義は両方の文書・nodeと位置付き診断を保持したエラーを返す。 namespaceの`Error::diagnostic`は重複定義の主位置と既存定義の関連位置を各memberのsourceへ照合し、既存の`DuplicateLabel`診断を生成する。 各siteのnode・名前・位置をmemberの定義または参照と照合し、両sourceを使用して診断全体を検査する。同じsnapshotを共有する断片も、この所属検査の対象とする。 Doc断片の選択とlowerは、suiteの`adapters::sentence::document_guests::collect`が担当する。 この公開APIはSentenceの局所syntaxを検査し、指定したschema identityとInline categoryに一致するclosureを解析する。 返却値は一意なDoc文書の所有列と、各表示出現から文書を参照する`DocumentId`の列を持つ。 同じembedは最深の出現位置で一度解析し、表示順と共有を保持する。処理は`no_std + alloc`で成立し、I\/O・HTML・原子的参照カウントを要求しない。 他言語のguestは各adapterが担当し、名前解決と表示は後段で実行する。開発hostはこの結果を既存のnamespaceとHTML処理へ接続する。

複数の独立Sentenceを配置するnative consumerは、`document_guests::collect_placed`へ所有列と表示配置列を明示する。 配置は所有indexと呼出元からの相対深さを持ち、範囲外indexと未配置ownerを拒否する。各ownerを最深の配置で一度検査・解析し、 Doc断片の所有はowner別に保持する。同じownerの繰り返し配置はDoc値を共有するが、異なるownerを本文や局所embed番号の一致だけで統合しない。 返却値は元の所有列・配置列の不変借用と、配置index・owner・局所embed・Doc所有indexの対応を保持する。 出現列は配置順とSentence内の順序に従い、参照先のDoc断片を明示的なnamespace memberとして使用できる。 この操作は名前解決や描画を行わず、相対深さが実際のArticleの配置から求められた証明でもない。 未選択guestを含むSentence構造とsource閉包を検査し、同じ累積Budgetで停止した場合は選択結果を返さない。 Article全体の走査・namespace統合と旧Doc Sentence所有の除去は、引き続きconsumer移行の残件である。

foreign Inlineの構造検査はguestを内容ありとして扱い、guest固有の表示内容を確定しない。 `CheckedShape::content_with_foreign`はhostが明示したguestの内容判定を用いてRubyのbase\/readingとAnnoのbase\/noteの非空条件を再検査する。 各embedのcallbackは最深の共有出現の深さで一度だけ呼び、同じ累積Budgetを使う。callback後の停止を成功や別errorで上書きしない。 Doc guestの内容判定は元のDoc shapeの`annotation_content`から取得する。この入口はSentence\/Inline rootだけに値を返し、Article等の空判定には使わない。空labelのanchorは単独なら有効だが、Ruby\/Annoの必要な内容にはならない。 native annotation hostはDocの意味モデルと明示選択したMath Exprの内容契約を使用する。HTMLや文字列長からguestの内容を推定しない。 これは内容条件の検査であり、sourceの許可・名前解決・guest実行・完成HTMLの検査を代替しない。

このnamespaceは一つのSentence表示に属する。外側のArticle全体とのnamespace共有、およびDocの旧Sentence所有の撤去はconsumer所有移行の残件である。 複数のSentenceを一つの文書へ配置するrendererは、suiteの`html::render_part_with_foreign`で`PendingSentence`を取得できる。 この値は局所syntax・guest closure・Phrasing構造の検査と要素の由来を保持し、HTML IDの重複とfragment参照の解決を合成後の文書検査へ委譲する。 各Sentenceのsource所有者を保持して要素位置を再対応させ、完成したHTML全体を検証した後にserializeする。 `PendingSentence`はserializerへ直接渡せない。単独表示の`render_with_foreign`は従来どおり文全体のIDと参照を検査する。 `html::paragraph::compose`はSentence断片を表示順に消費し、段落のHTMLを構成する。 各出現の入力Sentence、要素範囲、nodeの由来、foreign要素の位置を保持し、移動先の要素番号へ対応させる。 段落全体の参照・重複ID・構造・深さを共通Budgetで検査し、失敗時は部分出力を返さない。 この段落合成はHTMLの操作であり、Docの文書モデルとArticle全体の名前解決への接続は引き続きconsumer移行で扱う。

開発hostの`doc::annotations::paragraph::render`は、明示した独立Sentence所有列と配置列を、一つの段落namespaceへ接続する。 Doc Inlineの出現を`collect_placed`で収集し、全memberの名前解決とguest内容検査を行ってから、各Sentenceをpending HTMLとして描画する。 `render_part_with_foreign_codec`は再帰guestと同じSourceAdmissionを使用し、現在の深さで入力を検査する。 段落自身の階層を加えた深さを収集・内容検査・各Sentenceの描画へ渡し、完成した段落全体を検査した後だけ返す。 Doc fragmentの前方参照はSentenceをまたいで解決できるが、同じ定義を持つ配置の反復は重複定義となる。 Doc内のInlineMathとそのSentence注釈は既存の選択済みhostへ接続し、Math注釈の局所定義を段落namespaceへ導入しない。 foreignのdispatchとDoc\/Mathの由来記録は単独annotation表示と共通にし、各配置のmetadataを完成した段落のHTML要素へ対応させる。 返却値は元のSentenceを不変借用し、Doc断片の共有所有と配置ごとの表示位置を保持する。失敗や停止時に部分段落を返さない。 この入口は明示配置の段落表示であり、Article走査・CLIの既存Article経路・旧Doc所有の撤去を完了したものではない。

<a name="n-616e6e6f746174696f6e"></a>

<a name="注釈と移行完了条件"></a>

## <ruby>注釈<rt>ちゅうしゃく</rt></ruby>と<ruby>移行完了条件<rt>いこうかんりょうじょうけん</rt></ruby>

以下は移行後にも維持する注釈契約と、旧経路を除去する際の受入条件である。実施順・各段階の残件はT07と実装状態で管理する。旧コメントの認識だけを先に削除する変更はmainへ統合しない。

`annotate Sentence target`はarity 2の通常formであり、各host categoryへ固定shapeで登録する。 runtimeのgeneric categoryや独立Comment、commented、特殊trivia導入子を追加しない。 targetのbinding\/import\/export\/visibilityとdomain意味を保存し、構文とannotation relationは残す。 文章中のInlineAnnoとは別kindである。生成注釈も同じ正式syntaxを返す。

各categoryで無注釈とのbinding\/domain比較、source\/Origin保持、通常Doc HTMLから著者注釈の除外、 literal\/prefixの意味一致、native\/portable往復、Unicode境界、資源停止と不正入力を検査する。 旧\#消費、TriviaKind\:\:Comment、wire\/sidecar、各profile、再生成元、公式sourceを同じ移行で除去する。 旧schema拒否と旧\#負例を残し、互換decoder・flag・恒久移行runtimeは残さない。

<a name="n-6d6174685f636f6d706f736974696f6e"></a>

<a name="articlepagesetへのmath表示の合成"></a>

### Article・PageSetへのMath<ruby>表示<rt>ひょうじ</rt></ruby>の<ruby>合成<rt>ごうせい</rt></ruby>

nativeのDoc HTML hostは、Code表示とInlineMath・DisplayMathを同じ文書走査へ接続できる。 InlineMathの返却markupはPhrasing、DisplayMathはBlockとして検査する。DisplayMathの前後ではparagraph内のsentence runを分ける。 Code\-only・local\-onlyの既存入口は従来の選択範囲を維持し、追加のguestを暗黙に許可しない。 Mathの表示は式の評価を要求せず、17章のrenderer選択と明示的なfallback診断を維持する。

MathのSentence注釈に局所Doc namespaceがある場合、注釈内部で定義・参照の解決とID一意性を確定した後、Mathの表示出現ごとにそのHTML IDと局所fragment参照を束縛する。 この束縛は外側Docの名前解決へ定義を導入しない。同じMathの独立した表示出現は区別できるが、同一注釈内の重複定義や、未選択の外側Doc anchorへの参照を救済しない。 異なるMath出現や親Docと衝突するIDをそのまま採用せず、親Docの名前を偽装できない出現別prefixを使う。 これは検査済みの局所namespaceの束縛であり、任意の重複HTMLを修復する処理ではない。

hostの合成結果は、元Doc・Math・Sentenceと、対応先の実際の出力arenaを併せて保持する。 Math node・注釈nodeの要素番号は各出現の移動先へ写像し、HTML文字列だけに元arenaの番号を添えて対応保持済みとはしない。 PageSetは全ページの準備・描画・出力検査が成功するまで部分成果物を返さない。 入力検査と再帰guest表示は同じ操作のsource admissionと累積Budgetを共有し、同じsnapshotの再検査でSourceBytesを二重計上しない。 検査済み構文を使う内部の走査も、その公開入口で現在のdepth・source・closureを検査する。別操作や浅い位置のproofによって現在の入力検査を迂回する公開入口は設けない。

<a name="n-73656e74656e63655f736c6f74"></a>

<a name="docの明示的なsentence-closure-slot"></a>

## Docの<ruby>明示的<rt>めいじてき</rt></ruby>なSentence closure slot

段階移行の入口として、Doc\/SentenceとDoc\/Flowは `Sentence` に続く独立Sentence\/Sentenceを受け取る。 Doc側は `SentenceGuest { syntax }` と `EmbedKind::Sentence` で閉じたforeign syntaxを保持し、Sentenceのinline arenaをDocへ複製しない。 ArticleやSectionのtitle、Variant、table cell、caption、alt、OptionalSentenceも既存のSentenceRefを通してこのslotを保持する。 従来のDoc\-owned sentence literal\/inline経路はこの段階では残る。Article全体のSentence所有への移行完了を意味しない。

shape検査はslotのrole、到達性、未使用entryを検査する。構造検査はSentence category、閉じたsource\/environment、最深の所有位置とcaller depthを検査する。 `GuestLanguage::Sentence` はprint bindingの識別であり、汎用Doc\/GuestやCodeの別入口を追加しない。 fragmentはclosure groupと元のsource・Origin・Viewを保持し、選択したentryのindexだけを再配置する。 Docのshape証明はforeign Sentenceのannotation contentを判定せず、そのrootに対するcontent照会はNoneを返す。

printは明示的なSentence bindingとdocument\/guest digestに対応するguestの印字結果を要求する。 plainTextも対応するguest textが未選択ならUnresolvedEmbedを返し、空文字で代替しない。独立Sentence側のannotation policy適用は、textを供給するhostの責務である。 local\-only HTML準備はSentenceをForeign requirementとして返す。PageSet・CLIの自動描画、Doc namespaceへのguest定義の導入、旧Doc Inline型の除去はこのslotの追加だけでは成立しない。 Sentence意味変換の自動選択と独立paragraph合成hostへの接続は未完了である。下記の明示的なguest描画はArticleの挿入境界を提供する。

<a name="n-61727469636c655f646973706c6179"></a>

<a name="明示的なarticle-guest描画"></a>

### <ruby>明示的<rt>めいじてき</rt></ruby>なArticle guest<ruby>描画<rt>びょうが</rt></ruby>

`prepare_display` はArticleの構造・source・closureを検査し、未解決要求が対応済みのForeign（Sentence、InlineMath、DisplayMath、Code）だけの場合に限り描画入力を返す。CircuitFigure・独立Guest・未解決AssetはNeedsResolutionとして準備段階で返す。既存の安全な外部URL処理を保ち、未解決要求を空の結果で置き換えない。 `render_display` はtitle、本文、table cell等の表示順に元のclosureを選択済みadapterへ渡す。同じguestの複数表示は別のplacementを持ち、所有元と挿入範囲を保持する。 SentenceGuestの出力は挿入位置でphrasing構造を検査する。Sentence間の参照はArticle全体で解決し、全体のID重複・未解決参照・不正なネストを検査してから結果を返す。Math等の既存guest境界は引き続き個別の閉じたnamespace検査を行う。 adapter失敗・停止・深さ超過・不正なmarkupでは部分Articleを返さない。入力検査とcallbackは呼出し側のBudgetを継続し、source admissionは選択したhostの同一操作で保持する。 この拡張は既存display／PageSetの明示選択されたadapterの挿入境界を共用する。Sentence意味変換の自動選択、CLIへの接続、旧Doc inline所有の撤去までを完了としない。

<a name="n-6f6363757272656e636573"></a>

<a name="構造上の出現列"></a>

### <ruby>構造上<rt>こうぞうじょう</rt></ruby>の<ruby>出現列<rt>しゅつげんれつ</rt></ruby>

`ValidatedDocShape::structural_occurrences` は不変の検査済みDoc arenaからconstructor順の出現列を返す。同じnodeの共有参照を展開し、各出現はnode index、親出現index、親のchild index、rootを1とする相対depthを保持する。 これは表示順やnamespaceへの採用を保証しない。全Parallel variantとImage altも含み、foreign closureへは進入しない。hostは表示選択・alt投影と意味上の名前解決を区別し、構造列を描画callback列へ無条件に対応付けない。 走査は現在の呼出元depthを加算して再検査し、Work・確保量を累積Budgetへ計上する。停止・overflow・確保失敗では部分列を返さない。出現数はDAGのnode数に対して指数的になり得るため、この入口を既存の線形shape検査や飽和出現数による重複label検査の代用にしない。 元sourceの再入場、foreign closureの深さ、HTML wrapperの挿入depth、表示対象の選択、Article全体のguest namespaceは別の検査を要する。これだけでCLI接続やArticle走査の完成とはしない。

hostの`annotations::article::collect_structural`は、この構造列から独立Sentenceの所有元を選択する。結果は元のArticleへの不変borrow、局所EmbedRefごとに一度だけlowerしたSentence、全構造出現列とSentence出現から所有元への対応を保持する。 各所有元は最深の構造出現位置で、選択済みschema\/category、同一codecとsource admission、累積Budgetを使ってlowerする。異なるArticleの同じEmbedRefを同一所有元としない。後続の不正なSentenceや停止が見つかれば部分所有元表を返さない。 既存の注釈rendererと同じclosure loweringを共用し、Sentence内のDoc／Math guestは閉じた値として保持する。全variant・altを含む構造収集であり、それらの定義を文書namespaceへ自動導入する操作ではない。表示出現・名前解決・HTML位置への対応付けは、引き続き別のhost操作で確認する。

<a name="n-61727469636c655f6e616d657370616365"></a>

<a name="article本体と明示採用したsentence断片の名前解決"></a>

### Article<ruby>本体<rt>ほんたい</rt></ruby>と<ruby>明示採用<rt>めいじさいよう</rt></ruby>したSentence<ruby>断片<rt>だんぺん</rt></ruby>の<ruby>名前解決<rt>なまえかいけつ</rt></ruby>

`annotations::article::namespace::resolve`は、元の構造収集結果と、hostが明示的に採用するSentence出現index列を受け取る。indexは構造順に狭義単調増加で、範囲外・重複・逆順を拒否する。表示言語・alt・出力対象の選択をこの操作が推定することはない。 Article本体の局所siteは元のconstructor順で一度ずつ扱い、採用したSentence内の直接のDoc Inline断片のsiteをその出現位置へ挿入する。共有された参照nodeの再出現は局所siteの重複列挙にせず、同じguest所有元の別出現は別memberとして扱う。Math注釈の局所namespaceをArticleへ導入しない。 所有元を現在のschema選択・source admission・呼出元depthで再検査し、最深の採用出現でDoc断片を収集する。既存の`resolve_interleaved`で全siteと局所順の完全性を検査してから前方参照を解決する。未採用の断片の定義は参照先にならない。 結果は元のSelectionへの不変borrow、guest文書の共有所有、memberの由来、定義・参照nodeと宣言IDへの対応を保持する。名前エラーは元sourceの診断と、Article／Sentence出現／局所Doc guest出現を区別する情報を保持する。停止・不正入力では部分結果を返さない。 この結果は明示した採用集合での名前解決の記録であり、可視性、HTML安全性、portable proof、別のBudgetでの描画許可ではない。描画操作は所有元と名前解決を再検査し、実際に出力したArticle全体のID・参照を確定する。この操作単体はCLIへの接続や旧構造の撤去を行わない。CLIの採用範囲は後述する。

<a name="n-6e616d6573706163655f646973706c6179"></a>

<a name="明示的なnamespaceのarticle描画"></a>

### <ruby>明示的<rt>めいじてき</rt></ruby>なnamespaceのArticle<ruby>描画<rt>びょうが</rt></ruby>

`namespace::display::prepare`は検査済みの採用member集合から表示用の準備を作る。各memberのsource・closureと要件を再検査し、Article displayと同じSentence／Code／InlineMath／DisplayMath、および共通URI規則を通るExternal linkだけを選択する。Page／Relative link、未解決Asset、その他のguestはNeedsResolutionのままとする。既存の`prepare_with_foreign`のInlineMath限定契約は拡張しない。 `render_part`は個別memberの構造を検査したpending partを返す。`render_article`は明示されたArticle memberだけをrootとして描画し、adapterが挿入した出力全体のID重複・未解決参照・nestingを最後に検査する。member集合を別のDivへ連結せず、guestを二重に出力しない。External linkのnode対応は各member内で保持する。 この入口はSentence意味変換、採用集合と実際のcallbackの対応、可視性の決定を自動化しない。hostは正しい所有元を選択し、実際の挿入位置のBudgetでpending partを描画する。名前を持たないmemberの挿入省略や入替えを、最終ID検査だけで検知できるとはしない。CLI接続とnested MathのKaTeX優先出力は別のhost層が担当し、portable受渡しの完成をこの入口から推定しない。

<a name="n-74797065645f686f7374"></a>

<a name="型付きsentenceを持つarticle-host"></a>

### <ruby>型付<rt>かたつ</rt></ruby>きSentenceを<ruby>持<rt>も</rt></ruby>つArticle host

`annotations::article::namespace::resolve_with`は、採用memberの再検査・名前解決後、局所のCheckedNamespaceと所有元情報が存続するscope内で一つのhost操作を呼ぶ。操作の結果・エラーから一時proofのborrowを持ち出せない。既存のresolveの結果と検査は維持し、callbackのエラーよりも同じBudgetの停止を優先する。 `annotations::article::render::render`は全構造Sentence出現を明示採用して実際のSentence意味値からArticleを描画する。Rows／Columnsでは全出現を挿入し、Singleでは共通の表示inventoryから得た元の構造出現indexを使って挿入対象を選ぶ。namespaceへの採用集合を表示によって縮めず、hidden memberの定義・参照・重複・Sourceも検査する。画像・Asset等は共通準備がcallback前に拒否する。出現数・所有元EmbedRef・Doc memberの由来と消費数を照合し、同じ所有元の可視／非表示出現を混同しない。Math／Code等の外側guestは別の明示adapterへ渡し、Sentenceの出現として消費しない。 Sentence内のDoc断片とMathの記録は、まずSentence arenaへ、次に最終Article arenaへ対応付ける。Sentence・Docの元node、保持した所有元、入れ子Mathの位置を維持し、Article全体の名前・nesting検査後だけ結果を返す。各挿入は実際のcallbackのBudgetと祖先depthを使用する。 `namespace::display::prepare_located`はmember局所の準備失敗にMemberIdを付ける。集合全体の入場検査で生じた失敗はmemberを推定せずNoneとする。Article hostはこの区別を保持し、guest内のURI等のnodeエラーを外側Articleのnodeとして扱わない。既存prepareの返却型・選択規則は変えない。 このhostはHTML出力基盤への段階接続であり、単体CLIと入れ子MathのKaTeX優先変換への接続は後述のexport層が担当する。PageSetへのnative組込みは後述のexport層が担当する。単体Article CLIのasset\-free表示選択は後述のexport層で接続する。画像altのnative text投影は後述の限定入口で接続する。画像altを含むexport全体の表示選択、portable成果物の認証、旧Doc Inlineの撤去は未完了である。

<a name="n-636c69"></a>

<a name="doc-cliのsentence選択"></a>

### Doc CLIのSentence<ruby>選択<rt>せんたく</rt></ruby>

Doc hostの既定profileは独立SentenceのInlineへ`math`（Math Expr）と`document`（Doc Inline）を明示選択する。これはstandalone Sentence標準へのconstructor追加ではなく、host profileの拡張である。選択されたsurface／parse\-profile identityは両formを含む実内容から求め、以前のidentityへ固定しない。独立Sentence内では`math frac 1 2`、Doc Inlineを選ぶ場合は`document math Math frac 1 2`のように、各所有言語の構文を区別する。 単体Doc HTML exportはSentence guestを含むasset\-free Articleに型付きArticle hostを使用し、既定Rows表示で本文・Doc断片の参照と外部URLを検査する。Sentenceの所有元・構造出現・描画記録をLocalDocumentへ保持し、原文Docと共に返す。Sentenceを含まない既存SVG経路は維持するが、SentenceとAssetの組合せを黙って別経路へ落とさず未対応として返す。 CLIでの単体Article対応からPageSetのSentence対応を推定しない。PageSetは後述の専用namespace経路で接続する。単体Article CLIのasset\-free Single選択と画像alt選択は区別し、後者のexport接続は未完了であり、native text投影だけを後述する。既存Math注釈内の入れ子数式の生成はPageSetでも共通の数式収集経路を使う。

`doc-html export --display display.json [--limits limits.json] [--css inline|external] [--math-renderer katex-preferred|mathml-only] input.nepld new-output` はasset\-freeの<ruby>単体<rt>たんたい</rt></ruby>Articleへ<ruby>表示選択<rt>ひょうじせんたく</rt></ruby>を<ruby>明示<rt>めいじ</rt></ruby>する。\-\-displayと\-\-limitsは<ruby>先頭<rt>せんとう</rt></ruby>で<ruby>順序<rt>じゅんじょ</rt></ruby>を<ruby>入<rt>い</rt></ruby>れ<ruby>替<rt>か</rt></ruby>えられる。その<ruby>後<rt>あと</rt></ruby>に<ruby>既存<rt>きそん</rt></ruby>のCSS・Math renderer<ruby>指定<rt>してい</rt></ruby>と<ruby>入力<rt>にゅうりょく</rt></ruby>・<ruby>新規出力先<rt>しんきしゅつりょくさき</rt></ruby>を<ruby>置<rt>お</rt></ruby>く。<ruby>指定<rt>してい</rt></ruby>を<ruby>省略<rt>しょうりゃく</rt></ruby>した<ruby>既存<rt>きそん</rt></ruby>entryとSVG entryはRowsを<ruby>維持<rt>いじ</rt></ruby>し、localeから<ruby>選択<rt>せんたく</rt></ruby>を<ruby>推測<rt>すいそく</rt></ruby>しない。`generate_with_display` と `write_with_display` は<ruby>同<rt>おな</rt></ruby>じnative `DisplayOptions` と `ExecutionLimits` を<ruby>受<rt>う</rt></ruby>け<ruby>取<rt>と</rt></ruby>る。display\.jsonは `{"mode":"rows"}`、`{"mode":"columns"}`、または `{"mode":"single","language":"ja","fallbacks":["en"]}` とする。<ruby>設定<rt>せってい</rt></ruby>は65536byte<ruby>以内<rt>いない</rt></ruby>のstrict JSONとし、<ruby>未知<rt>みち</rt></ruby>field・mode・null・<ruby>重複<rt>ちょうふく</rt></ruby>keyを<ruby>拒否<rt>きょひ</rt></ruby>する。escape<ruby>表記<rt>ひょうき</rt></ruby>だけが<ruby>異<rt>こと</rt></ruby>なる<ruby>同名<rt>どうめい</rt></ruby>keyも<ruby>重複<rt>ちょうふく</rt></ruby>である。Singleのlanguageは<ruby>必須<rt>ひっす</rt></ruby>、fallbacksは<ruby>省略時<rt>しょうりゃくじ</rt></ruby>だけ<ruby>空<rt>から</rt></ruby>リストとし、<ruby>言語名<rt>げんごめい</rt></ruby>と<ruby>候補<rt>こうほ</rt></ruby>の<ruby>意味検査<rt>いみけんさ</rt></ruby>は<ruby>共通描画器<rt>きょうつうびょうがき</rt></ruby>で<ruby>行<rt>おこな</rt></ruby>う。<ruby>全構造出現<rt>ぜんこうぞうしゅつげん</rt></ruby>の<ruby>所有元<rt>しょゆうもと</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>してから<ruby>表示<rt>ひょうじ</rt></ruby>を<ruby>選<rt>えら</rt></ruby>び、<ruby>非表示<rt>ひひょうじ</rt></ruby>targetへの<ruby>参照<rt>さんしょう</rt></ruby>を<ruby>黙<rt>だま</rt></ruby>って<ruby>成功<rt>せいこう</rt></ruby>させない。<ruby>設定<rt>せってい</rt></ruby>・parse・lower・<ruby>生成<rt>せいせい</rt></ruby>の<ruby>失敗<rt>しっぱい</rt></ruby>では<ruby>出力先<rt>しゅつりょくさき</rt></ruby>を<ruby>作<rt>つく</rt></ruby>らない。<ruby>最終<rt>さいしゅう</rt></ruby>manifestは<ruby>完了<rt>かんりょう</rt></ruby>markerであり、<ruby>書込<rt>かきこ</rt></ruby>みI\/Oの<ruby>途中失敗<rt>とちゅうしっぱい</rt></ruby>をtransactionや<ruby>永続復旧<rt>えいぞくふっきゅう</rt></ruby>で<ruby>保護<rt>ほご</rt></ruby>する<ruby>契約<rt>けいやく</rt></ruby>ではない。

Rowsの<ruby>生成<rt>せいせい</rt></ruby>scopeと<ruby>既存<rt>きそん</rt></ruby>manifestの<ruby>値<rt>あたい</rt></ruby>は<ruby>維持<rt>いじ</rt></ruby>する。Columns・Singleではoptionsに<ruby>要求<rt>ようきゅう</rt></ruby>modeを<ruby>記録<rt>きろく</rt></ruby>し、Singleだけlanguageと<ruby>順序<rt>じゅんじょ</rt></ruby>を<ruby>保<rt>たも</rt></ruby>ったfallbacksを<ruby>追加<rt>ついか</rt></ruby>する。`display_scope` のcontractは `nepl3.local-doc-export.display-scope/1` とする。<ruby>基底<rt>きてい</rt></ruby>はsource SHA\-256・parse\-profile SHA\-256の<ruby>小文字<rt>こもじ</rt></ruby>hexとMath renderer<ruby>名<rt>めい</rt></ruby>をcolonで<ruby>連結<rt>れんけつ</rt></ruby>した<ruby>文字列<rt>もじれつ</rt></ruby>のSHA\-256である。identityはcontractのASCII・NUL・<ruby>基底<rt>きてい</rt></ruby>32byte・mode<ruby>符号<rt>ふごう</rt></ruby>を<ruby>連結<rt>れんけつ</rt></ruby>してSHA\-256とする。Columnsは1、Singleは2を1byteで<ruby>表<rt>あらわ</rt></ruby>す。Singleでは<ruby>続<rt>つづ</rt></ruby>けてlanguageのUTF\-8 byte<ruby>長<rt>ちょう</rt></ruby>と<ruby>原文<rt>げんぶん</rt></ruby>、fallback<ruby>数<rt>すう</rt></ruby>、<ruby>各<rt>かく</rt></ruby>fallbackのbyte<ruby>長<rt>ちょう</rt></ruby>と<ruby>原文<rt>げんぶん</rt></ruby>を<ruby>順<rt>じゅん</rt></ruby>に<ruby>加<rt>くわ</rt></ruby>える。<ruby>長<rt>なが</rt></ruby>さと<ruby>数<rt>かず</rt></ruby>はu64 little\-endianとし、<ruby>同<rt>おな</rt></ruby>じ<ruby>表示結果<rt>ひょうじけっか</rt></ruby>でも<ruby>大文字小文字<rt>おおもじこもじ</rt></ruby>・<ruby>順序<rt>じゅんじょ</rt></ruby>・<ruby>未使用候補<rt>みしようこうほ</rt></ruby>を<ruby>正規化<rt>せいきか</rt></ruby>しない。このidentityのhexをcolonで<ruby>旧<rt>きゅう</rt></ruby>scope<ruby>文字列<rt>もじれつ</rt></ruby>へ<ruby>付加<rt>ふか</rt></ruby>し、<ruby>生成側<rt>せいせいがわ</rt></ruby>KaTeXにも<ruby>同<rt>おな</rt></ruby>じ<ruby>選択<rt>せんたく</rt></ruby>を<ruby>束縛<rt>そくばく</rt></ruby>する。<ruby>追加<rt>ついか</rt></ruby>hash・buffer・suffix<ruby>構築<rt>こうちく</rt></ruby>は<ruby>共通<rt>きょうつう</rt></ruby>output Budgetで<ruby>計測<rt>けいそく</rt></ruby>する。この<ruby>局所<rt>きょくしょ</rt></ruby>scopeはCSSの<ruby>梱包方式<rt>こんぽうほうしき</rt></ruby>や<ruby>全工程<rt>ぜんこうてい</rt></ruby>の<ruby>認証<rt>にんしょう</rt></ruby>ではなく、portable provenanceや<ruby>実行権限<rt>じっこうけんげん</rt></ruby>を<ruby>与<rt>あた</rt></ruby>えない。<ruby>既存<rt>きそん</rt></ruby>のmanifest<ruby>直列化<rt>ちょくれつか</rt></ruby>・file hash<ruby>生成<rt>せいせい</rt></ruby>の<ruby>計測除外<rt>けいそくじょがい</rt></ruby>は<ruby>変<rt>か</rt></ruby>えない。

<a name="n-70616765735f7265736f6c7574696f6e"></a>

<a name="pagesetへ明示採用したnamespaceのnativeリンク解決"></a>

### PageSetへ<ruby>明示採用<rt>めいじさいよう</rt></ruby>したnamespaceのnativeリンク<ruby>解決<rt>かいけつ</rt></ruby>

`doc-core::pages::namespace::resolve` は、PageSetの登録と、ページ順に明示された検査済みDoc namespaceを接続するnative操作である。各namespaceのmember 0は、そのPageSetが所有する同一Articleへのborrowでなければならず、等値な複製・順序違い・個数違いを拒否する。追加memberの採用とSentence所有元の保持はhostの責務であり、この操作はforeign closureやMath注釈を暗黙に走査・採用しない。

各memberの構造・source・closureを現在のregistryと共通source admissionで再検査し、Page／Relative linkのsource位置をpage・member・nodeの組で保持する。Relativeは出力routeでなく、所有ページの論理sourceを基準にする。参照先fragmentは対象ページの明示採用namespace内で照合し、他ページの同名ラベルは衝突と扱わない。登録fileへのfragment、未登録page、sourceのrootを越えるrelative pathを拒否する。External URI・Asset・Foreign要件は解決済みとせず、そのmemberの型付き要件として保持する。

結果は元PageSetとnamespaceへの不変borrowを持つ操作内の検査記録である。portable proof、既存PageSet identity、最終HTMLのID到達性、Sentence可視性、描画やCLI接続の完了を表さない。後続hostは採用所有元・表示選択・URI policy・出力全体のanchorを検査し、同じ累積Budgetを継続する。停止と失敗時は部分結果を返さない。通常のouter\-only PageSet resolverと既存portable契約は変更しない。

<a name="n-70616765735f6578706f7274"></a>

<a name="型付きsentenceを含むnative-pageset-html-export"></a>

### <ruby>型付<rt>かたつ</rt></ruby>きSentenceを<ruby>含<rt>ふく</rt></ruby>むnative PageSet HTML export

`doc-html::namespace::pages` は明示採用namespaceのリンク計画からmemberごとの表示準備を構築する。描画器はArticle本体を所有して構築し、hostへ渡すcallbackはforeign出現のみに限定する。Code callbackの実際の返却markupがDOM Idを持つ場合は拒否する。member 0以外のCodeは所有元付きCode adapterが未実装のため未対応とする。Docの固定HTML depthと全体markup検査を省かず、callback後の停止を通常エラーより優先し、limits／呼出元depthの変更を拒否する。

実際に出力されたBetweenArtifacts hrefはsource route・登録target・最終target fragmentを検査する。targetのDOM Id表はページごとに一度構築して再利用する。登録を迂回するArtifact hrefは拒否する。名前解決で採用しただけの未出力anchorは到達可能なtargetにならない。返値はnative断片と描画器由来のorigin／foreign placementであり、既存PageSet identityやportable proofを付与しない。

local PageSet exportはSentenceを含む場合に、全ページの型付き所有元収集・全構造Sentence出現の明示採用・元constructor順の名前解決を同じ出力操作で行う。各ページのSessionが所有元・member・callbackの消費順を照合し、最終page arenaへSentence originと入れ子Math記録を写像する。Sentenceを含まないページも同一集合で扱い、通常のlegacy\-only出力経路は維持する。native hostの明示的なDisplayOptionsではRows／Columns／Singleを選択できる。retained native jobも同じ選択を固定して利用する。CLIのpage manifestも明示選択を受け取り、省略時とretained wrapperの既定表示はRowsである。画像altの選択、Asset組合せ、旧Inline撤去の完了を意味しない。

typed host identityのRows／Columnsは、`nepl3.local-doc-pages.sentence-input/1\0` と完全なPageSetのcanonical NDF\/1 CBORから得たdigestに表示modeの1 byte（Rows\=0、Columns\=1）を続け、その33 byteをSHA\-256に通す既存契約を維持する。Singleは別のbyte列をSHA\-256に通す。その順序は `nepl3.local-doc-pages.sentence-single/1\0`、同じ32 byteの入力digest、Singleのtag 2（1 byte）、要求言語のUTF\-8 byte長（little\-endian u64）と言語byte列、fallback件数（little\-endian u64）、順序付きの各fallbackのUTF\-8 byte長（little\-endian u64）とbyte列である。要求optionsそのものを識別し、表示結果の等価性による正規化はしない。連結時のWork・確保量とhash対象byte数を同じ出力Budgetへ課す。これは固定の全出現採用を行うnative hostの入力同定であり、portable検査済みnamespaceの認証ではない。manifestのrendererを`nepl3-doc-html sentence-pages/1`とし、legacy identityと混同しない。

出力段階のCode照合はSentence／Docと同じcodecのsource admissionを使う。前段のCode構文解析に要した独立の入場・処理費は返却せず累積Budgetへ残す。停止後のretained再開は準備済み文書を保全し、描画だけを再試行する。名前エラーには所有ページのindexと登録IDを付す。全ページが生成・検証・serializeされるまで部分出力を公開しない。

<a name="n-737667416c74"></a>

### native SVGのSentence<ruby>代替文<rt>だいたいぶん</rt></ruby><ruby>投影<rt>とうえい</rt></ruby>

開発hostのannotations\:\:alt\:\:prepareはImageとInlineImageのalt slotにある独立Sentenceだけを選択し、実際のforeign closureをlowerしてBaseOnlyのplain textを作る。全variantのassetを準備する既存SVG契約に従い、表示optionsでalt所有元を省略しない。RubyのreadingとAnnoのnotesは出力せず、選択されたforeign textが未解決ならエラーを返し、空文字やHTMLで代替しない。元のDocと各SentenceSyntaxのSource・Origin所有を保ち、文書入場・identity・lower・検査・投影を同じBudgetとSourceAdmissionで行う。共有guestは一度だけ投影するが、呼出元depthに最深の構造所有depthを加えて検査する。

native prepare\_svg\_display\_with\_textは明示的なResolvedInlineTextを受け取り、Doc plain textでdocument digest・guest digest・EmbedRef・重複を検査する。assetがない場合の非空resolutionも拒否する。供給textはデータであり、guestの意味を認証するportable証拠ではない。altはHTML callbackやnamespaceへ採用せず、captionの描画とは分離する。既存SVG入口はresolutionなしのwrapperとして保持する。このnative接続はSentenceとassetを組み合わせた単体CLI・PageSet export、画像関連のnamespace表示選択、portable proof、プロセス終了後の復元を完成させない。

<a name="n-7669736962696c697479"></a>

<a name="html挿入対象sentence出現の構造inventory"></a>

### HTML<ruby>挿入対象<rt>そうにゅうたいしょう</rt></ruby>Sentence<ruby>出現<rt>しゅつげん</rt></ruby>の<ruby>構造<rt>こうぞう</rt></ruby>inventory

`doc-html::visibility::sentence_occurrences` は検査済みDoc shapeと表示optionsから、HTML guest callbackの対象となるSentence出現を列挙する。各要素は元の全構造出現列でのindex、node、EmbedRef、constructor depthを保持し、共有nodeの再出現を同じ所有元へ畳み込まない。Parallelの選択は実描画準備と共通のselection tableを使い、要求言語・明示fallback順・ASCII大小文字を無視した比較・不正または重複言語・MissingVariantの既存規則を変えない。hidden部分を含むarena全体のoption検査を保つ。

ImageとInlineImageのaltはHTML guest挿入でなくtext projectionの対象なので列挙しない。Imageのcaptionは挿入対象として残す。この一覧はforeign closureへ進入せず、Source\/Originの入場、Sentence所有元のlower、namespaceへの採用、Asset解決、最終HTMLの安全性を証明しない。本文がCSSで見えているかの判定でもない。停止時は部分一覧を返さず、元の構造展開と選択・出力配列のWork／確保量を同じBudgetへ計上し、現在の呼出元depthを維持して検査する。構造展開がDAGの共有を出現ごとに展開する費用も省略しない。

このinventoryだけでSingle対応を認証しない。native Article Sessionは同一の不変Selectionに対して再検査したshapeから表示対象を得て、元の構造index・node・EmbedRef・depthを保持済み所有元へ照合する。元のSentence出現IDを描画記録へ残し、挿入前にはそれ以前の非表示memberだけを飛ばす。可視Sentenceの直接Doc memberはそのcallback内で全て消費したことを直ちに検査する。末尾の非表示memberが未出力で残ることは許すが、実際のHTML参照がそのanchorを要求すれば最終検査で拒否する。Code等を含む実callback位置とoriginの写像は表示順を維持する。PageSet hostもページごとにこのSessionを作り、同じ明示的RenderOptionsを使う。全出現の名前解決と、実際のページ間hrefが指す出力anchorの検査を分離する。非表示の定義が名前解決に存在しても、実際のhrefの到達先として出力されなければ拒否する。Rows／Columns用identity契約は維持する。

`pages::generate_with_display_options`はnative host用の明示入口であり、DocのRenderOptionsとMath rendererをまとめたDisplayOptions、別々のparse／lower予算と既存の出力Budgetを受け取る。既存入口のRows既定値は変えない。manifestのoptionsには実際のmodeを記し、Singleでは要求言語と順序付きfallbackも保持する。CLIのpage manifestはstrict設定から同じ表示選択へ変換する。portable namespace receiptは追加しない。retained jobへの明示入口は後述する。

legacy\-only PageSetの意味identityは入力同定のまま保持するが、非Rowsのexecution identityは表示optionsを必ず束縛する。従来の `nepl3.local-doc-pages.execution/2` のdigestをbaseとし、`nepl3.local-doc-pages.display-execution/1\0`、32 byteのbase、mode tag（Columns\=1、Single\=2）を続ける。Singleはさらに上記と同じ言語長・言語・fallback件数・各長とbyte列を続け、SHA\-256に通す。全非Rows入力に共通のboundedな構築を使い、phase identityにもこのexecution identityを渡す。Rowsの従来execution identityと既定CLI／retained出力は変更しない。manifestのoutput\_budget\.contractは実際のexecution契約を示す。

retained native PageSetでは `PreparedPages::into_render_with_display` が同じDisplayOptionsを受け取り、文書・Code証拠とともに表示選択を操作内で所有する。pause後のresumeは追加allowanceと出力上限だけを受け取り、言語・fallback順・rendererを変更できない。停止した描画は同じ選択でatomicにやり直し、完了済みページを再parseせず、試行ごとの使用量を同じjob ledgerへ加算する。意味上の選択失敗や非表示targetは終端失敗とし、資源停止へ読み替えない。非Rowsのexecution identityはこの固定選択を束縛し、retained preparationとretry jobのidentityにも渡す。従来の `into_render` はRows既定のwrapperとして保つ。登録file／aliasのretained入力・portable証拠への拡張はこの入口に含めない。

PageSet CLIの通常出力と対話的再開は、version 1 manifestの省略可能な `display` を受け取る。省略時はRows。明示する場合は `{"mode":"rows"}`、`{"mode":"columns"}`、または `{"mode":"single","language":"ja","fallbacks":["en"]}` とする。modeは小文字の固定値であり、Singleのlanguageは必須、fallbacksだけ省略時に空リストとする。未知field・mode、他modeのSingle用field、重複JSON key、nullは拒否する。これは開発hostのstrict入力契約であり、coreのportable codecやRust enum表現を公開wire契約にするものではない。選択の意味検査は共通描画器に委ね、失敗時は出力directoryを作らない。対話的再開ではmanifestを再読せず、準備開始前に所有した選択を全再試行へ引き継ぐ。変更できるのは既存の明示allowanceだけである。単体Article CLIは前述の\-\-display設定で接続し、このPageSet manifestとは入力形式を区別する。canonical専用設定の選択追加は別範囲である。
