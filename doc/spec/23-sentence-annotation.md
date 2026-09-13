# 23. 構造化文章と対象付き注釈への是正

この章は[#158の統合設計](../decisions/multilanguage-hca.md)を実装へ具体化する。
Sentence/A移行が完了するまでの作業branch上の契約であり、既存Doc・readerの移行完了を意味しない。
旧コメントの認識だけを先に削除する変更はmainへ統合しない。

## 所有者と実装順

NEPL3sentenceは独自LanguagePackage、Sentence root、sentence literalと前置構築、
単独parse/check/printを持つ文章言語である。NEPL3aはSentenceと対象syntaxの付与関係、
NEPL3dは文書構造を所有する。Doc本文をA経由にしない。
Sentence coreはfoundationだけへ依存し、Doc/Math/A/engine/hostへ依存しない。
reader adapter、language package compile、各hostとの橋渡しは外側へ置く。

実装順は、独立した有限文章モデルと検査、公開schemaとcodec、literal/prefixとLanguagePackage、
Aの具体category別wrapperとbinding、Doc/Math consumerの移行、公式source移行、
旧comment-as-triviaの認識・schema・codec・生成器の撤去とする。
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
送信前と受信後にnativeと同じarena検査を行う。schemaだけに適合する循環・不正参照・空Ruby/Annoを
成功値にしない。各foreign closureはhostが選んだFoundationValueCodecでsource・Origin・environmentを
検査する。ambientなsourceによる欠落閉包の補完は認めず、guestの意味評価は行わない。

SentenceValueの局所node順・共有参照・notes順・文字列・Breakは保持する。foreign bundleの内部は
foundationの正準化規則に従うため、任意の送信元NodeRef番号を永久identityとはしない。
owner Originとguest Originを混ぜず、正準CBORの再encodeと意味上の参照対応を検査する。
この値境界は局所文章のSource/Viewを持つsyntax boundaryとは別であり、印字可能性や安全なHTMLのproofでもない。

## 注釈と移行完了条件

`annotate Sentence target`はarity 2の通常formであり、各host categoryへ固定shapeで登録する。
runtimeのgeneric categoryや独立Comment、commented、特殊trivia導入子を追加しない。
targetのbinding/import/export/visibilityとdomain意味を保存し、構文とannotation relationは残す。
文章中のInlineAnnoとは別kindである。生成注釈も同じ正式syntaxを返す。

各categoryで無注釈とのbinding/domain比較、source/Origin保持、通常Doc HTMLから著者注釈の除外、
literal/prefixの意味一致、native/portable往復、Unicode境界、資源停止と不正入力を検査する。
旧#消費、TriviaKind::Comment、wire/sidecar、各profile、再生成元、公式sourceを同じ移行で除去する。
旧schema拒否と旧#負例を残し、互換decoder・flag・恒久移行runtimeは残さない。
