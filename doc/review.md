# 初期設計の独立レビュー

このレビューは、実装担当とは別のエージェントが2026-09-06に行った。対象は取り込んだr1設計の14章、signature表、文法source、意味モデル、操作表、profile、依存・タスク表、例とconformance入力。設計として有用な責務分割は維持し、以下の矛盾を修正対象とした。全仕様の正しさや処理系の完成を認定するものではない。

同日のr3追補では、ユーザーが提示したWeb/TEA/Pages要件と最終Doc移行の指示、および提供された追補 `nepl3-web-pages-tea-2026-09-06-r1` の展開済み7ファイルを全て読んだ。独立にmanifestの6 payloadのSHA-256とUTF-8を検査し一致した。sandboxのZIPを直接取得したという記録ではない。元資料の検査報告はruntime実行の証拠へ転用しない。取り込み元は [記録](history/web-tea-import.json)、仕様上の訂正は [決定0003](decisions/0003-web-tea-doc-migration.md) に残す。

状態の正本は [design/review.json](../design/review.json)。`open` は影響する実装を確定する前に解消すべき設計課題、`corrected` は文書・データの訂正を独立に確認した状態であり、runtime試験の合格ではない。

## 確認した問題

| ID | 問題と再現根拠 | 必要な対応 |
|---|---|---|
| R001 | modelのrecord/sumのfield順がJSON objectの列挙順に依存する一方、NDFはfield順固定、digestはobject keyをsortする。順序を変えても同じdigestになり得る。 | 意味上の順序をarrayで記述し、canonicalizationでも保持する。 |
| R002 | Circuit本文はinitial(CheckedDesign,entry)、操作表はinitial(PreparedNetlist)。NOR本文はNextBit/OutputBitをnodeのように列挙するがmodelはsink配列。 | initialの前提をPreparedNetlistに統一し、NORは4種のnodeと2種のsinkに区別する。 |
| R003 | Doc:DocGuestだけがDoc/Article意味値を保持する。Codeは壊れたguestも構文・位置のまま表示する契約なので、この型ではlowerに失敗する入力を保存できない。 | 同言語のコード表示もForeignSyntax bundleを保持する。 |
| R004 | Math:Numberのvalueは任意RationalだがNumberのsurfaceは有限十進数のみ。Number(1/3)をfrac 1 3とprintすると別constructorへlowerされる。 | Numberを有限十進数に制約し、任意有理数からの式構築は必要に応じFracを返す。著者のFracを自動評価しない。 |
| R005 | mspaceのwidth/height/depthはNonnegativeDecimalで、非zeroの単位なし値を許す。MathML Coreのlength-percentage契約に適合しない。 | 型付きの単位付き長さに変更し、正負・単位・無効値の試験入力を加える。 |
| R006 | 操作表の入力・出力は「ParsedTree + Profile」等の説明記法であり、閉じた型schemaではない。初期状態ではNdfScalarが未定義だった。ReplyのrequestId envelope、Cancel、Closeのwire定義もない。 | NDF intrinsicとfield型参照の部分訂正は下記参照。T01/T02/T03/T12などの確定前に残るschema、framing、cancel競合を定義する。現段階のinterfaceを完全な公開交換契約として広告しない。 |
| R007 | Limits.sourceBytesに対応するStopReasonがない。 | SourceLimitを追加し、過大sourceが型付き停止になる試験を定義する。 |
| R008 | XMLのText escapingが&と<だけでは、文字データの]]>をそのまま出す。Textはvalid UTF-8なのでXMLで表せないU+0000等も入る。 | 出力先の文字制約を検査し、表せない文字を明示的に拒否する。>やCR、属性内の改行等の保存規則を固定する。 |
| R009 | profileはaliasとsource path中心で、解決済みSchemaRef/digest/provider manifestを持たない。 | source用manifestとruntimeの解決済みProfileを区別し、T05/T11で生成・差分検査を実装する。値を仮digestで埋めない。 |
| R010 | 元の43件の設計検査には実行可能な検査器のsourceが同梱されていない。 | 履歴資料として保持し、現在のCIや言語処理系の合格証拠へ転用しない。 |
| R011 | T01の参照するE03/E04/A04を群全体の合格と解すると、後続editor実装が必要なのにT01完了を前提とする依存順で進められない。 | タスク単位の成果物と範囲を明示した証拠、群全体の合格を分離し、T16で全群を要求する。 |
| R012 | acceptanceのpassedは証拠fileの存在までしか検査せず、別群・failed・古いsource/spec・不足targetの証拠でも通り得る。 | 群・結果・版・現在の入力inventory・実行環境・logを型付きで照合し、必須targetの実行を要求する。 |
| R013 | ordered_fieldsの対象がmodelだけで、contracts.recordsのfields/variantsはobjectへ戻っても検出しない。 | 同じ順序付きfield検査を両方へ適用し、拒否例を入口から試験する。 |
| R014 | 現行Docでは全ての正式文書の表・list・一般リンク・汎用code・図を損失なく表す契約がない。 | T21で全inventoryと不足を確定し、schema・文法・安全なbackend・wire・conformanceを実装してから切り替える。計画を追加しただけではこの不足を解消済みにしない。 |
| R015 | 公開後smoke失敗を赤いstatusにしても、すでに公開された失敗候補は元に戻らない。最後に公開確認まで通ったartifactの保持・条件付き復旧が未定義だった。 | 成功artifactの保管とidentity、全writerの直列化、失敗候補が現在版である場合だけの復旧、初回・保管切れ・復旧失敗の状態を定義し、S06で実検査する。 |
| R016 | inventory抽出がlink内のSoftBreak/HardBreakを捨てて単語を結合し、heading/link内のInlineMath/DisplayMath本文も落としていた。 | 共通のinline投影で区切りと数式本文・種類・source範囲を保持し、実Markdown入力の回帰試験とinventory版更新を行う。 |

