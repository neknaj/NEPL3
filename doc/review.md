# 初期設計の独立レビュー

このレビューは、実装担当とは別のエージェントが2026-09-06に行った。対象は取り込んだr1設計の14章、signature表、文法source、意味モデル、操作表、profile、依存・タスク表、例とconformance入力。設計として有用な責務分割は維持し、以下の矛盾を修正対象とした。全仕様の正しさや処理系の完成を認定するものではない。

状態の正本は [design/review.json](../design/review.json)。`open` は影響する実装を確定する前に解消すべき設計課題、`corrected` は文書・データの訂正を独立に確認した状態であり、runtime試験の合格ではない。

## 確認した問題

| ID | 問題と再現根拠 | 必要な対応 |
|---|---|---|
| R001 | modelのrecord/sumのfield順がJSON objectの列挙順に依存する一方、NDFはfield順固定、digestはobject keyをsortする。順序を変えても同じdigestになり得る。 | 意味上の順序をarrayで記述し、canonicalizationでも保持する。 |
| R002 | Circuit本文はinitial(CheckedDesign,entry)、操作表はinitial(PreparedNetlist)。NOR本文はNextBit/OutputBitをnodeのように列挙するがmodelはsink配列。 | initialの前提をPreparedNetlistに統一し、NORは4種のnodeと2種のsinkに区別する。 |
| R003 | Doc:DocGuestだけがDoc/Article意味値を保持する。Codeは壊れたguestも構文・位置のまま表示する契約なので、この型ではlowerに失敗する入力を保存できない。 | 同言語のコード表示もForeignSyntax bundleを保持する。 |
| R004 | Math:Numberのvalueは任意RationalだがNumberのsurfaceは有限十進数のみ。Number(1/3)をfrac 1 3とprintすると別constructorへlowerされる。 | Numberを有限十進数に制約し、任意有理数からの式構築は必要に応じFracを返す。著者のFracを自動評価しない。 |
| R005 | mspaceのwidth/height/depthはNonnegativeDecimalで、非zeroの単位なし値を許す。MathML Coreのlength-percentage契約に適合しない。 | 型付きの単位付き長さに変更し、正負・単位・無効値の試験入力を加える。 |
| R006 | 操作表の入力・出力は「ParsedTree + Profile」等の説明記法であり、閉じた型schemaではない。NdfScalarはexternal宣言だけで定義がない。ReplyのrequestId envelope、Cancel、Closeのwire定義もない。 | T01/T02/T03/T12などの確定前にschemaを閉じ、全型参照とfield順、framing、cancel競合を定義する。現段階のinterfaceを完全な公開交換契約として広告しない。 |
| R007 | Limits.sourceBytesに対応するStopReasonがない。 | SourceLimitを追加し、過大sourceが型付き停止になる試験を定義する。 |
| R008 | XMLのText escapingが&と<だけでは、文字データの]]>をそのまま出す。Textはvalid UTF-8なのでXMLで表せないU+0000等も入る。 | 出力先の文字制約を検査し、表せない文字を明示的に拒否する。>やCR、属性内の改行等の保存規則を固定する。 |
| R009 | profileはaliasとsource path中心で、解決済みSchemaRef/digest/provider manifestを持たない。 | source用manifestとruntimeの解決済みProfileを区別し、T05/T11で生成・差分検査を実装する。値を仮digestで埋めない。 |
| R010 | 元の43件の設計検査には実行可能な検査器のsourceが同梱されていない。 | 履歴資料として保持し、現在のCIや言語処理系の合格証拠へ転用しない。 |
| R011 | T01の参照するE03/E04/A04を群全体の合格と解すると、後続editor実装が必要なのにT01完了を前提とする依存順で進められない。 | タスク単位の成果物と範囲を明示した証拠、群全体の合格を分離し、T16で全群を要求する。 |

R006には、Source/Origin/Environmentのtableを各操作がどのbundleで受け渡すか、ReaderPlanとReadReplyの具体型、診断・失敗codeのschema、Checked値の再検査条件も含む。文章で責務が説明されていることと、別実装でdecodeできることは別の検証対象である。

## 章ごとの確認範囲

| 章 | 確認した契約 | 結論・残る検証 |
|---|---|---|
| 00 対象 | 4言語、prefix arity、構文と実行の分離、完成の定義 | 境界は維持。公開交換契約の完成表現はR006に従い限定する。 |
| 01 構成 | 18 crateの責務、13 no_std、依存方向、bootstrap生成物 | 目標と現在のCargo memberを分離。依存・targetの実検査は実装段階。 |
| 02 基盤 | UTF-8 snapshot、Origin、診断、予算、結果 | R001/R006/R007。位置変換や逆写像の実試験は未実行。 |
| 03 Reader | ordered choice、巻戻し、NeedMore、commit、入力境界 | 意味記述を確認。具体的reader値型・provider schemaはR006。 |
| 04 Grammar | form/leaf、binding、foreign、bootstrap | 4文法sourceの155 formのkind・field順・読取文脈がforms.jsonと一致。compiler/bootstrapは未実装。 |
| 05 Doc | sentence、prefix、parallel、ラベル、Code | R003。sentence内部の意味等価性は今回の構造検査の対象外。 |
| 06 Math | exact/symbolic、束縛、表示と計算、MathML | R004/R005。独立算術実行やpixel描画の合格は主張しない。 |
| 07 Circuit | 階層、同時更新、state feedback、NOR | R002。NORの論理式を確認。vector/NOR evaluator比較は未実行。 |
| 08 Editor | revision、UTF位置、回復、rename、trust | 契約の分離を確認。LSP実行・増分解析比較は未実行。 |
| 09 交換 | NDF、field順、provider、frame | R001/R006。CBORで運ぶことだけで交換可能とはしない。 |
| 10 統合 | bridge、入れ子、環境、CLI、Wasm | R003/R009。profile解決と各runnerの実行は未実装。 |
| 11 受入 | 37群、property、runner不在、履歴 | R010。現在のmetadata CIとruntime合格は別。 |
| 12 モデル | surface/meaning、Checked、IR、markup | R001/R003/R004/R006/R008。schemaに未記述の型を黙ってRust型で補わない。 |
| 13 再現性 | hash循環回避、順序、source表、markup byte列 | R001/R008。正規化・digest・serializerの実装試験は未実行。 |

