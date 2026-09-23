# 23. 構造化文章と対象付き注釈への是正

この章は[#158の統合設計](../decisions/multilanguage-hca.md)を具体化した、Sentenceと対象付きannotationの規範契約である。契約の採用と実装完了は別であり、実装・正式受入の状態は [実装状態の正本](../../implementation-status.json) を参照する。

入出力・意味・失敗条件・source閉包・資源停止はこの章で定義する。`literal::read`等のRust名は現在の実装入口を指し、他言語の実装にRust ABIや内部module構成を要求しない。[Doc runtimeの記録](../progress/doc-runtime.md) は現在の接続範囲と履歴を説明し、[T07/T19/T21のタスク定義](../../design/tasks.json) が移行順と成果物を所有する。

## 所有者と実装順

NEPL3sentenceは独自LanguagePackage、Sentence root、sentence literalと前置構築、
単独parse/check/printを持つ文章言語である。NEPL3aはSentenceと対象syntaxの付与関係、
NEPL3dは文書構造を所有する。Doc本文をA経由にしない。
Sentence coreはfoundationだけへ依存し、Doc/Math/A/engine/hostへ依存しない。
reader adapter、language package compile、各hostとの橋渡しは外側へ置く。

この節の見出しは既存参照のため維持する。実装順の正本はT07であり、本文は所有境界を定める。
Doc本文とannotationはそれぞれSentenceを使用し、Aの完成をD本文や公開の前提にしない。
途中のモデル検査成功を独立言語の完成としない。schema digestは実descriptorから計算する。

## 独立した文章モデル

rootはSentenceまたはInlineの型付き参照とし、公開LanguagePackageの既定rootはSentenceとする。
有限arenaの各nodeはSentence、Text、Concat、Ruby、InlineAnno、Code、Emphasis、Strong、
Break、ExternalLink、ForeignInlineのいずれかである。子は順序付きの型付きindexで表す。
Sentence/ConcatはInline列、Rubyはbaseとreading、InlineAnnoはbaseと順序付きnotesを持つ。
Code/Textは文字列、Emphasis/StrongはInline一つ、Breakは子なし、ExternalLinkはURIとlabelを持つ。
ForeignInlineは明示的なForeignClosureを参照し、言語名の閉じたenumを持たない。

文章内の外部URL構造はSentenceが所有する。page/section/anchorの名前解決、画像asset、
数式等は明示foreign-inline adapterの契約で扱い、D固有の名前空間をSentence coreへ移さない。
URIの存在確認・network accessやguestの意味解析・実行は文章の構造検査では行わない。
安全な出力と外部参照の解決には後段の独立した検査を要求する。

arenaは型の一致、参照範囲、非循環性、全nodeと全foreign closureの到達性を検査する。
共有部分木を許すが、深さは最初の訪問経路だけでなく最長経路で制限する。
空Sentence、空Text、空Concatを許し、Rubyのbase/readingとInlineAnnoのbase/各noteは非空とする。
InlineAnnoのnotesは一つ以上とし、空Textをwrapperで包んでも非空にしない。
Breakは明示した文章内容として非空、ForeignInlineは構造上の内容であり、その表示可能性は
adapter準備で検査する。Text内の改行とBreakは別の意味値として保持する。

型・意味モデルはsource位置と独立させる。syntax層は各nodeのSource/Origin、literalのView、
foreign閉包、field位置を明示して保持する。source-less値に架空Spanを作らない。
構造proofはsource閉包・binding・表示・hostへの挿入proofを兼ねない。
全検査は共有BudgetでWork/Nodes/AllocationUnits/Depthを計上し、Stoppedから成功へ戻さない。

## Sentence値のportable境界

`portable::to_value/from_value`は`nepl3.sentence`のSentenceValueを既存NDF値へ対応させる。
finalize済みregistryと実descriptorの完全identityを照合し、同package・revisionでも異なるdigestを
採用しない。型付きrecord/variantと順序付きfieldを用い、Rust enumの既定serialize形式へ依存しない。
foreignの深さを文章arenaと別々に判定しない。共有部分木と共有closureを含む最深の所有位置を求め、
呼出元の深さに文章側の経路を加えたBudgetでguest閉包を検査する。各arenaが単独で上限内でも、
合成後の深さが上限を超えれば両codec方向でStoppedを返す。
送信前と受信後にnativeと同じarena検査を行う。schemaだけに適合する循環・不正参照・空Ruby/Annoを
成功値にしない。各foreign closureはhostが選んだFoundationValueCodecでsource・Origin・environmentを
検査する。ambientなsourceによる欠落閉包の補完は認めず、guestの意味評価は行わない。

SentenceValueの局所node順・共有参照・notes順・文字列・Breakは保持する。foreign bundleの内部は
foundationの正準化規則に従うため、任意の送信元NodeRef番号を永久identityとはしない。
owner Originとguest Originを混ぜず、正準CBORの再encodeと意味上の参照対応を検査する。
この値境界は局所文章のSource/Viewを持つsyntax boundaryとは別であり、印字可能性や安全なHTMLのproofでもない。

## 局所syntaxの位置情報

`SentenceSyntax`はSentenceValueと独立したlocations、sources、origins、views、sourceMapsを持つ。
locationsはvalue.nodesと同じ順序・長さで、各nodeへOriginId、任意head、任意coverを対応させる。
OriginIdは必須とし、source-less生成はSynthetic/Generated Originを使う。架空Spanで穴を埋めない。
headがある場合はcoverを要求し、同snapshotまたは明示SourceMap経由で包含を検査する。
意味arenaの共有edgeは構文の包含関係と同一ではないため、すべての意味上の子にsource順を強制しない。

SentenceViewはownerの意味node index、元head、ローカルViewBundleを持つ。owner範囲、宣言済みsource、
ownerにcoverがある場合のhead包含、およびhead内の全ViewElementを検査する。複数の原表記を一つの
意味nodeへ正規化する場合はownerを明示的に再対応させ、元のViewRefを別ViewBundleへ流用しない。
coverがない生成nodeでも、明示Source/Originと対応させた元表記のViewは保持できる。

native検査とportable::syntaxの両方向で、重複snapshot revision、未宣言source、古いsnapshotへのSpan、
Origin参照、SourceMap、View閉包、foreignの合成深さを検査する。portable受信はpayload自身のsourceで
codecをscopeし、ambient sourceを欠落閉包の代わりにしない。UTF-8 byte位置を保持し、editorで必要な
UTF-16等への対応は共通Source/LineIndexに委譲する。
Viewの親子が変換前後のsourceへ分かれる場合も、nativeとportableで同じ明示SourceMapを使う。
FoundationValueCodecの`scoped_with_mappings`はpayloadのsourceとmappingを同時に限定し、
View検査時に全mappingの端点・geometry・source admissionを検査する。通常の`scoped`はmapを
引き継がない。受信側はmappingを検査してからViewをdecodeする。SentenceだけでなくDoc/Mathの
syntax境界もこの契約を使い、ambient mapや同じ本文で包含を補完しない。wire形状の追加ではなく、
既存SourceMapをtyped codecへ正しく渡す共通境界の修正である。
scope内で不変に借用したsource/mappingに対するadmission・geometry・cycle proofは再利用できる。
各Viewの検査とそのSpanのadmissionは省かず、別scopeへproofを持ち越さない。
SourceAdmissionを可変参照で公開する前にもproofを失効させる。全mappingの検査を
各Viewで反復して文書全体の既存Budgetを浪費しない。
これは関連付けの構造検査であり、原ソースを再parseしたこと、任意の文章値が原文と意味一致すること、
binding、HTMLの安全性を証明しない。それらはlanguage/adapterの操作で別に検査する。

## Sentence literalの読取りと印字

Sentence coreの`literal::read`は、明示SourceSnapshotのUTF-8 windowからちょうど一つの
引用符付きliteralを読む。NoMatch/NeedMore/Failedでは構文や途中の文章を返さない。
成功時はSentenceSyntaxに元source、denseな位置表、Direct Origin、元綴りのViewを残す。
このnative helperはLanguagePackageやreader/provider包絡そのものではなく、その解析本体である。
Doc coreへの依存なしで動作する。Doc本文もこのreaderを明示adapter経由で使用し、
Doc coreの重複したSentence parsing APIは撤去した。Docの意味schema・payloadの所有移行は別の残件である。

`[base/reading]`はRuby、`{base/note/...}`は文章内部のInlineAnnoへ対応する。入れ子を許し、
空のbase/reading/note、余分なRuby区切り、不正な括弧対応を型付き失敗とする。literal内の
直接CR/LFは拒否し、escapeで生成するTextの改行とは区別する。Unicode escapeの結果を再び
括弧や区切りとして解釈しない。失敗位置とopening位置も元snapshotへ束縛する。

`literal::print`はliteralで表現できるSentenceを印字する専用操作である。Text/Concat/Ruby/
InlineAnnoの文章内容を保持するが、元のescape綴りやarena共有を保存する操作ではない。
Break、code、強調、link、foreignはprefix/adapter出力が必要なためNotLiteralを返し、
Textや改行へ黙って変換しない。独立LanguagePackageの汎用printerはprefixも扱う別の入口とする。
readerとprinterは非再帰で処理し、共有値を展開した実出力にもBudgetを適用する。

reader/providerのliteral受渡しには`SentenceLiteralPayload`を用意する。SentenceValue、
denseなlocations、Direct Origin列、一つのroot所有Viewを順序付きfieldとして持ち、
SourceContentをliteralごとにwireへ複製しない。`portable::literal`の受信は呼出側が明示した
一つのSourceSnapshotだけでcodecをscopeする。同じID/revisionでも本文digestが異なるsource、
head外の位置、root以外が所有するView、prefix専用node、foreign、mappingを拒否する。
一般の生成・変換済みsyntaxは閉包付きSentenceSyntaxを使い、このliteral契約へ縮小しない。
受渡しの構造検査は本文の再parseによる意味一致のproofではない。包むTokenのhead/Viewとの
照合はconsumerのlower操作が行う。新schemaのdigestはdescriptorから算出し、旧Doc payloadの
identityやdecoderをSentenceの契約として使い回さない。

開発hostのSentence reader adapterは`nepl3.sentence.reader`の`literal`操作を明示登録する。
ReadRequest/ReadReply、Unit state、SentenceLiteralPayloadを使用し、ReaderSessionの通常の
停止・再開・reply検査を通す。操作schemaの正確なidentityを検査し、別operationや不正stateを
受理しない。NoMatch、NeedMore、位置付きtyped診断、Stoppedを区別する。これはDoc専用readerの
aliasではなく独立したadapterであり、Sentence coreからreader/engine/toolsへ逆依存しない。
開発hostのnative adapter検証と、独立processのportable provider比較、LanguagePackage経由の
prefix parse/check/printは別の受入である。

## 独立LanguagePackageの表層

`languages/Sentence/syntax.neplg`をGrammar処理系で解析し、`nepl3.syntax.sentence`へcompileする。
意味schemaの`nepl3.sentence`とは別identityである。公開rootはSentence、内部categoryはInline、
列は共通cons/nilを使う。各formの順序付きfieldは`design/forms.json`と対応させる。

| category | head | 順序付き引数 | arity |
| --- | --- | --- | ---: |
| Sentence | sentence | inlines: Inline列 | 1 |
| Sentence | 引用符付きliteral | — | 0 |
| Inline | text / code | text: Text | 1 |
| Inline | concat | inlines: Inline列 | 1 |
| Inline | ruby | base: Inline, reading: Inline | 2 |
| Inline | anno | base: Inline, notes: Inline列 | 2 |
| Inline | em / strong | inline: Inline | 1 |
| Inline | break | — | 0 |
| Inline | link | uri: Text, label: Inline | 2 |

このlinkは外部URIの文章内構造であり、Docのpage/section/anchorを暗黙解決しない。
foreign-inlineは登録されたadapterが別の具体formで導入する。全guestを列挙するformや、
任意の文字列を未検査foreign値へ変換する入口は標準Sentence packageへ追加しない。

`lower::prefix_with_foreign`は、hostが選択した追加Inline formを受け取る。
選択にはformのkind、guestの完全なschema identity、categoryを指定する。
追加formは単一のforeign fieldを持ち、照合後にForeignClosureを保持する。
標準constructorの上書き、重複した選択、未選択のform、guest identityとcategoryの不一致を拒否する。
`lower::presentation::sentence_with_foreign`は同じ選択を使い、意味nodeとSource/Origin/Viewの対応を保持する。
この変換が行う操作は構造の取込みであり、guestの実行は別途認可された操作が担当する。

SentenceのCode modeは空白（space/tab/CR/LF）だけをskipする。独立comment・annotationや
directiveをskipに入れない。旧Docのcomment readerをSentenceへ再利用せず、旧`#`入力、
arity不足、未消費の末尾入力を成功文書にしない。既存Docの旧comment撤去は別の移行境界である。

開発hostは具体OperationRefと実装identityへreader関数を登録し、ResolvedProfileの要求に
照合してdispatchする。DocとSentenceが同じsource driverを使っても、SentenceへDoc schemaや
Doc readerを要求しない。native呼出しと所有Await経路は同じ検査済みParseTreeを返す。
source driverは引き続き開発hostの入口であり、完全なsuite/runtimeや独立process providerの完成を
意味しない。

`lower::prefix`は選択済みsurface identityとValidatedSyntaxBundleを受け、現在のBudgetで
source・schema・Origin・参照・循環・共有経路の深さを再検査してから標準10formを意味arenaへ投影する。
BuiltinTextのpayload、Inline子、cons/nil列、constructorのfield数を言語側でも検査し、空Rubyや
空Anno等の意味制約を出力arenaで検査する。先行する構文検査成功だけでは意味適合としない。
非再帰で処理し、停止・不正入力時に部分的な意味値を返さない。

ProjectionはSentenceValueと入力syntax nodeから意味nodeへの局所対応を返す。
list constructorやBuiltinText operandは独立した意味nodeではないため対応値はNoneとする。
Source/Origin/Viewを持つ入力構文木は保持し、この対応表を別bundleへ持ち越さない。
source-less生成も元のSynthetic Originを持つ構文木と対応し、架空Spanを作らない。
この入口はprefix専用でありSentenceLiteralはUnsupportedとする。

`lower::literal::sentence`はrootのSentenceLiteralを専用payload codecへ渡す。選択surface、
leaf shape、実tokenの存在、head/cover一致を検査し、tokenの正確なsnapshotをownerとしてdecodeする。
payloadとtokenのView/headがそれぞれ妥当なだけでは受理せず、両者の一致も検査する。
syntax leaf内の意味arenaという所有関係の深さを加え、停止時も呼出元のdepthへ戻す。
旧Doc payloadや原文の再parseによるfallbackは使わない。

`lower::presentation::sentence`は標準literal/prefixをSentenceSyntaxへ返す共通入口である。
prefixでは投影mapからdenseな位置表を作り、元のSource/Origin/SourceMapを保持する。
form headのViewは対応する意味nodeへ、list/BuiltinText等の補助tokenのViewはrootの表記へ付与し、
全Viewを位置閉包とともに再検査する。これは編集用の元SyntaxBundleを置き換えない。
literalのSentenceSyntaxはreader payload自身の文章内位置とOriginを保持する。包んでいたleafの
Originやbindingをpayload内のOriginへ同一化せず、呼出側は元SyntaxBundleも保持する。
source-less prefixは元Synthetic Originを使用し、架空Spanを作らない。
literalで表現可能なRuby/Annoの例では、両経路の意味値から得るliteral印字が一致することを検査する。
任意foreign adapterの意味比較、表示の安全性、Doc本文移行の完了をこの一致から推定しない。

`print::prefix`は標準Sentence表層の全constructorを印字する。Sentence rootと明示Inline入口に
対応し、Concat、Ruby、InlineAnnoの境界、notes順、Code、強調、Break、外部URIとlabelを保持する。
TextはBuiltinTextとしてescapeし、文章literalのRuby/Anno区切りとして再解釈しない。
共有arenaをsourceへ展開した各出現と出力byteにもBudgetを適用し、失敗・停止時は部分文字列を返さない。
foreign-inlineは対応する表層adapterを別途選択する必要があるため、標準printerでは
AdapterRequiredを返す。印字は元の綴り・Source/Origin・共有indexを再現する操作ではない。
本番packageで標準prefixのparse/lower/print一致とText payload復元を検査する。
foreign adapterを含む全意味往復とDoc consumer移行の完了とは区別する。

## Sentenceのplain-text生成

Sentence coreのnative API `text::prepare`は`SentenceValue`の構造とforeign closureを検証し、
入力とregistryの不変借用を保持する。`PreparedText::render`は標準Inlineの順序に従って文章を生成する。
TextとCodeは内容を保持し、強調と外部リンクは本文を、BreakはLFを出力する。
共有nodeは各出現位置で出力する。

`BaseOnly`は注釈のbase、`WithReadings`は`base[reading]`、`WithAllNotes`はさらに
`base{note/note}`を出力する。RubyとInlineAnnoの内部にも同じ方針を適用する。
hostはforeign-inlineの文章を明示的に供給する。`resolve`が供給値を対象入力のembedへ結び付け、
生成時に所属と重複を検査する。出力対象に必要な供給値が欠けた場合は`Unresolved`を返す。
注釈方針によって出力から除外される部分も、準備時の構造・foreign closure検査の対象とする。
供給文章の意味的な正しさとproviderの認証は、hostのadapter契約に従う。

生成はWork・AllocationUnits・OutputBytes・Depthを計上し、失敗・停止時にはErrorを返す。
成功時の返り値は全文のStringである。元sourceの表記と位置情報は入力側で保持する。
このAPIの対象はnativeの意味値である。portable operationの追加とDoc consumerの所有移行は後続作業とする。

## SentenceのHTML出力adapter

suiteの`sentence-html` featureは、`adapters::sentence::html::render`を公開する。
SentenceSyntaxを検証し、標準Inlineを共通markupのphrasing fragmentへ変換する純粋な処理である。
このfeatureのdomain依存はSentence coreであり、HTMLの構造検査とserializeはmarkupが担当する。
Text、Code、強調、Break、外部リンク、Ruby、InlineAnnoの内容・順序を保持する。
RubyとAnnoには既存の`nepl-ruby`・`nepl-anno`等の表示classを用いる。
hostは対応するstylesheetの出自と配置を管理する。外部URIと出力文字はmarkupの安全性規則に従う。

返り値は元SentenceSyntaxの不変借用、検査済みの出力構造、各出力要素と意味nodeの対応を保持する。
共有nodeの各出現は個別の出力要素を持つ。元のSource・Origin・Viewは入力を参照して取得する。
出力構造を変更するconsumerは、変更後の構造と対応情報を再検証する。
Work・Nodes・AllocationUnits・Depthを生成と検証に計上し、失敗・停止時は部分fragmentを返さない。
serializeのOutputBytesはmarkup側で計上する。

現行の入口は標準Inlineを対象とし、ForeignInlineには`ForeignAdapterRequired`を返す。
guestの意味処理・出力は明示的なrole adapterで接続する。Mathの文章注釈の所有移行では、
この出力境界、foreign処理、既存のsyntax identityとsource対応を一貫して接続する。

## Doc本文readerへの接続

この節はconsumer所有移行中のadapter契約である。Sentenceの恒久的な意味モデルと、現在のDoc payloadへの変換を区別する。移行完了後も必要なforeign adapterと、旧所有を除去するまでの互換変換を同じ完成条件にしない。

Doc readerは独立Sentence coreのliteral読取りを使用し、suiteの`doc-sentence` featureで公開する
`nepl3_suite::adapters::sentence::document`を通して現在のDoc consumerへ渡す。
adapterは`no_std + alloc`で動作し、DocとSentenceの公開型・検査を接続する。
suiteの既定featureはこのdomain依存を有効にしない。開発hostはfeatureを明示して利用する。
Doc/Sentence core間の直接依存と、coreからtoolsへの依存を禁止する。
adapterはSentenceSyntaxを検査し、全標準Inlineを同じarena index・順序でDoc値へ変換した後、
Doc側でも構造を再検査する。CodeはDocのInlineCode、外部linkはLinkTarget::Externalへ対応し、
foreign CodeやDoc固有のpage参照へ読み替えない。foreign-inlineは個別adapterを要求する。
Source/Origin/SourceMapとViewを保持し、元SentenceSyntaxのdense位置・View ownerも呼出側で保持できる。
入力の共有・source-less Syntheticを展開や架空Spanで置き換えず、失敗・停止時は部分Doc値を返さない。

Doc catalogはSentenceの実descriptorを登録する。Doc readerの現在の出力は明示変換後の
Doc SentencePayloadであり、同じschema identityで独立Sentence payloadを装わない。
本文の意味を保持し、出力Viewの所有schemaをDoc schemaへ対応させる。文書の内部identityは
生成manifestに記録する。Markdownの出自はページ固有のsourceと実際の参照入力に対応する。
正本から生成し、本文・リンク・注釈の一致とmanifestの整合を確認する。
この接続はDoc本文の解析を独立Sentenceへ移す段階である。重複parserの撤去後も、Doc意味schemaの
Sentence所有、現行lowerが受信するDoc SentencePayload、Mathとの既存bridgeは残る。
これらは後続のconsumer所有移行で整理する。

## 注釈と移行完了条件

以下は移行後にも維持する注釈契約と、旧経路を除去する際の受入条件である。実施順・各段階の残件はT07と実装状態で管理する。旧コメントの認識だけを先に削除する変更はmainへ統合しない。

`annotate Sentence target`はarity 2の通常formであり、各host categoryへ固定shapeで登録する。
runtimeのgeneric categoryや独立Comment、commented、特殊trivia導入子を追加しない。
targetのbinding/import/export/visibilityとdomain意味を保存し、構文とannotation relationは残す。
文章中のInlineAnnoとは別kindである。生成注釈も同じ正式syntaxを返す。

各categoryで無注釈とのbinding/domain比較、source/Origin保持、通常Doc HTMLから著者注釈の除外、
literal/prefixの意味一致、native/portable往復、Unicode境界、資源停止と不正入力を検査する。
旧#消費、TriviaKind::Comment、wire/sidecar、各profile、再生成元、公式sourceを同じ移行で除去する。
旧schema拒否と旧#負例を残し、互換decoder・flag・恒久移行runtimeは残さない。