## r3追補の対応確認

| 要求 | 正式な対応先と確認点 |
|---|---|
| 純粋TEAと薄いhost | [14章](spec/14-web-ui.md)、T17、U01。init/update/view/subscriptions、保存の完了通知、計算Cmd、no_std UI coreとI/Oの分離。UI交換型の閉包はR006の前提として残す。 |
| 旧応答の拒否と停止 | 14章、U02/U03。session/worker epoch、request、source集合、profile、operation、options/resourcesの全identity。Grammar/対象DSLの2文書、cancel後、Worker再作成も対象。 |
| Editorと4言語機能 | 14章、T18、U04〜U08、S03/S05。購読の解除、IME・undo/rename transaction、二重source正本の禁止、追加DSLの共通editor、各言語の明示操作とpreview隔離。 |
| 公開routeと例 | [15章](spec/15-site.md)、T19、S01/S02/S04。JSなしの静的docsとhash route Playground、api/rust、共通base、同じ例byte列と版、stable page ID。T19はruntime/UIに依存しない。 |
| Pages配布 | 15章、T20、S06。検査した同じartifact、deploy job限定権限、HTTPS smoke。現在のsource配布を動くWeb製品と読み替えない。 |
| 最終Doc移行 | [16章](spec/16-doc-migration.md)、必須T21、J01〜J04。単一正本、root Markdownの生成projection、immutable履歴と機械入力の保存、全page inventory、R014解消、旧URL/anchor、Markdown依存checkerの移行、bootstrap循環回避。 |
| 完了判定 | T16が新タスクと登録済み全required群を要求し、37という旧件数を固定しない。process provider固有のG05/W02/W03はnative targetへ限定する。 |

追補のU/S群は元のID別意味へ照合した。U04はlifecycle、U05はIME、U06は追加DSL、U08はUI schema、S03はbrowser操作、S04はJSなしのdocs、S05は外部backend不要とpreviewである。静的docsを先に公開できるという本文に合わせ、runtimeを要求していた追補T19の依存を訂正した。T16完了をdeploy前提にせず、実検査、配信、公開smoke、T20/T16の順序を保つ。追補の初回公開時にDoc移植不要という条件と、ユーザーが後から指定した最終的な全Doc移行は、T19/T20と必須T21を分けて両立させた。