独立した検査器で、4文法sourceと20例のprefix構造を確認した。Grammar 68、Doc 32、Math 29、Circuit 26の計155 formについて、source宣言とforms.jsonのkind、field名・順序、読取category/list/builtinを比較した。再現可能な検査器を [tools/audit/structure.py](../tools/audit/structure.py) として管理し、字句の読み飛ばし・未閉じ引用符・重複form・過大ネスト等の拒否を [test.py](../tools/audit/test.py) で検査する。外側の構造だけを扱い、識別子は配布例に必要なASCII部分集合に限定する。Unicodeの版別XID適合性、BCP47の全条件、sentence内部、binding、意味計算、readerの実行・失敗回復を検証したとはしない。

再配置・修正後の検査器を独立に実行し、24 source・155 formの照合と6件の検査器試験の成功を確認した。R011についてはtask moduleの7件のRust試験を実行し、前段タスクの範囲付き完了、T16の全群要求、空の証拠・別タスクの証拠の拒否を確認した。元の43件の設計検証を再現したという記録ではない。

CIと開発toolsも独立に読んだ。3 OSのnative検査、全jobの成功を要求するquality、mainの検査済みSHAからのsource配布、読み取り権限、固定action revision、workspaceと設計依存表、UTF-8・JSON重複key・参照path・タスク状態の検査を確認した。公開APIの未定義部分や外部crateのcross-target適合性まで、このmetadata検査から推定しない。

## 次の実装前に閉じる設計課題

R006はリポジトリ整備を止める理由にはしないが、影響する公開APIやportable経路の完成を止める。次の作業を仕様・schema・conformanceの一つの変更として行う。

1. T01: 型参照の記法と束縛先を固定し、NdfScalar、SourceStore/SourceBundle、Environmentとresource/schema table、Origin/SourceMap bundle、Diagnostic codeと引数、Usage、Checked再検査条件を定義する。未定義名と重複名をschema checkerで拒否する。
2. T02: NDF tagと全型を対応させる。Record、Variant、union、Referenceのwire表現とfield順、tag3 Natural制約、bundle内ID空間、canonical byte列の既知入力を定義する。異なるJSON parserのobject順に依存しない検証を入れる。
3. T03: ReadRequest/ReadReply全variant、ReaderPlan、combinator値型、state/facts/view、provider manifest、Read/Transform/DependentReaderの入力・出力を具体化する。seqの異種値、choiceの結果型、名前leafのText値とlexemeの関係も決める。
4. T12: Invoke/Resume/Reply/Cancel/Closeを一つのFrame schemaへ定義する。ReplyはrequestIdで対応づけ、cancelとcompleteの競合、close後、重複ID、切断途中frame、unknown continuationの失敗と再開予算を固定する。
5. 各domain実装: `DomainSyntax` 等の説明用名を操作ごとの入出力recordに置き換える。Docのlower/check/prepare/render/plain_text、Mathのlower/check/evaluate/free_symbols/render、Circuitのlower/check/elaborate/initial/observe/step/run_tests/lower_nor/diagram、各print、Grammar compile、engine parse/analyze/queryの署名とschemaを閉じる。

R009は、T05の検査済みpackageとT11の標準profile生成時に解消する。source用manifestを読み、実際のSchemaRef、provider版と署名、bridge、resources、Limitsを含む解決済みProfileを生成する。仮のdigestや空のprovider結果を成功値にしない。

実装タスクを完了へ変更する際は、対応するopen findingを解消し、独立レビューを受ける。リポジトリ検査の成功だけでこの手順を省略しない。

## 外部仕様との照合

JSON objectは順序を持つデータ構造として規定されておらず、arrayは順序を保持する。R001の対応根拠は [RFC 8259 §4–5](https://www.rfc-editor.org/rfc/rfc8259.html#section-4)。

mspaceの寸法属性はCSSのlength-percentageを使う。R005の対応根拠は [MathML Core §3.2.5](https://www.w3.org/TR/mathml-core/#space-mspace)。旧MathMLの長さの扱いをMathML Coreへ流用しない。

XML 1.0は許可する文字集合と文字データ中の]]>を制限する。R008の対応根拠は [XML 1.0 §2.2](https://www.w3.org/TR/xml/#charsets) と [§2.4](https://www.w3.org/TR/xml/#syntax)。UTF-8として妥当であるだけではXMLとして妥当とは限らない。

## 検証の限界

仕様の通読、具体例による矛盾の確認、公開規格との照合を行った。全formのRust parse/lower、NDF roundtrip、bootstrap、browser描画、回路実行、LSP、portable provider、cross-target conformanceはこのレビューでは実行していない。対応する実装が存在する段階で、implementation-status.jsonの未実行記録を実行証拠とともに更新する。