TEAの純粋なModel/View/Updateと作用要求の分離は [Elm公式ガイド](https://guide.elm-lang.org/architecture/) と [Commands and Subscriptions](https://guide.elm-lang.org/effects/) に照合した。Pagesのartifact、environment、deploy権限は [GitHub公式workflowガイド](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages) に照合した。現在の設計はその責務分離と矛盾しないが、UI・Worker・Pages・文書移行の実装試験はまだ実施できない。

r3のtooling変更は差分と実際の入口を独立に確認し、`cargo test --locked -p nepl3-tools` の30試験が成功した。R012では別群・結果・版・target不足、source/specの変更・追加・削除、log改変、部分成功報告の合成を拒否する。手動レビューはcommandと別のtag付き証拠で、独立性・decision・scopeを検査する。R013ではmodelとcontractsの双方に順序付きfield検査が届き、object、重複、壊れたpair/type/shapeを拒否する。162の相対Markdownリンクに欠落はなかった。これらを根拠にR012/R013をcorrectedとした。

群全体の証拠identityは、Gitから収集した現在の入力pathと元byte列に結び付く。状態・証拠・生成task文書だけを除き、commit hash自身との循環を避ける。除外したlogも別のdigestで検査する。通常文書のLFを揃えたcheckoutから採取し、source位置fixtureをhash時に正規化しない。scope付きTaskEvidenceのsource鮮度は今回の自動検査対象ではなく、従来の独立レビューを要する。いずれの証拠も、記載された実行や意味上の正しさをhashだけで保証しない。

現在の変更はリポジトリ検査の訂正と実装計画として受け入れ可能である。R006、R009、R014はopenのまま維持する。19 crateの計画に対し実装済みは開発toolsの1 crateで、21タスク・55受入群のruntime/Web/移行の完了を認定したものではない。

R006には、Source/Origin/Environmentのtableを各操作がどのbundleで受け渡すか、ReaderPlanとReadReplyの具体型、診断・失敗codeのschema、Checked値の再検査条件も含む。文章で責務が説明されていることと、別実装でdecodeできることは別の検証対象である。

## 公開失敗からの復旧と早期Doc監査の追加レビュー

commit `25a096b` に対する指摘を独立に確認した。公開後smokeの失敗を記録するだけでは、配信済みのcandidateを復旧できない。R015の訂正では、公開確認を通った元tarをimmutable recovery releaseに保存し、取得検証後にLKGへ昇格する契約を加えた。元payloadのidentityと、再deployで変わるartifact/deployment IDは区別する。

通常公開・再実行・復旧・保管整理は同じ排他範囲を使い、変更前intentをjournalへ残す。失敗candidateが現在の公開物だと確認できる場合だけ復旧し、後続の健康な版や対象不明時は書き換えない。初回のLKG不在、保管破損、復旧失敗、run消失、保存途中の停止を明示し、自動復旧は有限にする。中断したLKG昇格には新しい公開smokeを要求し、復旧成功後も元candidate/runはfailedを維持する。これらの本文・site計画・S06失敗系を照合してR015をcorrectedとした。実Pagesの公開・復旧を検証済みという意味ではない。

[Pages API](https://docs.github.com/en/rest/pages/pages) にはexpected-currentを指定した原子的な切替契約がなく、特定deploymentの成功statusだけでは現在の配信対象を証明できない。[concurrencyとenvironmentは独立](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/control-deployments) であり、排他を使わない別writerまで保護されない。このため単一writer・journal・公開identityを合わせ、不明時に止める設計とした。通常のActions artifactは [期限切れやrun削除で失われる](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/remove-workflow-artifacts)。[immutable release](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases) のasset保護を使いつつ、release全体の削除や外部障害まで防ぐ保証はしない。

Doc監査は全T21完了を前提にせず、Docに関係する共通型・wire・意味モデルを確定する前の設計作業として行う。固定baseline `25a096bc183c2b71200902884084cd4082d0aae3` の全124ファイルがinventoryで一度ずつ分類されることを独立に照合した。57 Markdownページ、10 Rust sourceと5契約入力のbyte長・SHA-256がGitの元blobと一致した。root入口、GitHub checklist、生成task/signature、変化するhistory README、保存byte列を区別する。現在の追加・変更をbaselineの網羅性と取り違えず、移行時には最新の監査を要求する。R014の不足schemaとruntime検証はこの棚卸しだけでは解消しない。

[具体的な監査](doc-inventory.md) のDG01〜DG09を現行Docとmarkup契約へ照合し、観測した表・list・link・code等と、観測0の将来候補を区別した。11 code block、695 inline code、204 link/image、278見出しの範囲が元blobのUTF-8境界内にあり、codeのraw範囲digestが一致することも独立に確認した。裸のgeneric型がHTMLとして読まれる4箇所はinline codeへ修正されている。

最終toolingの38 Rust試験を独立に実行して成功した。ページの欠落・digest/構造の改変、現在の追加・変更・削除、履歴不足、Gitのsymlink mode、非通常の出力先、過大入力の拒否を含む。`doc-inventory --check` は固定baselineを再検証して成功し、`--check-current` は現在との差分を表示して終了コード1となった。後者は意図した拒否であり、現在の全ページを監査済みとする証拠にはしていない。

## Inline投影の欠落に対する追加レビュー

R016は旧Rust抽出器を独立した検証用programから呼んで再現した。改行を含むlink labelは`firstsecond`へ結合され、`# Energy $E=mc^2$`の見出しは`Energy `へ、数式を含むlinkも数式がないlabelへ変わっていた。[CommonMarkのsoft break](https://spec.commonmark.org/0.31.2/#soft-line-breaks) は区切りを残し、[hard break](https://spec.commonmark.org/0.31.2/#hard-line-breaks) はlink内でも改行として扱う。数式payloadはparserのイベントに存在しており、欠落はinventory側の処理によるものだった。

訂正後は共通の投影でsoft breakをspace、hard breakをLFとし、数式はinline/displayの種類とpayloadを保持する。見出し、link、入れ子imageへ同じtyped segmentをsource範囲付きで反映する。脚注markerも保存し、表示番号を推測しない。HTMLは別のliteral記録に残す方針が明記されており、要約文字列だけでHTMLやstyleを含む意味の完全同等性を判定できるとはしていない。

45件のRust試験を独立実行し、CRLF・補助平面文字・両hard break表記・両math種別・setext見出し・入れ子image・code・脚注の回帰試験が成功した。同じ57ページのbaselineからinventory schema /2を生成したこと、元page/Rust/契約のidentityと要素数が変わらないこと、482のtyped segment範囲が元blobのUTF-8境界内にあることを確認した。これを根拠にR016をcorrectedとする。R006/R009/R014と処理系・移行の未実行範囲は維持する。

## NDF intrinsicと型参照の部分訂正

R006のうち、未定義だったNdfScalarと説明のみだったTypedValueを、既存NDF/1の論理constructorの明示的な部分集合として定義した。[交換仕様](spec/09-portability.md) と [intrinsic検査](../tools/src/contract/intrinsic.rs) を照合し、12種のtag、payload field順、部分集合を確認した。これらは通常のdomain Variantとして追加符号化する型ではない。Integer/Rationalの物理payload、Record/Variantの生array、headerのSchemaRef tupleを論理field表と区別している。通常のdomain fieldとしてのSchemaRefの所有schemaと符号化は未解決として残す。

[共有の型参照検査](../tools/src/contract/types.rs) はName、List、Optionを解析し、modelとcontractsのrecord/sum field・union参照先を照合する。modelは自身の型と明示的にimportした型だけを使用できる。Unitを含む組込み型、再帰型の参照、重複owner、import漏れ、未知名、不正generic、NDF tagとsubsetの改変を検査する。operation表の説明用署名をこの検査が解釈できるとはしていない。

訂正後の52 Rust試験とrepository checkを独立に実行して成功した。実際のcontract検査入口から、未解決の入れ子型、intrinsicの欠落、使用中のimportの削除を拒否する回帰試験も含む。これはdescriptorと参照の検証であり、codec、任意精度値の正規化、schema digest、bundleの値検査、操作・frame・UI schemaの完成を示さない。R006/R009/R014はopen、21タスクと55受入群の実装状態は変更しない。

## r1初回レビューの章ごとの確認範囲

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

1. T01: 型参照の記法・束縛先とNdfScalarの定義、および未定義名・重複ownerの拒否は上記の部分訂正で追加した。残るSourceStore/SourceBundle、Environmentとresource/schema table、Origin/SourceMap bundle、Diagnostic codeと引数、Usage、Checked再検査条件を定義し、新しい型も同じ検査へ接続する。
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
