# 初期設計の独立レビュー

このレビューは、実装担当とは別のエージェントが2026-09-06に行った。対象は取り込んだr1設計の14章、signature表、文法source、意味モデル、操作表、profile、依存・タスク表、例とconformance入力。設計として有用な責務分割は維持し、以下の矛盾を修正対象とした。全仕様の正しさや処理系の完成を認定するものではない。

同日のr3追補では、ユーザーが提示したWeb/TEA/Pages要件と最終Doc移行の指示、および提供された追補 `nepl3-web-pages-tea-2026-09-06-r1` の展開済み7ファイルを全て読んだ。独立にmanifestの6 payloadのSHA-256とUTF-8を検査し一致した。sandboxのZIPを直接取得したという記録ではない。元資料の検査報告はruntime実行の証拠へ転用しない。取り込み元は [記録](history/web-tea-import.json)、仕様上の訂正は [決定0003](decisions/0003-web-tea-doc-migration.md) に残す。

状態の正本は [design/review.json](../design/review.json)。`open` は影響する実装を確定する前に解消すべき課題、`corrected` は記載された範囲の訂正を独立に確認した状態である。初期の設計訂正と、r4で初めて実行したcore/wireの試験を区別し、必須受入群全体の成功を意味させない。

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

r3時点の変更はリポジトリ検査の訂正と実装計画として受け入れ可能とした。当時は19 crateの計画に対し実装済みが開発toolsの1 crateであり、21タスク・55受入群のruntime/Web/移行の完了を認定しなかった。R006、R009、R014はopenのまま維持し、後続のr4 runtime実装レビューは下記に分けて記録する。

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

## r3でのNDF intrinsicと型参照の部分訂正

R006のうち、未定義だったNdfScalarと説明のみだったTypedValueを、既存NDF/1の論理constructorの明示的な部分集合として定義した。[交換仕様](spec/09-portability.md) と [intrinsic検査](../tools/src/contract/intrinsic.rs) を照合し、12種のtag、payload field順、部分集合を確認した。これらは通常のdomain Variantとして追加符号化する型ではない。Integer/Rationalの物理payload、Record/Variantの生array、headerのSchemaRef tupleを論理field表と区別している。通常のdomain fieldとしてのSchemaRefの所有schemaと符号化は、このr3段階では未解決だった。r4ではnepl3.foundationの通常recordとして定義し、下記の実codecで確認している。

[共有の型参照検査](../tools/src/contract/types.rs) はName、List、Optionを解析し、modelとcontractsのrecord/sum field・union参照先を照合する。modelは自身の型と明示的にimportした型だけを使用できる。Unitを含む組込み型、再帰型の参照、重複owner、import漏れ、未知名、不正generic、NDF tagとsubsetの改変を検査する。operation表の説明用署名をこの検査が解釈できるとはしていない。

訂正後の52 Rust試験とrepository checkを独立に実行して成功した。実際のcontract検査入口から、未解決の入れ子型、intrinsicの欠落、使用中のimportの削除を拒否する回帰試験も含む。これはdescriptorと参照の検証であり、codec、任意精度値の正規化、schema digest、bundleの値検査、操作・frame・UI schemaの完成を示さない。R006/R009/R014はopen、21タスクと55受入群の実装状態は変更しない。

## r4の最初のcore・wire実装レビュー

開始commitは `6c9dd5f07376a3920a52ef61022d0adc011cca9f`。以下は `feat/foundation-runtime` の変更中の実装を対象とする独立レビューで、開始commitそのものに実装が存在したという記録ではない。実装担当へ再現入力を戻し、訂正された公開APIとproduction試験を再実行した。

| ID | 観測した失敗 | 訂正と残る範囲 |
| --- | --- | --- |
| R017 | URI-based SourceRefが独立文書のSourceIdを失い、UsageがLimitsと同型だった。 | r4でIDとlocator、許容量と使用量を分離。nativeのURI競合拒否まで確認した。portable source adapterとschema/nativeの最終照合は継続する。 |
| R018 | imported Originがnodes/depthゼロでも成功、reportがallocationゼロでも成功、SourceMapがdisjointな逆方向対応をCycleと誤判定、4段のhost/guest構造がdepth 3で成功した。 | 共通予算、事前allocation検査、byte/anchor単位の対応graph、guestへ引き継ぐnode深さを訂正。unused schema参照もfinalize時に検査する。 |
| R019 | 約200 KBの入力に100,000段のSomeを入れるとcodecがstack overflowした。codec訂正後も返されたStructuralValueのClone/Eq/Debugが同様に落ちた。 | codec・cleanup・Clone/Eqを反復処理に変更。Debugはcontainerのheader/countを示す明示的な要約とする。完全な値の交換にはNDFを使う。 |

`cargo test --locked -p nepl3-core -p nepl3-wire` を独立実行し、core 42件とwire 7件が成功した。source/位置/atomic edit、schema digestと参照、Origin/View/構文graph、共有予算、NDF全12tagの既知byte列、非canonical入力と切断、深い値の所有・比較を含む。R018/R019の訂正範囲はこれらの実装であり、T01/T02全成果物やR006の全操作契約を完了とはしない。

後続のsource adapterレビューでは、SourceAdmissionが10,007 byteのURIを複製する際、allocationUnits=100でも65だけを計上して成功する不備を公開APIで再現した。保存するtupleとURI payloadの事前計上へ訂正後、同じ入力はAllocationLimitで停止する。管理対象の `admission_accounts_for_copied_locator_payload` を含む `cargo test --locked -p nepl3-core` の43件を独立再実行した。

WASI CIの追加は、native/WASIの両job成功をquality条件に含むことと、Wasmtime Linux配布物の固定SHA-256が[公式release API](https://api.github.com/repos/bytecodealliance/wasmtime/releases/tags/v44.0.1)のasset digestに一致することを独立確認した。[Rustの対象platform資料](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip2.html)とrunner指定も整合する。このworkflowレビュー自体をLinux jobの実行証拠とはしない。

ReaderPlanへの型接続を確認する過程で、R019に別の経路を追加した。100,000段のListを持つTypeDescriptorをSchemaDescriptorへ入れると、`reference` はdepth=128で正しくDepthLimitを返すが、その後の通常dropがSTATUS_STACK_OVERFLOWとなった。隔離した `--bin deep_descriptor` で再現し、descriptorのDrop/Clone/Eqを反復処理、Debugを上位構造の表示へ訂正した。元の再現に加え100,000段のList/Option交互構造もclone/比較/表示/失敗cleanupに成功した。管理対象の `rejected_deep_descriptor_is_safe_to_clone_compare_debug_and_drop` を含むcore 45件を独立実行し、R019を再びcorrectedとした。

TokenがViewBundleを所有する変更では、head外のviewをroot以外の表要素へ置くとnative/portable双方で受理される漏れを発見した。全owned elementへ包含検査を適用し、`token_owns_even_unreachable_view_elements` が追加された。上記45件と、別入力のnative validate・portable encode・改変wireのdecodeがCoverで拒否されることを独立確認した。これはSyntaxBundle全体の参照移送の完了を意味しない。

typed adapterではsource 3件・view 2件・syntax 5件・origin 1件とcodec 7件、計18件を独立実行した。SourceBundleの同一URI別ID、元byte列、偽digest、canonical source順、部分失敗後の一度だけの計上、CBOR/UTF-8を先に検査する失敗順、Spanのscalar境界を含む。foreign bundleのTokenRef/ViewRef/OriginRefと構造化payloadの保持、環境digest偽装・Origin cycle拒否も確認した。同じ2-node graphのnative配置だけを変えた `--bin canonical_syntax` は当初wire bytes不一致かつroot=1だったが、root-first再採番の訂正後はroot=0とbyte一致を確認した。guest root/childの独立再採番、非canonical入力と未到達nodeの拒否も管理対象回帰に含む。

Environment内容digestと参照先Origin閉包のidentityは別物であることを13章へ明示した。reader native入口はprivateなCheckedReaderContextを要求し、実際の環境digestとOriginが参照するsource閉包を検査する。context 1件とportable request 2件を含む `cargo test --locked -p nepl3-core -p nepl3-reader -p nepl3-wire` を独立実行し、core 45件・reader 26件・wire 18件が成功した。requestのNDF loopbackでは宣言source表を二段で復元し、同じReaderSessionのvalue/end/stateが一致する。宣言から欠落したsourceをhostのglobal storeで補っても拒否する。reply/continuation全体のportable化と別process provider比較は未実装であり、R017はopenのまま維持する。

reader予算の独立再現には `cargo run --locked --manifest-path .tmp/independent-core/Cargo.toml --bin reader_allocation` を使用した。100,000 byteのSourceIdを持つsnapshotを準備し、ReaderSession constructorの消費分だけを上限として同じ操作Budgetを使い切る。その後のread区間だけをSystem allocator wrapperで計測すると、Stopped(AllocationLimit)を返しながら200,000 bytesを割り当てた。fixture構築、session構築、結果の表示は計測外である。借用による位置・identity検査へ訂正後、同じread区間の実allocationは0になった。管理対象の `long_source_identity_is_not_copied_after_allocation_budget_is_exhausted` も上記reader試験に含む。計測用harnessはignoredの補助証拠であり、clone可能な正式試験の代替とはしない。

`--bin reader_stopped_report` はchecked contextを構築し、Seqのproviderがdiagnostic/eventを各1件返した後、長いliteral期待値のallocationで停止させた。返却はStopped(AllocationLimit)だったが、Reportの両列は0件、Usageのdiagnostics/eventsは各1となった。drive/finishで現在の報告を保持する訂正後、元の再現と管理対象の `stopped_reader_retains_committed_provider_reports` は両列1件を保持した。ただし追加の `--bin reader_second_resume` ではSeq(Call, Call)の第2回resumeでallocation停止すると、最初のproviderの報告が再び消えた。上限79,200、使用量79,169でStopped、diagnostics/events使用量は各1、返却列は各0だった。resume準備ではcheckpoint、provider検査後ではMachineの報告を保持する訂正後、同じ補助probeの10,000〜179,900を100刻みとする全走査で報告を保持した。管理対象の `allocation_faults_across_later_resumes_preserve_already_accepted_reports` は3回のCallを使い、後続resumeの停止と高予算での完了を確認する。追加後のruntime 24件を独立実行し成功した（reader合計27件）。これらのfixture helperは入力構築だけに使い、実行VMはproduction APIを呼んでいる。

checked contextを別operationで再利用する `--bin reader_context_conflict` は、閉包sourceと同じID/revisionで内容が異なるsnapshotをcaller storeへ置くと、訂正前はMatchedを返した。digest込みの検索で不一致を単なる不在と扱ったためである。訂正後はIdentityConflictを返し、管理対象の同名境界試験も成功した。末尾に `-- missing` を付けてcaller storeから当該sourceを除くと、proofが保持する閉包から供給してMatchedとなることも独立確認した。不在と衝突を区別し、operationごとに閉包のsource予算を再計上する。

readerの26件はnative VM、UTF-8分割、Unicode 16、巻戻し、消費するprovider反復、非進行・隠れた再帰、state復元、署名・session echo・予算の検査を含む。`map_decode_and_then_use_typed_provider_envelopes` はprovider dispatchと値の対応を検査する。escape decodeによる生成source/SourceMapの意味や、そこでのlook/choice巻戻しの証拠とはしない。SyntaxBundleには現時点でSourceMap所有tableがなく、readerの生成source対応をbundle全体で交換する契約も次段へ残る。組込みreader、完全なprovider交換、engineとGrammar bootstrapは次の実装・レビュー範囲である。

独立probeはignoreされた `.tmp/independent-core/` から実際のcrateをpath依存で呼んだ。この補助harnessはcloneに含まれず、配布されたproduction試験として再実行できるものではない。実行証拠の中心は上記の管理対象の回帰試験である。`cargo run --locked --manifest-path .tmp/independent-core/Cargo.toml --bin deep_wire` で100,000段のdecode→encode一致→drop、およびListの第1子に深い正常値、第2子に不正byteを置いた失敗cleanupを再実行し、成功した。`--bin deep_traits`、同コマンド末尾の `-- eq` と `-- debug` は、checked decodeの成功値のClone/Eq/Debugを別processで確認した。旧版の終了は `0xc00000fd`、訂正後は全て終了コード0だった。これらの本質的な失敗条件は管理対象のcore/wire回帰試験にも残る。

追加の `--bin wire_corpus` は、長さ0〜2の全byte列65,793件と、seed `0x71921a13` の32-bit LCG（`state = state * 1664525 + 1013904223`、wrapし上位byteを使用）による100,000列を実行した。後半の長さは連番 `n % 65`。計165,793入力でpanicはなく、受理された2件はencode後もbyte一致だった。この短い不正入力probeは深い構造や全意味のfuzz網羅性を示さず、管理対象の受入試験・runnerの代わりにしない。

実行環境はWindows `10.0.26200.0`、target `x86_64-pc-windows-msvc`、rustc `1.97.0 (2d8144b78 2026-07-07)`、通常のCargo debug runner。WASI/browser/他OS、Grammar bootstrap、portable operation/frame全経路、言語・editor・UI・Pages・Doc移行の受入はこの結果から推定しない。入力identityと最終受入証拠は、対応する実装と仕様を固定したtreeで別途記録する。

## 組込みreaderとSourceMap接続の追加レビュー

次段 `feat/reader-builtins` はfirst sliceの `3bccd48d49ee1a7564335c4fa703960791ea4bc3` から継続する。R020では、1 byteのExact mappingと100,000 byteのSourceIdを使い、allocation_units=0の `SourceMap::insert` がStoppedを返す前に100,000 bytesを割り当てることを独立確認した。最初のignored `--bin map_allocation` によるSystem allocator計測はinsert呼出し区間だけを対象とした。SourceMapのpoint構築がsnapshot identityを複製してからstackの予算を検査する経路であり、前段R018の訂正範囲をこの未検査経路へ広げない。

Pointを借用identityへ訂正後、同じ測定は0 bytesとなった。mapsの管理対象2試験も成功したが、返却値と論理予算だけの試験は旧版でも成功するため、この先行heap allocationを捕捉する回帰とは呼ばない。そこで [独立計測器](../tools/audit/allocation/run.py) を管理対象へ置き、[開発文書](development.md) でdev専用GlobalAlloc wrapperの限定的unsafeを明示した。production/workspaceのforbidは変えない。固定Rust 1.97.0のCargo-selected rlibへリンクし、4,096 bytesのpositive controlを通してから測定する。同一計測器を旧commitの別worktreeへ適用すると100,000 bytes・終了1、修正treeでは0 bytes・終了0となった。[旧版log](../conformance/results/reader-builtins/allocation-baseline.log) と [修正中treeのlog](../conformance/results/reader-builtins/allocation-working.log) を保存した。native CIへの必須step追加も確認し、この具体的な先行割当をR020 correctedとする。後者のlogはfreeze前の部分記録であり、最終treeの証拠は統合時に別途採取する。

R021は別のWork計上不備である。公開builtin Textへ3 byteの入力と100,007 byteの予約URIを渡すと、Work上限1,000でもMatched、Usage.work=7となった。URI末尾だけを不正spaceにすると、長いURIを検査後にLocator、work=1となった。`--bin builtin_reservation` で再現し、builtin内部の予約検査と、Budget付きsource constructor/importのlocator再検査へ事前Work計上を要求した。低水準の所有値validatorの責任と、Budgetを受け取る操作内部の責任を区別し、先行heap allocationを測るR020とは別に追跡する。

訂正後は同じvalid/末尾space URIの両入力がWorkLimitで停止した。管理対象の `generated_source_locator_scan_is_charged_before_validation` と `reservation_locator_validation_obeys_work_limit_before_scanning` を独立実行し成功を確認したため、このlocator経路をR021 correctedとする。任意長のidentity比較全般を検証済みとは扱わない。

R022では、providerが診断/eventを各1件生成してからAllocationLimitとなる公開tokenizer経路が両方を捨て、Usageだけ1を残す不具合を再現した。またTriviaを受理してText予約で待機した後、同じLimitsの新しいBudgetを渡すとUsageが減少したままtokenを返した。訂正後、元のprobeは診断/event各1件を保持し、新BudgetはContinuationとして拒否した。

Reportだけを引き継ぐと、生成source上のSpanに必要なsource/mapが失われるため、ReadReplyの全terminalへ正式artifactを追加する契約も確認中である。単独VMの管理対象 `failed_stopped_and_rollback_results_have_their_exact_formal_source_closure` は独立実行で成功した。Await外包の複製失敗を追加で走査すると、allocation上限18,792/18,920/19,048でStoppedを返した後に内側pendingが残り、次のreadがBusyとなった。`.tmp/independent-core/` の `--bin tokenizer_pending` はこの公開API再現であり、管理対象回帰と修正の確認まではR022をopenに保つ。

この後、停止で返せなくなったAwaitの内側slotを破棄する訂正を確認し、元probeの全走査はfailures=0となった。管理対象のpending allocation sweep、先行skipから後続NoMatch/NeedMore/Failed/Stoppedへの正式source/report保持、および生成source上の診断を保った停止が成功した。reader全44試験（builtin 13、context 1、portable request 2、runtime 28）を独立実行した。最後に予約待機中cancelの回帰を読み再実行し、正式trivia/cursor/state保持、slot消費、同sessionでの新operation再開を確認した。このnative範囲をR022 correctedとし、全reply/continuationのcodecとprocess交換はR017に残す。

新たなmapped containmentでは、source表とmap表が存在するだけで別snapshotのviewを受理してはならない。子の全byteと空anchorがmapを介して親の範囲に帰属すること、穴や外部の起源を隠さないことを検査対象とする。escapeの生成source/mapは失敗choiceやlook後に正式結果へ混入せず、消費済み予算は戻さない。

実装されたSourceMap所有tableとmapped containmentについては、core maps 2件とwire syntax 6件を独立実行した。host/guestそれぞれの生成source、別snapshotのview、空anchorの往復、およびmap欠損・無関係range・Exact偽装・guest source宣言不足の拒否を確認した。これは全reader reply/continuationのportable化やprocess provider比較の完了を意味しない。

reader修正のfreeze後、`cargo test --locked -p nepl3-core -p nepl3-wire` も独立実行し、core 50件・wire 19件が全て成功した。ここでの件数はproduction回帰の実行範囲であり、55受入群の合格数とは異なる。

engineの今回の範囲は `LanguagePackage::check` の局所metadata/shape検査である。4件の公開API試験を独立実行し、field/selector/binding、登録済operationとの署名相違、provenanceの未宣言source・不正OriginRef、ゼロWork/Depth、直接ReaderId cycleの拒否を確認した。ListOfのforeign headをLocal NodeRefへ潰す草稿と、binding退出frameを1段深く計上する草稿を指摘し、ForeignSyntax head/NodeRef tail、およびBinding::Noneの深さ1の実回帰で訂正を確認した。

WithModeは構文rootのownerへ適用し、hostのListOf spineとguest要素を分ける。guestのmodeをhostの同名modeで検査済みにしないという訂正文と局所検査を照合した。文字化けした規範段落も修復後に再読した。package意味digest、解決済EntryContext/Profile、prefix parse、Grammar compile/bootstrapはこの4件の範囲に含まれない。特にsurface SchemaRefの一致だけで挙動identityやcache有効性を保証せず、R006/R009をopenに保つ。

Langの契約は [RFC 5646 §2.2.9](https://www.rfc-editor.org/rfc/rfc5646.html#section-2.2.9) と照合した。well-formedはABNFへの適合であり、登録済みsubtagやvariant/extensionの重複拒否を含むvalidとは異なる。grandfatheredとprivate-useもABNFの対象に含む。文法だけの検査にnetwork照会や未指定のvalid制約を追加しない。

## prefix engineへ向けた追加レビュー

`8416e1f7b130012abcd0a6d5c79fe544ec7dcbfe` の組込みreader区切り後、`feat/prefix-engine` でowned TokenizationContinuationのnative境界を検査した。既存のreader 46試験は独立実行で成功した。一方、R023として停止済みの新Budgetを使う組合せを再現した。入力はspaceと引用されたaの4 bytes。Triviaを受理してReserveとなった時点のUsageはsource 4/work 1,231/allocation 12,012だったが、同じLimitsの新品Budgetをcancelしてreserveへ渡すと、Stopped(Cancelled)としてslotを消費しUsageを全0で返した。

原因はprivate slotのLimits/累積Usage照合より先にpollと停止結果を返す処理を置いたことだった。独立補助probeは `.tmp/independent-core/` の `--bin tokenizer_cancel_reset` で本番APIを呼び、修正後はContinuation拒否となった。履歴sourceを比較すると、8416e1fのtokenizer reserveは照合をpollより先に行い、ReaderSession resumeは既にpollが先だった。後者の旧版での動的再現は実行していないため、3経路全てを今回導入された不具合とは扱わない。

tokenizer reserve・tokenizer resume・ReaderSession resumeの3公開入口で、cancel済みの新Budgetを拒否した後も元slotで正式に再開できる管理対象回帰を実コードで照合した。修正後のreader 46試験は独立実行で成功した。その後に強化された `mode_text_reservation_is_lazy_and_echo_is_checked`、`cancellation_during_provider_wait_stops_and_consumes_the_pending_slot`、`tokenizer_preserves_accepted_skip_reports_and_sources_across_candidate_rollback` も `cargo test --locked -p nepl3-reader <試験名>` で個別に再実行し成功した。正規Budgetのcancelでは、先に受理した診断・event・累積Usageを保持し、tokenizerでは生成source・Triviaも保持する。R023はこのnative境界についてcorrectedとし、前段R022の検証範囲と区別する。完全なcontinuation codecとprefix実行はこの訂正の完成条件に含めていない。

新しいaccepted collectorでは、R024を公開APIから再現した。`AcceptedTokenizationReport::diagnostic` へ一時host store内のauxiliary sourceを指す診断を渡すと、診断1件・所有source 0件で受理された。その後cancelし、元入力だけのstoreで `read_with_accepted` を呼ぶと、Stopped(Cancelled)にauxiliaryのprimary Spanだけが残った。独立補助入力は `.tmp/independent-core/src/bin/collector_closure.rs`、実行は `cargo run --manifest-path .tmp/independent-core/Cargo.toml --bin collector_closure`。修正後の同probeは所有source 1件をcancel後も保持する。

訂正ではdiagnostic追加を明示source宣言の入口とし、primary/related/fixの参照先を検査・共有admissionで計上・所有化した後、診断とsource列を同時に確定する。root sourceだけに限定せず正当な複数sourceを保持する。管理対象の `accepted_diagnostic_owns_primary_related_and_fix_source_closure_before_publication` を独立再実行し、3種類の位置が別sourceを参照してもcancel後に全3件を保持し、fix参照先の宣言欠落とSourceLimit 0では診断・source列を確定しないことを確認した。`accepted_collector_crosses_language_sessions_only_in_its_original_operation` も独立成功し、別operation/profile/入力を拒否して正規の言語切替とcancelでは診断・event・sourceを保持する。R024はこのnative閉包についてcorrectedとする。完全なportable report/continuationの受入とは区別する。

Profileの実入口は `cargo test --locked -p nepl3-engine --test package` の7件が独立実行で成功した。補助probe `alias_recovery` では、同じpackage・同じroot modeを登録したA/BでChildカテゴリだけをCode/Alternativeへ分け、登録順を逆転してもentry結果とprofile digestを保持することを確認した。recoveryの既定方針と同期候補順の単独変更でpackage意味identityが変わることも確認した。これらはresolve/entry/identityの検査であり、未接続のprefix parserが実際の子をそのmodeで読むことや回復することの証拠ではない。

構文のstorage複製は `cargo test --locked -p nepl3-core --test syntax owned_syntax_copy` を独立実行し成功した。100k SourceIdに対する低Allocation停止、Work 0、source admissionの非再計上、32k段foreignの複製・比較・破棄を検査する。追加補助probeでは、外側Budget深さ3、4段bundle、内側tokenのSome 4段を合成し、必要総深さ11に対してlimit 10はDepthLimit、11は複製成功となった。これは `SyntaxBundle/FieldValue::clone_with_budget` のstorage/深さ検査であり、構文の意味検査やParseTree検証の完成を示さない。

永続ParseTreeの検査では `cargo test --locked -p nepl3-engine persistent_tree` の公開回帰1件が成功した。一方、補助probe `tree_mutations` で既知form `let` をLeafとして保存しても受理され、R025として修正を要求した。同じprobeは訂正後にSelection拒否となる。さらに通常の `Text → Unit` facts操作をDynamicのshape/child_contextへ登録・allowlistすると、正規HeadShapeとchild contextsを付けて受理できた。訂正では投影要求・操作署名が未実装のDynamicをUnvalidatedDynamicで明示拒否し、同じprobeと追加された管理対象負例の両方で拒否を確認した。spec04も静的・回復・payload型の検証範囲を明記した。R025は偽の成功proofを防ぐ範囲でcorrectedとし、Dynamicの成功経路やP03の完成は未達のまま維持する。

同probeで、意味Profileを保つReadSpec arena再配置に対して旧treeをExecutionIdentityで拒否し、digestだけ更新して古いread indexを残す場合も拒否、全indexを正しく写し替えれば成功することを確認した。局所child fieldをforeign pathとして使用する場合と重複BundleContextも拒否される。追加 `tree_list` probeではhost Alternative modeのListOf spineとguest BのCode modeを持つtreeが通り、tailのmode reset・guest alias差替え・局所tailをforeign pathとして指定する変更が拒否された。builtin原文 `x` とText payload `forged-name` の組合せは現在受理されるため、この検証器の型・所有位置・選択のproofをreader実行等価性へ広げない。完全なdynamic callback契約、実prefix解析、wrapper codecは別に検証する。

最初のprefix公開試験 `cargo test --locked -p nepl3-engine --test parse` の2件は独立成功した。追加補助入力 `let` と `let x let` は、修正後に同じbundleのMissing 2件を一つのrecovery索引へまとめ、tree検査にも成功した。修正前の重複索引の動的失敗は独立実行していない。

R026では、arity 2の `f(Name, Expr)` とCapture 20段のScalar readerを組み合わせ、depth limit 25で `f x ` の親を0/5/10/15個積んでも全てComplete・peak 23となることを本番APIで再現した。補助実行は `cargo run --manifest-path .tmp/independent-engine/Cargo.toml --bin parse_depth`。prefix frameのobserveだけでは内側readerのactive depthへ加算されず、共有上限を満たさない。修正と管理対象の複合境界回帰を要求している。

R027では補助probe `parse_trivia` から、final `let ` のWhitespace[3,4)、final `let x # comment` のWhitespace[5,6)とComment[6,15)、同nonfinal入力の確定Whitespace[5,6)が結果/progressに保存されないことを確認した。元snapshot bytesは残るが、tokenが生成されない終端でreaderが確定したTriviaの種別・範囲・所属を捨てている。修正草稿でR026の元probeは親0のみComplete、親5/10/15でStopped DepthLimitへ変化し、R027の元入力もHost付きbatchへ各Whitespace/Commentを保持することを再確認した。最新のproduction parse 8件は独立成功したが、停止・再開を含む範囲の追加確認までR026/R027はopenとする。

R028として、Name tokenizerがtokenを読めるがform/leafが一致しない `unknown`、`unknown tail`、`let x unknown tail` を補助probe `parse_unknown` で解析し、全てTree(Selection)となることを確認した。unknown headのtoken付きRecoveryUnparsedを生成する側と、通常node tokenを持たないUnparsed契約の不整合である。原文rangeと認識済tokenの保存先を明確にし、未知arityをleaf成功へ推測せず回復結果を返す修正を要求している。

その後、`cargo test --locked -p nepl3-engine --test parse` の11件を独立に再実行し成功した。途中の10件MissingSchemaはfixtureに追加したoperationの参照schemaをProfileへ選択し忘れた同期不備で、成功へ置き換えて記録せず修正後に再実行した。補助 `parse_boundary` でも、caller depth 7からText Reserveとprovider Awaitを再開して両方peak 13で完了し、第2Awaitのcancelで診断1件とTrivia[3,4),[5,6)を保持した。元のunknown 3入力は全てRecoveredとなり、headと全Unparsed coverを別々に保持する。`unknown tail` はhead[0,7)/cover[0,12)、`let x unknown tail` はhead[6,13)/cover[6,18)である。spec04の規則、native型、ReaderFactBatchのschemaを照合し、R026/R027/R028をこの静的native範囲でcorrectedとした。動的HeadProvider、portable wrapper、全bootstrapの完成を意味しない。補助probeはignoredであり、再実行可能な管理対象production試験を検証の中心とする。

同じpackageをA/Bへ登録しrootを共通Expr/Code、ChildだけB.Alternative（`~`をskip）へ変更した補助 `parse_alias` も実行した。`let x ~y` はAでRecovered/Child.Code、BでComplete/Child.Alternativeとなり、登録列の順序を反転しても結果・Profile digestが変わらない。これは先のentry APIだけの照合から、実ParseSessionとtree.validateまでの確認を追加したものである。

`continue_input` の管理対象append回帰2件も独立成功した。補助 `parse_append` はcaller depth 7で短い `f x ` をNeedMoreにした後、host depth 0から親8段の入力へ伸ばす。Capture20段と合わせて、depth上限36/37/38は直接解析・再開ともStopped、39/40/80は両方Complete・peak39となった。元caller深さを保持しながら、新しく増えた親の深さも計上する。これは原状態からの全再解析であり、増分再利用やproviderの全経路の完成を証明するものではない。

Grammarの初期reader loweringは `cargo test --locked -p nepl3-grammar-core` 1件、seed入力adapterは `python -m unittest discover -s tools/bootstrap -p test_grammar.py` 3件を独立に実行して成功した。seed adapterは元bytesとUTF-8 byte offsetsを保持するhost用入力変換であり、production parser/seed/P1/P2のbootstrap成功ではない。R029では補助 `grammar_catalog` で、同名NamedClassを異なる実classへ2件対応させると先頭だけで解決し、順序反転で意味が変化することを確認した。public ReaderContextの名前付きcatalogは事前検査proofを持たないため、同名/空名を明示拒否する入口検査と管理対象回帰を要求した。

補助 `check_seed_positions.py` では4文法へ日本語🙂commentを前置しLFをCRLFへ変換した入力で、全head/list/literalの範囲を元bytesと照合した。対象はGrammar 1214、Doc 701、Math 722、Circuit 620要素で、未閉じ引用符・不正escape・末尾lexical gap・余剰tokenの4入力も拒否を確認した。初回の検証器がkind名とspellingを同一と仮定して失敗したため、forms正本のcategory別対応へ訂正して再実行した。確認対象は位置と構造であり、各DSLの意味ではない。

R030では同じGrammar補助probeで、5 nodeのDocumentをnodes=0でvalidateしてもOk・Usage.nodes=0となることを確認した。他資源はsourceBytes67/work303/depth5/allocation598が計上されていた。共通のNodes上限をGrammar arenaだけ省略する理由はなく、統括ともこの境界を修正対象として合意した。実node数とDAG再訪時のwork/depthを区別して計上し、zero/exact境界の管理対象回帰を要求している。

修正後のGrammar試験2件を独立実行して成功した。R029の元probeは両順ともDuplicateCatalogName(Classes)、R030の元probeはStopped(NodeLimit)へ変化した。各imports/classes/viewsのempty/duplicate 6ケースを公開compile入口から拒否する管理対象試験を確認し、node上限4は停止、5はUsage.nodes=5で成功する追加probeも実行した。R029/R030はこの境界修正としてcorrectedとした。全23 reader formの実行、package組立て、production bootstrapは別の未達範囲である。

統括から指摘されたAngleTag例について、補助 `build_angle_probe.py` / `grammar_angle` で元sourceの全ASTとbyte spansを構築し、production reader compilerがReader(OutputType)で拒否することを独立実行で確認した。ChoiceのSeq枝2つは`List<NdfValue>`、Scalar枝はTextとなり、同型の選択契約に合わない。各枝を明示Discardで包むtyped AST候補は同じcompilerで成功した。R031として正式例source・契約説明・元不一致の負例・quoted `>` を含むtoken境界の実行試験を併せた訂正を要求した。まだ候補ASTだけの成功であり、修正済み正式sourceやG04合格とは扱わない。

R032では補助 `build_print_probe.py` / `print_source` を使い、setup Budgetで検査済みのtreeをfreshなsourceBytes上限0のBudgetへ渡しても、公開 `print::source_tree` がComplete("let x ~y")・Usage.sourceBytes=0となることを再現した。構造の検査proofと現在操作へのsource受入れは別契約である。printerにも共有SourceAdmissionまたは同等の操作所属を持たせ、別操作では受け入れ直し、同じ操作では二重計上しない修正を要求した。

共有SourceAdmission引数を追加した修正後、元printer probeはStopped(SourceLimit)・空出力へ変わり、先に同じ8-byte snapshotを受け入れた操作では印字後もsourceBytes8だった。管理対象のfresh cap0/同一操作/OutputLimit回帰を読み、parse全13件を独立実行して成功したためR032をcorrectedとした。printerは原綴りを保持する静的tree用であり、任意constructorの意味printerやportable操作全体の完成ではない。

区切り保存前にhost seed importerの実Rust試験2件を独立成功確認した。全constructorの型付きロードと、source digest・constructor・literal・list境界の偽装拒否を検査する。実package組立てのtools integration 1件も独立成功した。Angle正式sourceはdiscard付きへ訂正され、旧bytesは明示的な負例へ保存されている。元例はOutputType拒否、訂正例はpackage.checkまで成功したが、quoted `>` の実reader境界は未検証なのでR031はopenを維持する。同じ試験でbinding正式例がInvalidBindingとなることも確認した。wordのSeq出力Listと、leafのreference selfが要求するTextが合わないため、R033として明示Text readerを使う訂正と実parse/binding回帰を残す。新Grammar lower入口では提供Profileへのtree.validate再実行をコード確認し、意味identityだけで別execution配置のindexを解釈する草稿の懸念は解消方向だが、lowerのproduction正例・変異試験はまだ未実行である。

次のbootstrap段階ではR034を再現した。補助source `shared-kind.neplg` は、AtomをA/Bの別categoryで受け入れ、双方のfield schemaを同じ空列にする。spec04が明示的に認める構成だが、実host import・package compilerは2つ目をDuplicateNameで拒否した。category内の宣言重複と、category間で共有するsurface kindのshape一致を別々に検査し、許可された構成を受け入れる修正を要求した。

修正後の元Atom A/B probeは同じKindRefを持つ2つのformへcompileされた。管理対象 `shared_kind_is_category_local_with_one_shape_and_distinct_provenance` を作業treeと隔離した `NEPL3-prefix-validation` の両方で独立実行し成功した。宣言順の反転でsemantic/schema identityが一致し、category別Originが各宣言原文を指すこと、同categoryの重複と異なるfield shapeを拒否することを確認したため、R034をcorrectedとした。別途見つかったhost seed importerのnative main-thread stack overflowと、標準bootstrapの未達はこの修正承認に含めない。

R035は、実Angle sourceをhost importerへ渡す通常のnative executableで再現した。補助 `seed_load_only` は `bootstrap::load` だけで main-thread stack overflowとなり、descriptor登録やreader実行へ到達しなかった。test thread上の既存試験成功と、通常main threadで安全に実行できることは異なる。再帰constructor呼出しをpostorder worklistと借用JSONの一時lookupへ置き換えた後、同じ `cargo run --quiet --manifest-path .tmp/independent-engine/Cargo.toml --features nepl3-tools,nepl3-grammar-core --bin seed_load_only` はstack設定を変更せずexit 0・36 nodesとなった。管理対象の別process回帰が確認できるまではopenを維持する。

importer修正後の補助 `grammar_angle_runtime` は正式Angle sourceのimport・package compile・ParseSessionを実行した。二重引用符と単引用符の中の `>` を含む両入力はcursor 15・一tokenで成功し、途切れた引用符はnonfinalでNeedMore、finalで診断付きRecoveredとなった。これはR031の実行期待を満たす独立確認であり、管理対象の同じsource経路の回帰を確認後に訂正を確定する。上記補助probeはignoredであり、cloneから再実行できる管理対象試験の代わりにはしない。

R035の管理対象 `seed_import_runs_on_main_thread_for_complete_sources_and_rejects_bad_input` を作業treeと隔離 `NEPL3-prefix-validation` の両方で独立実行し成功した。実tools CLIを別processとして通常main threadで起動し、Angleと4言語の正式grammarをimportする。Angleの36 nodes、空JSON objectのShape拒否と16MiB超過の入口拒否（exit 1）も確認した。入力依存の再帰constructor呼出しは明示worklistへ置き換わり、stack設定の拡大がないため、この不具合をcorrectedとした。空objectの負例は深い不正JSON cleanupの検証とは扱わない。bootstrap全体の完成条件は引き続き未達である。

続く管理対象 `compiled_angle_reader_keeps_quoted_delimiters_and_streaming_commit` も独立実行で成功した。訂正済み正式Angle sourceをimport・compileした実ReaderPlanをReaderSessionへ渡し、両引用符のend 15、閉じdelimiter前の全cutのNeedMore、final未閉じ入力のFailedを検査する。旧sourceのOutputType拒否と併せてR031をcorrectedとした。G04全項目や標準bootstrap完了へ範囲を広げない。

factsの最初の境界はnative管理対象2件とtyped wire管理対象1件を独立実行した。ID・namespace root・source閉包・予約範囲・権限の検査と、正しいNDF構造内でのnamespace偽装、未許可resolution更新、fresh SourceLimitの拒否を確認した。補助の実FactDelta.validateでは、Source spanをRelation.sourceにする未許可追加は拒否され、同じspanへのhostの明示grantは成功する。foreign namespace・逆転予約範囲・MissingOriginも拒否された。CheckedFactSetは構造整合proofであり、参照先のlexical visibilityやT06の名前解決アルゴリズム、FactsRequest/Reply全包絡の往復をこの成功へ含めない。

SourceMapのsnapshot DAG fastpathを読み、coarse graphが循環する場合は元のbyte-point検査へ戻ることを確認した。管理対象maps 3件と既存structureのmap関連4件を独立実行し、大きいTransformedの低予算成功、forward overlap・区間が離れた逆方向・本当の循環の既存判定が維持されることを確認した。reader側のcandidate artifactはmapを一つずつ再検査する代わりに同じ全列を一度検査する変更であり、SourceMapの意味をsnapshot単位のcycleへ置き換えたものではない。

標準Grammar bootstrapの管理対象 `complete_grammar_bootstrap_uses_production_parser_lower_and_semantic_identity` をnativeで独立実行し成功した。完全な9369 bytesの原文から初期typed seedを作り、実compilerによるP0、実ParseSession・標準reader・lower・compilerによるP1、同じ原文をP1で読むP2を順に構築する。3つのsemantic identityが一致し、各ASTは518 nodesとなった。identity writerがreader正準形・binding・styleを含む実metadataを比較することも確認した。この実行は116.11秒、Work 9,126,464,211、AllocationUnits 59,758,042,639、Depth 96を消費した。AllocationUnitsは累積の論理計上量であり、同時にその容量のheapを保持したという計測ではない。高い処理コストは未改善で、これを全受入群やタスク完成へ拡張しない。

管理対象 `original_grammar_parser_compiles_independent_reader_binding_style_mutations` も独立実行した。さらにignoredの別native main-thread executableでは、元の完全Grammar seedを保ったまま、標準source自身のword readerへCommitを追加、NamespaceのbindingへScopeを追加、最初のhead styleをmarkerからcontentへ変更する3入力を個別にparse・lower・compileし、それぞれのsemantic identityが変わることを確認した。Styleの初候補keywordは未登録classでMissingClassとなったため、登録済みcontentに訂正して再実行した。この拒否は実装不具合ではない。変異ごとに新しいseedを作って比較したり、Rust ASTだけを書き換えてsource解析を省いたりはしていない。標準facts callbackの実行、全包絡codec、別process・WASI bootstrap、配布seed artifactの一致検査は別の未達範囲である。

別の補助probe `parse_text` ではText予約を実際にresumeし、`let "ok" y` がCompleteとdecoded sourceを返し、`let "a\q" y` がRecoveredで元InvalidEscape診断・位置を保持することを確認した。このbuiltinの失敗経路については元診断の消失を再現していない。

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

1. T01: 型参照・intrinsicに続き、r4ではSource/Origin/Environment/Usageと構造検査の実装・交換試験を追加した。上記部分実装と残る操作ごとのresource/schema table、Diagnostic code catalog、意味検査済み条件を区別し、未実装型を同じ検査へ接続する。
2. T02: NDF全12tag、通常SchemaRef record、foundation bundle内の局所IDとcanonical node番号を実装した。今後のdomain・操作型もwire field順・値の不変条件・既知byte列へ対応させる。特にSourceMapを含む構文bundleと全操作reply/continuationの往復は残る。
3. T03: ReadRequestとReaderPlan、state/facts/view、Read/Transform/DependentReaderのnative envelopeはr4で具体化した。request以外の全variantのportable adapter、完全なprovider manifestとの照合、builtins、文字escapeとSourceMapの意味・巻戻し、および実process経路を引き続き実装・検証する。
4. T12: Invoke/Resume/Reply/Cancel/Closeを一つのFrame schemaへ定義する。ReplyはrequestIdで対応づけ、cancelとcompleteの競合、close後、重複ID、切断途中frame、unknown continuationの失敗と再開予算を固定する。
5. 各domain実装: `DomainSyntax` 等の説明用名を操作ごとの入出力recordに置き換える。Docのlower/check/prepare/render/plain_text、Mathのlower/check/evaluate/free_symbols/render、Circuitのlower/check/elaborate/initial/observe/step/run_tests/lower_nor/diagram、各print、Grammar compile、engine parse/analyze/queryの署名とschemaを閉じる。

R009は、T05の検査済みpackageとT11の標準profile生成時に解消する。source用manifestを読み、実際のSchemaRef、provider版と署名、bridge、resources、Limitsを含む解決済みProfileを生成する。仮のdigestや空のprovider結果を成功値にしない。

実装タスクを完了へ変更する際は、対応するopen findingを解消し、独立レビューを受ける。リポジトリ検査の成功だけでこの手順を省略しない。

## 外部仕様との照合

JSON objectは順序を持つデータ構造として規定されておらず、arrayは順序を保持する。R001の対応根拠は [RFC 8259 §4–5](https://www.rfc-editor.org/rfc/rfc8259.html#section-4)。

mspaceの寸法属性はCSSのlength-percentageを使う。R005の対応根拠は [MathML Core §3.2.5](https://www.w3.org/TR/mathml-core/#space-mspace)。旧MathMLの長さの扱いをMathML Coreへ流用しない。

XML 1.0は許可する文字集合と文字データ中の]]>を制限する。R008の対応根拠は [XML 1.0 §2.2](https://www.w3.org/TR/xml/#charsets) と [§2.4](https://www.w3.org/TR/xml/#syntax)。UTF-8として妥当であるだけではXMLとして妥当とは限らない。

## 検証の限界

`d743c0b` から始めた次のcheckpointでは、Report共通validationとtyped codecの管理対象2件を独立成功確認した後、R036を追加で再現した。既存の有効なFixのexpected_digestだけを元sourceの該当範囲と異なる値へ変更しても、Report.validateとencode_report/decode_reportが成功する。同じ非空範囲[2,3)に異なるreplacementを置く2編集も同様に受理された。spec02のFix前提digestと非重複編集の不変条件が共通validatorに無いためである。診断内容のdomain上の真偽の証明ではなく、明示sourceに対して適用不能な修正候補を外部境界で拒否する契約として実装担当へ戻した。補助probeは `.tmp/independent-core/src/bin/report_boundary.rs` であり、管理対象回帰を追加後に再検証する。

R037は統括のallocator再現を独立再実行して確認した。`.tmp/root-edit-allocation.exe` は、1編集・100000 bytesのSourceId・AllocationUnits上限0で実SourceStore.applyを呼び、AllocationLimitを返す前に100008 bytesを割り当てる。報告AllocationUnitsとWorkはともに0だった。同じ測定器の4096 bytes正例とSourceMapの上限0/実割当0も確認した。source.rsには予算検査前のedit参照Vec生成とsource identity cloneがあり、R020のSourceMap修正範囲とは別の未検査経路である。入力setup・結果cleanupは計測外で、coreへunsafeを追加した計測ではない。管理対象allocator回帰への追加と、snapshot更新をatomicに保つ修正を要求した。

R036の元2反例は修正後にExpectedDigest/OverlappingEditsで拒否され、管理対象report 3件も独立成功した。続く適用前提の確認では、同じSourceIdのrevision 0と1に対する2編集を一つのFixに入れるとReport.validateは成功するが、SourceStore.applyはSnapshotMismatchになることを再現した。双方のsource宣言と区間digestが正しくても、同じsourceを同時に2つのbase revisionから更新するatomic transactionにはならない。Fixの編集だけにSourceIdごと一つの前提snapshotを要求し、related spanの履歴参照や別sourceへの編集は許可する追加修正を依頼した。

R037の同じapply入口では、別setupで構築した既存source `abc` を全削除する編集が、fresh SourceLimit 0でも成功しrevision 1を挿入することも独立再現した。Usage.sourceBytesとAllocationUnitsはいずれも0だった。出力が空でも元の3 bytesは操作が読むsnapshotであり、変更後sourceとは別に一度受け入れる必要がある。補助 `edit_source_limit` の実行を根拠として、共有SourceAdmissionを使う元/生成snapshot計上をR037の修正範囲へ追加した。

R036の追加修正後、同一SourceIdの2 revisionを持つ元Fix probeはSnapshotMismatchで拒否された。管理対象report 4件を独立実行し、元のdigest/重複編集の拒否、schema自体は正しいNDF改変からのdecode拒否、順不同の隣接編集、挿入境界、異なるSourceIdへの同時編集、related spanの履歴参照を確認した。codeはFixの編集だけを同一前提snapshotへ制限し、診断全体の履歴参照を禁止しないため、R036をcorrectedとした。SourceStore.applyの資源計上漏れR037は別のopen課題として残す。

次の性能変更では、`ParseSession::read_with_host` の管理対象native_host 5件を独立実行し成功した。明示hostが実provider catalogの要求を照合し、同期応答も既存reader/tokenizerの再開検査を通すことをコードで確認した。Noneまたは通常callback失敗はowned Await/Reserveへ戻り、停止時は正式diagnostic・event・source/map閉包を保持する。dynamic head providerの実装や権限隔離の完成とは扱わない。

独立の補助 `native_host_boundary` では、caller深さ7で `let "x\n" y tail` を読み、Text予約とproviderが交互に現れる経路を検査した。全同期、2回目None、2回目通常失敗の各結果がowned経路とtree・source/map・diagnostic・depthで一致した。decoded Text受理後の取消はdiagnostic 1・source 1・map 2を保持した。Allocation上限100000から2000000を32768刻みで振ると、Text受理後の停止6ケースと完了51ケースを確認し、停止でsource/map閉包が欠落せず、使用量が上限を超えないことを検査した。この補助probeはignoredであり、同じ交互経路と取消を扱う管理対象 `native_host_mixed_text_provider_fallback_preserves_decoded_sources` の独立成功と区別して記録する。

変更後の通常bootstrap試験もログ保存付きで独立再実行し、完全な原GrammarからP0/P1/P2のsemantic identity一致を確認した。`cargo test --locked -p nepl3-tools complete_grammar_bootstrap -- --nocapture` は必須の通常試験1件が成功した。任意のowned性能比較1件はignoreのままであり、この実行で比較試験も通したとはしない。今回のnative debug実行は25.29秒、Work 8,073,006,166、AllocationUnits 10,333,383,050、Depth 96、AST 518 nodesだった。実行時間は並行処理の影響を含む単回観測で、releaseの速度比を独立証明する値ではない。累積コピー計上量は以前のbootstrapより減ったが、標準予算での完了や内部コピーの全解消、全targetでの性能要件達成は未検証のままである。

この25.29秒の計測対象は共用runtimeの変更中treeであり、R037のadmission比較Work計上も含む。native host差分だけを隔離したcheckpointのsource identityや速度比較と同一視しない。

統括の報告では、Reportだけを保存したcheckpoint 267c531のCI 34070506992は、3 OSともP2 byte 9223でWork上限10Bに達して失敗した。このremote結果は統括による確認であり、本レビューのnativeローカル成功へ置き換えない。今回の同期host経路は通常bootstrapの上限10BとCIでの実行を維持し、owned比較だけを任意の追加測定とする。改善後のremote CIは再検証待ちで、owned経路の高いコピー費用は未解決である。

R037の修正後、管理対象edit 5件と `python tools/audit/allocation/run.py` を独立実行し成功した。実allocatorは4096 bytesの正例に反応し、AllocationUnits 0のSourceStore.applyは実割当0 bytesで停止した。元の全削除probeもSourceLimitで拒否され、元storeのrevision 0が維持された。borrowed参照の並替え・snapshot/locator比較・出力準備の前に資源を計上し、全出力identityを検査してから生成admissionとstoreを一緒に確定する実コードを確認した。

追加の独立 `edit_transaction` は、順不同の隣接Unicode編集、末尾挿入、複数SourceIdの返却順、入力と生成snapshotのSourceBytes 25を検査して成功した。同じ操作で古いrevisionと重複anchorを拒否した後も全storeが不変で、訂正した編集は成功する。初回補助sourceはPowerShell pipeで日本語が `?` へ変わり、fixture構築時のBoundsで失敗したため、Rust Unicode escapeで元の意図を保持して再実行した。この補助fixtureの失敗を実装不具合とは扱わない。管理対象の全allocation上限走査、失敗後retry、長ID/URI、生成identity衝突の回帰と併せてR037をcorrectedとした。

次のT05実装入力として、短いGrammar sourceからsource位置を失うcompile失敗を3件確認した。日本語commentとCRLFを含む有効な対照sourceはcompileに成功する。`ref absent` はMissingRule、`scalar range "z" "a"` はInvalidRange、Seq readerの`List<NdfValue>`をleafのreference selfで名前として扱うsourceはPackage(InvalidBinding)を返す。いずれも実ASTにはLocatedのSpanがあるが、返却errorには該当NodeIdやSpanがない。

補助fixtureの期待位置は、未定義名absentが[88,94)、逆転rangeのhiが[101,104)とrelated lo[97,100)、bindingのselfが[227,231)とrelated reader式[84,108)である。これは後から原文を検索して診断位置を捏造する提案ではなく、既に保持するASTの位置をtyped error/Diagnosticへ渡すための独立期待値である。実行はbootstrap::load・Document.validate・compile::package::compileを使った `.tmp/independent-engine/src/bin/compiler_diagnostics.rs` と `.tmp/compiler-diagnostics-review.log` に記録した。初回range fixtureのNat引数は正式文法がTextを要求するため訂正して再実行した。途中で別担当のFacts adapter接続により再buildが未完となったため、成功build時の補助executableを用いて3例を実行した。未完成adapterのcompile失敗を製品不具合とは扱わない。この診断位置の未達はT05の要件であり、binding例の型不一致R033の解消とは別である。

Reader/Tokenizerのnested continuationコピー削減は、reader runtime 31件を独立実行した。外側TokenizationContinuationの全量echo照合は残り、成功後にprivate pending slotから取り出した内側continuationだけを唯一のresume_from_tokenizer呼出しへ渡すことを確認した。公開ReaderSession.resumeの全量照合は維持される。管理対象9種類に加え、補助 `nested_continuation` では内側の環境digest・report.traceOverflow・frame.phaseも変え、全12偽装が拒否された。元echoで正常再開し、二重再開を拒否した後に同sessionを別操作で再利用できた。生成source上の正式diagnostic/eventを持つ停止と、Await構築中のallocation faultで両pending slotを残さない既存回帰も成功した。性能比較やFacts全包絡を含む全workspace検査は、この境界確認とは別に行う。

最終の1/4/8段prefix比較を独立再実行した。owned/nativeのWorkは11068/8510、48832/34727、135556/91663、AllocationUnitsは244688/150447、971108/542043、2589212/1352585で、各段のoutcome・診断の一致と費用の大小を確認した。これらは当該fixtureの観測値であり、全入力で一定比率の高速化を保証するものではない。

releaseは、実装担当が最後の同source buildと確認したtest executable（SHA-256 `39da4aaa9ef437d7f5dca11d955a4e724b581ba98169183ba27af07502c38edd`）を独立に再実行した。通常の非ignore bootstrapをexact指定し、上限10BのままP0/P1/P2の意味一致・518 nodes、Work 7,287,412,952、AllocationUnits 8,045,061,614、Depth 96で成功した。3.30秒はこの単回native実行の観測で、以前の別source/buildやowned経路との速度比には使わない。ログは `.tmp/review-nested-release.log`。実装担当の追加owned/native比較ログ `.tmp/continuation-copy-after.log` も読み、owned側は上限20BでWork 10,597,852,458を要することを確認したが、そのowned比較を独立再実行したとは記録しない。

追加profilingは実値のcharge_cloneを独立Budgetで計測し、その内訳は重なるためoperation Usageへ足さない。通常bootstrapのCI実行・10B上限は維持し、追加owned比較だけをignoreの任意測定とする構成を確認した。動的head protocol、全Facts包絡、内部コピーの全解消はこの性能sliceの承認範囲に含めない。

ParseTreeのportable下層では、管理対象2件をruntimeと隔離したNEPL3-prefix-validationの両方で独立実行し、既存wire syntax 6件も成功した。host root 1とguest root 2を別々に0へ変換し、各context・ForeignStep・RecoveryEntryのNodeRefが同じ局所対応表に従うこと、Environment id 9とToken/View/Originの別tableをnode番号へ変換しないことを確認した。受信時は構造schema・宣言source・解決済みProfileの静的選択を検査し、context/recoveryの正準順も検査する。元source 15 bytesのonce計上と、encode/decodeの資源上限0・取消による停止理由も管理対象に含まれる。

追加の独立 `portable_tree` は、schema検査が成功するNDFからcontext順・selection順・局所node重複・profile digest・execution digest・guest source宣言を個別に変更して、typed decodeがそれぞれ拒否することを確認した。guest sourceをambient storeに残しても、そのbundle内の宣言不足は補われない。さらに、正しくencodeできるhostのSpanをguestのMissing recoveryへ注入した値も、guestに属さないsourceとして拒否された。正常値はdecode後の再encodeで同じ正準値となる。この7種類の補助変異はignoredであり、管理対象試験の実行と区別する。core NodeMapping単独は参照範囲と到達順の計算で、schema/source/cycle/選択の検査済みproofではない。Dynamicは引き続き明示拒否し、FactsRequest/Reply全包絡やparse continuationのportable化は今回の下層codec成立から推定しない。

Facts操作包絡の次段では、管理対象 `cargo test --locked -p nepl3-engine --test package portable::facts --target-dir .tmp/review-target` の2件を独立実行した。host/guestのcanonical path/nodeとopaque ID、元要求全体の7種類の改変拒否、Complete・Invalid/Stoppedのpartial有無による全5返信形、診断専用source/map、guest上のrelated/Fix、Source/Work/Depth/Allocation各0と取消のencode/decode計50境界を確認した。Reportの参照をambient storeだけで補う入力と、予約外Entity IDも拒否する。これはnative値とNDF包絡の検査であり、別processの通信認証や名前解決アルゴリズムの実行証拠ではない。

独立補助 `facts_envelopes` は実返信の非空mapping列を旧field型 `List<SourceMap>` と訂正型 `List<SourceMapping>` にそれぞれ渡し、前者の型拒否と後者の受理を全5形で確認した。旧engine schema全体を現digestのまま再現したとは扱わない。空列だけの検査では発見できない包絡fieldの不整合であり、04章・正本・生成descriptorの訂正理由を確認した。同じ補助はpartialを含む3形でschema-validな予約外Entity101を注入し、元host authorityによる拒否と50停止境界のsticky状態も確認した。

この補助で、同じ返信source表に同一snapshotを2回置くとnative `FactsReply.validate` が成功する一方、portable encodeがIdentityConflictで拒否するR038を実測した。修正後は元native/encodeの両入口がIdentityConflictを返し、全5形の合法NDFへ同じ重複を加えたdecodeも拒否された。独立表間の正当なsource共有と一度だけの計上は維持される。管理対象2件の再成功と実コードの先行Work計上を確認してR038をcorrectedとした。ignored補助の結果を管理対象回帰の数へ加えない。

要求の `request_from_value` は保存済み元要求のproofと全canonical値を照合するsender側のecho/loopback境界であり、元Rust要求をまだ持たない外部providerの初回decodeには使えないことも確認した。その後、型・参照整合の検査だけを行いraw FactsRequestを返す `request_decode` が追加された。元要求・source store・codec・admissionをscope破棄した後、selected profile/registryと空storeだけを持つ受信側で復元する管理対象が成功した。正しい参照を持つ異なるgrantのraw decodeは許容するが、認証・実行授権を与えたことにはならない。issueを呼べるhostの信頼境界、元proofとの完全一致を要求するecho境界を04章とrustdocで分けたことを確認した。

最終Facts snapshotの `C:/projects/NEPL3-prefix-validation` で管理対象3件を独立再実行して成功した。同snapshotへリンクした補助は、新しい初回decodeの5停止理由とsticky状態、schema-validなtarget NodeRef999/current ScopeId999の拒否、欠落したtree sourceをambientに残しても拒否することを追加確認した。補助のsource表を変更する最初の試行ではParseTreeのfield位置を誤ってrecord取得に失敗したため、公開schemaのbundle位置へ訂正して再実行した。元の返信5形・50停止・予約外delta・R038の負例も同実行で成功した。型付きFacts包絡のこの範囲をレビュー済みとするが、transport認証、callback実行、語彙scope解決、Dynamic tree、別process/provider比較の完成は含めない。Head向けDiagnosticSourceResolver変更はこのsnapshotから分離されている。

次のHeadProvider初稿は、profile登録1件、投影unit2件と公開ParseSessionのdynamic入力1件を独立実行した。明示alias/categoryごとのproviderをprofile identityへ含め、同じpackageの別aliasが異なるoperationを選択できる。補助 `head_profile` でこの差分と、languages/登録順交換によるidentity不変を確認した。既読tokenのcompound NdfValueも投影へ保持するが、その内部の値をsource lookupやenvironment権限として自動解釈しない。SourceWindowは全snapshotを公開せず、元位置の範囲だけを持つ。補助 `head_window` は日本語・補助平面文字・CRLF、別ID/revision/digest・範囲外・scalar途中・不正UTF8/長さ/空ID、fresh Source/Work上限を確認した。部分windowの形検査をsnapshot全体のdigest認証とは扱わない。

最初のdynamic管理対象は `choose alt @let z x tail` を実reader/parserへ渡し、arity2のshape、compound selector読了後のBody/Alt選択、内側letの子でCodeへ戻ること、cursor19でtailを未読に保つことを検査する。補助 `head_runtime` ではcall/session/execution/operation identity改変、fresh cancelled Budget、不正な未読tail位置のProjectedReportを拒否した後、元pendingを再開できた。正式diagnostic1件を受け入れた次のAwaitで取消してもdiagnostic/Usage各1が残る。caller深さ7で開始し深さ0からHeadを再開した経路のpeakは基準7から14へ増え、外側static parentを持つ `let n choose alt @let z x tail` はcursor25へ復帰した。これらの補助はignoredであり、管理対象数に含めない。深いFailed返信、foreign/生成sourceの投影・復元、全停止境界、portable Head操作は引き続き検証範囲として残す。

Head管理対象が6件になった段階で全件を独立実行し、native/owned/fallback比較、Foreign selector、Failed childの固定arity保持、取消と長いFailed codeの先行予算検査を確認した。補助の100段Someを含むFailed.argumentsはDepthLimitで停止し、既受理diagnostic/event各1を保持する。窓内spanを持つがexpectedDigestが違うProjectedFixも拒否後に元pendingを再開できた。別の補助は固定Foreign/NodeRef slotsを使い、Guest Selectorの全windowを[7,10)へ限定してHost Body/Altへ復帰する。最初の補助の結果検査ではcontext配列先頭をhostと誤認したため、empty pathでownerを選ぶよう訂正した。

その後の `head_runtime` scenario8でR039を確認した。正しい窓内diagnosticを返す際、caller storeだけ同SourceId/revision/digest/bytes・別URIへ変えるとresumeはIdentityConflictで拒否するが、元storeで同じecho/replyを再試行するとNoPendingとなる。保存windowとのbytes一致を検査した後にslotを取り出し、collectorへのappendでsource admissionがURI不一致を発見する順序が原因である。型付き拒否だけでは保持済みcollectorと再試行可能性を満たさないため、修正と独立再検証までR039をopenにした。

appendをslot取得前へ移した途中修正では、空Reportを返す2回目のChildContextに別URIを渡す入力は拒否後に再開できた。一方、同じ入口へ空SourceStoreを渡すと後続driveでMissingSnapshotになり、元storeでの再試行は依然NoPendingとなった。`head_sources` はこの2結果を個別に出力する検証器であり、初回の終了0を両例の成功とは扱わない。主parse入力をcaller storeから読み続ける現在のAPIに合わせ、存在・内容・URIの前提をslot取得前に検査する必要がある。既に保存済みの生成sourceや環境sourceを一律にcallerへ再登録させる制限を、この修正から推定しない。

主入力guardの追加後、元のURI不一致と空storeの2例は拒否後に元storeで再開し、既受理diagnostic/event/source各1を保持した。ただし別の `head_aux` は保存CheckedReaderContextにauxiliary sourceのOriginを持たせ、2回目のHead Awaitへ主入力だけのstoreを渡すとContext、完全storeで再試行するとNoPendingになった。後段のContext.retargetがcaller storeだけから保存auxを解決するためである。保存済みauxの一律再登録を求めない契約と合わせ、R039はこの経路の修正・再検証までopenを維持する。

R040は、正常Head Shape返信へtraceOverflow(dropped=1)だけを追加した `head_overflow` がCompleteかつoverflow付きで終了する実再現から記録した。さらに `reader_overflow` は実ReaderSessionのAwaitを経て、Read Matched、Read Failed、Map TransformReply、Decode TransformReplyの4種を独立に検査し、すべて非Stoppedのままoverflowを保持して返した。実行は `cargo run --manifest-path .tmp/independent-engine/Cargo.toml --bin reader_overflow --target-dir .tmp/review-target`。この補助はignoredであり管理対象の実行数へ含めない。spec02のevent上限契約はoverflow時の停止を要求するため、Report単独の形検査とは別に各操作outcomeの整合を検査する必要がある。Stopped(Cancelled/WorkLimit等)の終了処理で生じたoverflowまで禁じる縮小は行わない。Head/Factsの修正が進んだ時点でもReaderの4経路が残るため、R040全体の解消はまだ記録しない。

R039の最終修正は `retarget_preserving_sources` により保存checked contextの借用source閉包を維持し、選択registryを再検査する。元の `head_aux` は主入力だけのcaller storeで正常に継続し、最終tree.validateまで成功するようになった。`head_runtime` scenario8、`head_sources` scenario9/10も再実行して、URI不一致・主入力欠落の拒否後に元返信で再開できた。管理対象12件はaux URIの明示衝突後の正常再開、元storeへauxの再登録を要求しない完走、取消/Failed/固定arity/compound selector/foreign復帰を含め成功した。R039をこの再現範囲でcorrectedとした。

R040の修正後、独立Reader4候補はすべてProviderContractを返す。`provider_overflow_rejects_read_failed_map_decode_without_consuming_slot` は4種類それぞれの元返信再開と二重再開拒否、Stopped(Cancelled/WorkLimit)のoverflow保持まで独立成功した。外側tokenizerの既存echo回帰にも同じ負例が加わり成功した。Headの管理対象12件にはShape/ChildContext/Failedのoverflow拒否後の完走とStopped(Cancelled)受理が含まれる。Facts管理対象3件は5返信形についてnative/encode/合法schemaのNDF decodeを照合し、非Stoppedだけを拒否する。Reader/Tokenizer/ParseSessionのslot取得前にO(1)のoutcome検査を配置したコードを確認した。これらを根拠にR040をcorrectedとしたが、未実装Reader reply transportの検証まで含めない。

後続Head portable差分は管理対象4件を独立実行して成功した。初回受信の自己完結したwindow、compound payload、schema-valid改変、返信全枝の位置/Fix/outcome検査、private delegationのwindow課金と相対費用合流が対象である。読取レビューではgraphのobserve_depthだけでは内部payloadのactive depthへ加算されない点を指摘し、実装担当がnode深さでの検査へ訂正した。旧版の独立実再現前の修正なので新しい確定finding数へ加えていない。追加の共有node回帰は浅い経路で初回検査したpayloadを深い経路でも再検査し、限界27拒否・28成功を確認する。このportable差分はnative Head checkpointと別のレビュー単位であり、別process callbackや全protocolの完成宣言ではない。

後続portableの共有予算を調べ、R041を確認した。`IssuedHeadDelegation::issue` は親の残余Workを全額子limitsへ与えた後も、親のto_value・実CBOR encodeが同じBudgetを使う。独立補助は親Work上限500000で発行・encodeし、子側で実CBOR decode・delegation_decode・validate_projectionを294回実行した。上限停止時の計上値は親8544＋子497629＝506173で、結果合流の前に共有上限を超えた。任意のUsageを返信へ書いた再現ではなく、公開処理の実行で生じた計上である。実行は `cargo test --manifest-path .tmp/independent-engine/Cargo.toml --bin head_portable independent_parent --target-dir .tmp/review-target -- --nocapture`。補助のtest成功は上限超過のassertが成立した意味で、productionの合格とはしない。事後acceptでStoppedへ変換しても先行実行済みの費用は戻らないため、親framingと子quotaの先行配分を閉じる修正を担当へ戻し、R041をopenとした。native Head checkpointにはこのportable初稿を含めていない。

R041修正草稿は発行proofが親Budgetを排他的に借用し、明示HeadTransportReserveを子quotaから先に差し引く。親のframingはtransport closure内で一時ceilingを適用し、Usageへ予約容量を混ぜない。新しい独立 `head_reserved` は元の親Work上限500000のままreserve20000を使い、同じ実codec/投影処理を上限まで実行した。親8544＋子477666＝486210となり共有上限内に収まった。settle後のUsageは実合計で、二重精算による増加はない。別の補助は親transportのWorkLimit後にも完了子の観測実費を記録して元StopReasonを維持し、未精算proofのDropで親をCancelledにすることを確認した。

coreの独立 `budget_reservation` はrecord_observed_usageの全8資源について上限超過を拒否しUsageを変更しないこと、既存Cancelled後の合法実費記録、with_ceilingの入れ子で上限を増やせないこと、既使用量を下回るceilingでclosureを実行せず元Limitsへ復帰することを確認した。これは入力許可を与えるAPIではなく、hostが実測済み費用を記録する入口である。修正後のportable管理対象4件も成功したが、この時点では新しい予約・精算の管理対象と実parser callback往復の追加確認が継続中のため、R041はまだopenとしている。

R041の最終管理対象を独立実行し、core Budget2件、Head portable6件、実ParseSession callback1件が成功した。元Work cap500000を維持した実CBOR/投影反復の上限検査、6加算資源それぞれのtransport停止後の精算、二重精算、未精算Dropを管理対象で確認した。callback試験は `choose alt @let z x tail` の4回Head要求を初回raw受信から同じanswerへ渡し、Deliveryの実CBOR往復後にresume_headする。子の確定観測を親の返信デコード前に精算する順序へ訂正し、各呼出しでschema-valid callId改変を拒否後、正しい返信でnativeと完全同じtree/cursor19へ到達した。Dynamic tree自身もportable正準再encodeが一致した。

別の `head_callback` 補助では不正返信の拒否理由をHeadError::Identityと明示して検査した。拒否に要したWorkは増える一方、diagnostics/events/SourceBytesは変化せず、その後同じpendingから正常に完走した。spec04の予約・相対費用・絶対depth・窓免除・未精算破棄・受信認証責務の同期も読み、R041をcorrectedとした。この結果は同process内の実NDF/CBOR交換とcallback実行の証拠であり、別processの通信認証・終了制御・メータリング実装や全P03受入条件の合格まで主張しない。

T05の位置付きcompiler診断は、独立した日本語・emoji commentとCRLFを含む原文で再検査した。MissingRuleは名前のbyte範囲104..110、逆転rangeは上限122..125をprimary・escaped下限113..121をrelated、Choice異型は後続scalar129..147をprimary・先行literal112..123をrelatedに保持する。Choiceの期待Unit・実際Textも一致した。通常DiagnosticをReportへ包み、実NDF/CBOR encode/decodeを通した独立補助 `compiler_report` は3例すべてで構造全Eqとなり、SourceBytesは原文198/213/239bytesの各1回、Diagnosticsは1だった。管理対象 `cargo test --locked -p nepl3-tools --test grammar diagnostic --target-dir .tmp/review-target` の6件も独立成功し、fresh Source/Allocation/Diagnostics停止・別Documentへの流用拒否・共有SourceAdmissionを確認した。元fixtureは子 `.gitattributes` の `*.neplg -text` によりCRLFを保持する設定を確認した。これを全T05や未実装binding挙動の合格とはせず、R033は別の未達として維持する。

TypeDescriptor encoderの管理対象1件に加え、独立main-thread補助 `type_descriptor_depth` は100000層ListをDepth1000で停止し安全に破棄できた。2048層のList/Optionと未知packageのUnicode Named参照は実CBOR往復と各層の照合が成功した。Namedは記号的な型記述なので、ここで未知参照を実行可能型として登録・承認した意味ではない。これらignored補助は管理対象試験数へ含めず、位置付き診断の実行境界の追加検証として記録する。

process起動不要の管理対象 `diagnostic::compiler_report_boundary_without_host_process` は、wasm32-wasip2へbuildした実tools試験artifactをWasmtime44.0.1で直接実行し1件成功した。初回cargo実行はrunner未設定により試験未実行だったため成功へ数えず、同じartifactの明示runner実行結果を根拠とする。これは管理下seed入力からcompile/Diagnostic/Reportへ至る経路であり、Python adapter自体をWASI上で動かした意味ではない。

R042は後続Reader修正と分離し、clean commit ab9c50b1d36ee0240d759cc80ef986f905242804の公開APIで確認した。実Read providerの出力型Textに対し、Await後のMatched.valueだけをUnitへ変更するとSchema(WrongType)となり、同じ保存continuationへ正常Textを再返信するとNoPendingになる。実行は `cargo run --manifest-path .tmp/reader-payload-old/Cargo.toml --target-dir .tmp/review-old-target`、依存先は読取専用の隔離snapshotである。補助の成功はこの旧欠陥assertの成立を示す。現Reader草稿のslot取得前検査では同じ正常再送がMatchedへ改善したが、最終回帰とcollector/source閉包の原子性は後続sliceで確認するためR042はopenとする。

後続sliceでR042の修正を再検査した。独立 `reader_retry` はSeq(Call,Call)の先行返信で生成source上の診断/eventを受理し、後段のvalue型・state型・end超過・複数sourceの後半URI衝突・範囲外ViewRefを個別に拒否させた。5件とも同じechoへの正常再送でMatchedとなり、正式source/diagnostic/eventは各1件だけ残る。途中で入場した候補sourceを正式列へ混ぜず、使用したWorkとadmissionは返却しない。管理対象 `retry::rejected_read_payload_state_end_source_and_view_preserve_formal_collector_and_pending` も成功した。

外側Tokenizerは内側Readerの非停止拒否時に元の所有checkpointを戻し、ParseSessionも同じ条件で待機を保持する。独立 `tokenizer_retry` は型不一致後の正常再送でTokenを返し、管理対象 `synchronous_head_reader_head_dispatch_matches_owned_interleaving` は実parserの3層を通してcompound型へのUnit返信を拒否後、同ParseContinuationで正常完走する。`cargo test --locked -p nepl3-reader --test runtime --target-dir .tmp/review-target` の34件も独立成功し、既存allocation停止sweep・正式Report/source保持・pending解放を含む。これらの範囲でR042をcorrectedとした。

TransformはMap/DecodeそれぞれのComplete/Failed/Stoppedをnative/NDFで往復し、取消前resumeを加えた24経路の管理対象を独立実行した。全枝で共通source/maps/Reportを持ち、Failed primaryは診断1件の再掲として扱う。独立 `transform_boundary` はschema-valid NDFのComplete.valueを宣言Textと違うUnitへ、Failed.primary.codeだけをReportと異なる値へ変えて拒否を確認した。一方Stopped理由WorkLimitへの変更とtraceOverflowの組合せは受理する。5種類のdecoder資源停止、未宣言sourceをadmissionやambient storeで補わない負例、取消時の既受理Report保持も管理対象で確認した。

外側OperationReplyはComplete/Invalid/Stoppedと内側TransformOutcomeを明示対応させる。独立 `transform_outer` は合法schemaのまま外側と内側のoutcomeまたはStopped理由を不一致にした6例を拒否し、diagnostic/event counterを増やさず正常packetへ戻れることを確認した。管理対象にはReport再掲の改変拒否とpartial=Noneのdispatch拒否往復もある。source/mapsを欠くpartial=Noneで新しい生成sourceの診断を持ち込めず、保存要求・正式checkpoint由来の閉包だけを返す。spec03/09の規範も照合した。このadapterはsenderの保存待機proofと同操作の観測済み累積Budgetへ結び付き、初回provider要求受信・remote計量の認証・全Read/Tokenizer継続codecまで提供したとは扱わない。

PackageSubjectの追加2試験は、既存checkerと同じ失敗・同じUsageを保った詳細帰属と、6種類のcategory/mode欠落の元UTF-8 byte位置を確認し独立成功した。独立 `package_locations` は日本語emoji/CRLF原文のcommentとliteralへ同じAbsentを先に置き、nested WithModeの内側LocalのAbsentだけをbyte255..261へ正しく帰属させた。位置は保持AST対応から得ており、同綴りのsubstring探索による代用ではない。

ReaderExpr matrixは正式Grammar原文を実seed入力adapterからtyped AST・production reader compiler・ReaderSessionへ通す管理対象4件がnativeで独立成功した。23constructorの値/end/捕捉・Region・Node、23種類の失敗時のOptional/Commit/実Transform Failed診断、23nonfinal NeedMoreを確認する。最初のnonfinal18例からScalar/Map/Decode/Thenと明示provider NeedMoreを追加するよう指摘し、最終23例を確認した。processを使わない3実行試験はWasmtime44.0.1でも独立成功した（`cargo test --locked -p nepl3-tools --test grammar reader:: --target wasm32-wasip2 --target-dir .tmp/review-target -- --skip reader_matrix_seed_matches_the_real_adapter --test-threads=1`、runner=`wasmtime run`）。実Python adapterとの一致はnativeの別試験で確認する。新CI2行は同じWASI runner配下でこの3試験を実行し、既存検査を維持する。有限matrixを全組合せの証明やT05全条件の完了とはしない。

T06の独立入力は実装の結果を読む前に原文とUTF-8 byte位置から期待する宣言・参照対応を固定した。補助 `binding_analyze` は実Grammar compiler・ParseSession・tree.validate・公開analyzeを24入力へ接続し、対応済み17入力のEntity.selectionへの解決が期待値と一致した。Let初期化子の非再帰、Lambda shadowingと兄弟へのscope復帰、同scopeの宣言前後・同名2宣言のAmbiguous、Import/Export、同aliasのForeign内外の分離と深さ3の独立root、escaped Text名の意味比較を含む。Textの名前が1文字でもselectionは元の引用token全byte範囲を保ち、SourceMapを失わない。Open policyの同入力も17解析が成功し、単独freeはEntityを作らずopen要求1件・診断0件となった。Sequential/Recursiveの7入力は明示UnsupportedPlanであり、この段階の解析成功には含めない。Global・Custom・明示foreign共有や全portable解析は未検証である。補助原文・期待値・実行器はignored `.tmp/binding-review.neplg`、`.tmp/binding-expectations.json`、`.tmp/independent-engine/src/bin/binding_analyze.rs` であり、管理対象試験の代用にはしない。

R043は同じ公開入口で確認した。`binding_stops` はText escapeとForeignの2入力を正常解析した後、fresh BudgetのWork/Allocation/Nodes/Depth/SourceBytesを個別に走査する。Text入力でSourceBytes=0はInvalid(Tree(Syntax(Source(Stopped(SourceLimit)))))、Nodes=5はInvalid(Tree(Syntax(View(Schema(Stopped(NodeLimit))))))、Work=51はSource由来のWorkLimitとなる。Budget.pollは対応する停止理由を保持しており、型付き入力不正への分類が誤っている。2入力の走査で102例を再現したためopenとして修正を依頼した。返却されたSome(partial FactSet)を別fresh validatorで検査した範囲では構造不正はなく、Occurrenceとstageの片側だけの公開もなかった。この構造検査成功をStopped/Invalid分類の成功とは扱わない。実行コマンドは `cargo run --manifest-path .tmp/independent-engine/Cargo.toml --features nepl3-tools,nepl3-grammar-core --bin binding_stops --target-dir .tmp/review-target` である。

R033の元の解消条件も再検査した。正式binding.neplgは明示的なname-v1呼出しからTextを得る。旧Seq/List版を保存した `binding-nontext-name.neplg` は実package compilerで今もInvalidBindingとなり、管理対象 `full_example_assembly_reaches_actual_plan_and_binding_validation` が独立成功した。訂正済み正式原文を実compile・ParseSession・標準provider・analyzeへ通す `binding::official_binding_let_lambda_visibility_and_entity_identity` も独立成功し、Let/Lambda/兄弟復帰/Unicodeの参照先を原位置に照合しFactSet NDF往復を確認した。暗黙Seq-to-Text連結を加えず、元の型不一致と要求したbinding挙動が是正された範囲でR033をcorrectedとする。T06の全完成を示す変更ではない。

Sequential/Recursive実装の追加後、同じ固定期待値の独立24解析はすべて成功した。ただし新たにbody ReadSpecをforeign B Exprとした合法Grammarでは、`sequence cons define x 1 nil x` と `recursive cons define x 1 nil x` が実compile・parse・tree.validateを通過後にInvalid(Fact(MissingNamespace))となった。通常Childとは異なりDeclaration actionが親stageを引継ぎ、guestのnamespaceとhost配下のscopeを組み合わせる。返却Some FactSetもnamespace-root所属を満たさない。既定Foreign分離の期待値はguest x未解決と別root維持であり、暗黙共有ではない。`.tmp/independent-engine/src/bin/binding_foreign_body.rs` と `.tmp/binding-review-foreign-body.neplg` の2実入力をR044として修正へ戻した。

R043修正後の最初の独立走査では102例から3例へ減ったが、Text Nodes11・Foreign Nodes18/19のSchema由来停止が残ったため担当へ戻した。Tree内の明示wrapも正規化経路へ訂正後、同じ2入力・5資源の走査は誤分類0となり、各Stoppedの理由が制限資源と一致することを追加assertして成功した。管理対象coreの9停止理由・nested変換・普通のWrongType/Bounds維持試験と、公開解析の6資源停止/Report閉包/Occurrence-stage同時保存/caller depth復帰を含むbinding5試験も独立成功した。R044の元2入力もchild_frame共通化後にComplete・guest未解決・root2・FactSet構造成功となり、同alias入れ子を加えた管理対象回帰が独立成功した。この具体的な再現範囲でR043/R044をcorrectedとする。

Binding portableの管理対象3件も独立成功した。完成した実解析結果をsenderの値から実CBORへ変換し、原source storeを持たないreceiverで復元する。再encodeの一致、共有SourceBytesの1回入場、stage cycle・Occurrence対応欠落・宣言source欠落、型付きInvalid/Stoppedとdecoder制限を検査する。独立補助 `binding_partial_wire` ではText/Foreignの各低予算結果を、初期facts=Noneだけでなく途中facts=Someを含め実NDF/CBOR往復し、Reportと再encodeが全て一致した。別の `binding_source_wire` はsource表の同identity別URI・同表重複をschema-validなまま加えた4例でIdentityConflict拒否を確認した。rawのDecodedBindingReplyは意味的なBindingAnalysis proofを生成しない。

失敗原因をTree/Profile等の大分類だけへ縮約する草稿は、nativeの全typed原因を保持するcodecへ訂正された。独立補助の静的照合では11 Rust error enumの151variantについてJSON正本と名前集合・payload型順が一致した。既存SyntaxErrorのView(ViewError)枝欠落も補われた。管理対象は実variant pathとSourceError::DecodeのvalidUpTo/errorLen順を別にassertし、全unit/nested原因および全9StopReasonの各層を実CBOR往復する。単独failure値は原原因を保持し、Binding Invalid包絡へ直接またはnested Stoppedを入れた独立2例はShape拒否だった。これはR006の具体的な型閉包の前進であり、全公開操作の契約が閉じたとは扱わない。

追加の管理対象を含む `cargo test --locked -p nepl3-tools --test grammar binding:: --target-dir .tmp/review-target` は11件が独立成功した。繰り返し実行したrecursive planは同じ宣言Spanでも別の実行scopeで別Entityを持ち、各回のbodyは自分のheader Entityを再利用する。同意味でも別execution配置のProfileは拒否される。Globalの実解析はまだUnsupportedGlobalNamespace診断とUnsupportedPlanで明示拒否され、lexical成功で代用しない。Custom facts provider実行・明示root共有・全T06受入条件は未達として維持する。

最終追加のOrigin前方参照回帰もnativeで独立成功し、binding管理対象の独立native確認は計12件となった。実treeへ合法なComposite→後方配置Directを加え、公開validate/analyzeのWork/Allocation走査で公開originsが空または全graphのいずれかであること、停止前後のSome FactSetが構造検査を通ることを確かめる。Wasmtimeの独立実行は11件すべて成功した（`cargo test --locked -p nepl3-tools --test grammar binding:: --target wasm32-wasip2 --target-dir .tmp/review-target -- --skip binding_seed_artifacts_match_original_source_and_host_adapter --test-threads=1`、runner=`wasmtime run`）。Pythonを起動するseed原文一致だけはnative必須としてcfgで区別され、最終CIはWASIでbinding::全対象を実行する。通常の解析・停止・portable試験を任意実行へ外してはいない。

次のGlobal実装は、先に固定した15原文と追加3入力を独立 `binding_global` で実compile・ParseSession・analyzeへ通して確認した。成功13例は原文位置で識別したEntityへ解決し、重複5例はDuplicateGlobalとprimary/relatedを保持する。先行bindはscope復帰後の兄弟にも見え、先行referenceへ後続bindは見えない。export候補だけでは公開せず、import後に可視となる。sequentialの順序とrecursiveの明示header収集を区別し、Foreignの同名宣言は別rootへ留まる。escaped Unicode名の重複は元綴りが異なっても意味名で検出し、primary27..32・related8..18を保持した。この例は管理対象にも追加され、その試験を独立再実行した。

Occurrence.scopeと既存stageは実発行のlexical位置を保ち、追加namespaceStageがGlobal rootの発行時履歴を捕捉する。Global Entityの配置先だけをrootとし、語彙位置をrootへ上書きしない。独立補助は各referenceについて保存namespaceStageから過去のintroduced列をたどり、実際の解決候補と一致することもassertした。18結果すべてを実NDF/CBOR往復し、NamespacePolicy・Entity・両stage・Report・再encodeの一致を確認した。`binding_global_stops` は重複入力を6資源で制限し、各partial resultをportable往復して、元StopReason・構造閉包・Report保持を確認した。

Global追加後のbinding管理対象15件は独立native成功し、process起動不要の14件もWASIで独立成功した（`cargo test --locked -p nepl3-tools --test grammar binding:: --target wasm32-wasip2 --target-dir .tmp/review-target -- --test-threads=1`、runner=`wasmtime run`）。portableの追加負例はnamespaceStageを語彙stageと取り違える場合、policyに合わないroot、範囲外IDをschema-valid NDFとして拒否する。共通codecは構造とroot所属を検査し、発行時点の履歴であることや導入権限を受信値だけで証明しないというspec04の区別も照合した。Globalへの従来のUnsupportedPlanはこの実装範囲で解除されたが、Custom facts callback・明示root共有・AnalysisKey/queryおよびT06全条件は引き続き別の未完範囲である。

AnalysisKeyの次段レビューでは公開prepare・execute・request codec・for_sourceを独立入力 `let x x x` と `let y y y` で実行した。source ID/revisionが同じでも内容変更はtree digestを変え、4種類のkey digest差替え・旧source revision・異なる実行Limitsは拒否された。元requestやsource storeを渡さず実CBORから復元・再準備・再実行したFactSetはnativeと一致した。さらにschema-valid NDFで要求ID・Work条件・tokenのText payloadを単独改変すると、source bytes不変のpayload変更も元keyとのRequestMismatchとなった。

独立GrammarのForm arenaの先頭と末尾を実交換すると、package意味identityとProfile digestは不変だが、旧treeはExecutionIdentityで拒否される。selection indexとexecution digestを正しく再配置したtreeでは、実analyzeのFactSetが元と一致し、AnalysisKeyのexecution digestが変わり旧結果へのアクセスは拒否された。補助は `.tmp/build_analysis_execution_probe.py` と `.tmp/independent-engine/src/bin/binding_key_execution.rs`、実行は `cargo run --manifest-path .tmp/independent-engine/Cargo.toml --features nepl3-tools,nepl3-grammar-core --bin binding_key_execution --target-dir .tmp/review-target`。これらignored補助は管理対象回帰の代用ではない。現for_sourceはidentity照合によるnative解析proofへのアクセス入口であり、definition/references/rename queryの正しさをまだ示していない。Custom callbackの実analyze経路もこの時点の実行範囲外である。

同じ確認をForeign入力 `lambda x guest x` へ広げると、nativeと初回receiverのAnalysisKeyは完全一致するがFactSetのID対応が異なった。nativeのhost定義はNamespaceRef(1)/OriginId(2)、receiverではNamespaceRef(0)/OriginId(1)となる。nativeのcontext path長は `[1,0]`、wire正準化後は `[0,1]` であり、binding.prepareがその入力列順にroot・namespace・Origin統合順を割り当てることが原因である。名前解決のhost/guest分離自体は保たれるが、同keyの結果に異なるID対応を生むためR045としてopenで記録した。元補助 `.tmp/build_analysis_guest_probe.py` / `.tmp/independent-engine/src/bin/binding_key_guest.rs` はこの相違をexit 1で再現する。guest局所のsource宣言と環境resourceを個別追加するとtree digestが変わり古いkeyを拒否する点は同じ補助で成功しており、この成功とID再現を分ける。修正後は元入力、context/selection逆順、SourceMapとOriginの保持を検証する。

R045修正は共通binding.prepareに借用BundleMappingsを接続し、host先頭のforeign DFS、正準node順位でのnamespace初登場、借用source identity順を用いる。全treeをNDFへ複製する修正ではなく、通常unkeyed analyzeも同じ経路を通る。元guest補助を変更せず再実行して2入力のnative/初回receiver FactSet全一致を確認した。追加補助 `binding_key_permutation` は `lambda x guest lettext "\u{78}" x` とy版についてcontext・selection・source表を個別反転し、同keyかつ直接analyzeの全BindingResult fields、Origin、SourceMap、stage、正式source/diagnostic/event内容の一致を確認した。source-only逆順のText入力2件も成功した。Originとmappingの宣言順は契約どおり保存する。最新管理対象AnalysisKey4件はnative/WASIで独立成功した。元guest・Text・反転表の一致に加え、Complete/Invalid DuplicateGlobal/Stopped Cancelledのkey付き返信と全BindingAccessError原因の実CBOR往復を確認し、この具体範囲でR045をcorrectedとした。借用FactsRequestViewのmanaged1件も独立成功したが、空deltaによる既存検査入口の比較であり、Custom callback実行や定義・参照queryの完成証拠には含めない。

次のCustomレビューでは、独立Grammarの原文を実compiler・ParseSession・analyze_with_hostへ通した。疎なEntity ID 100の受理後に通常bindが101を割り当て、MAX-1の受理後はMAXを使い、さらに必要になったIDはFact(Reservation)で拒否する。どの部分FactSetも構造検査を通り、入力によるpanicはなかった。Globalでは以前の同名Entityとの重複をDuplicateGlobalで拒否した。追加Entityのdefinition/selectionがともにNoneでも、primaryを捏造せず、実在する以前の位置だけをrelatedへ返した。補助は `.tmp/build_custom_review_probe.py`、`build_custom_sourceless_probe.py` と対応する `custom_review`、`custom_sourceless` の実行である。これらはGit管理外の独立測定であり、管理対象回帰の代用にはしない。

Open原文 `early x z custom x x` では、明示grantを持つCustom更新が過去のxをEntity100へ解決し、元OccurrenceStageとnamespaceStageの0を変更せずresolutionHistoryを追加した。grantを除くと元Unresolvedを保持してFact(Authority)となり、既受理eventは残る。更新後のopenInputsからそのOccurrenceは除かれる。全history・FactSet・stageを実CBORで初回受信し、schema-valid NDFの最終resolution不一致、grant欠落、重複transition、未存在Occurrenceを拒否した。raw履歴codecは型・chain・最終事実との一致を検査するが、元要求なしでprovider実行や権限発行の真正性を証明しない。

独立 `custom_stop` は289予算条件中68停止を得て、そのうち24件は診断・event受理後だった。生成sourceとExact mapを正式emitterへ加えた `custom_emitter` は同じ289条件で71停止、受理後停止24件となった。後者では受理済みsource2件・map1件・診断・eventと原SourceBytes一度分を保持し、全結果を実CBOR往復し、全partial FactSetを検査した。元StopReason、stickyなBudget.poll、消費Usageが一致する。停止後の未検査raw deltaは正式事実へ混ぜない。管理対象Custom9件も独立nativeで成功し、Foreign callbackの専用namespace/root、同batch Global重複、delta source上の重複診断、history更新、停止時閉包を確認した。Recursiveのheader段階でCustomに出会う経路はUnsupportedPlanでcallbackを呼ばず、現段階の明示的未対応として残す。外部processの費用認証・quota貸出も、このnative callbackとraw codecの成功から推定しない。

Definition/References queryは先に原文から固定した7入力・11参照位置を公開prepare/execute/queryへ通した。Let initの未解決、入れ子と兄弟のshadowing、UnicodeのUTF-8 byte位置、Foreign独立root、Ambiguous両候補、escaped Text名の元綴り8..16を確認した。同綴りでも異Entityへの参照を混ぜず、候補順を勝手に一件へ縮めない。各要求・返信の実CBOR初回受信は空のsource storeを使い、全結果・SourceBytes一度分・再encodeが一致した。EOFは選択なし、scalar内部と範囲外はSourceエラー、別execution keyとsource revisionは古い解析へのアクセスを拒否し、取消はStoppedと空の候補閉包を返す。補助は `.tmp/prepare_query_expectations.py`、`build_query_runtime_probe.py`、`build_query_boundary_probe.py`、`build_query_portable_probe.py`、実行例は `cargo run --manifest-path .tmp/independent-engine/Cargo.toml --features nepl3-tools,nepl3-grammar-core --bin query_portable --target-dir .tmp/review-target` である。

Customから発行されたkey付き結果でも、source-less Entity100とMAX-1の定義先はlocation=Noneを保ち、実参照位置だけを返した。過去stage0の参照は明示更新後の最終resolutionを使い、語彙検索のやり直しで更新を消さない。追加の合法Import201/Export202を持つdeltaを受理し、両roleの包含optionを独立に切り替える4組もnativeと実CBORで一致した（`query_roles`）。通常の `import field` は子のexport候補を導入する操作であり、独立な名前Occurrenceを増やす契約ではない。追加管理対象の3件期待をこの原文で2件へ訂正した理由を実コードとspec04から確認した。query管理対象6件は独立native成功し、URI/ID/key/宣言source/openInputのschema-valid変異、元範囲・全候補・role filter・typed停止を照合した。本物のCustom Import occurrenceを使うoption正例も管理対象へ追加され、独立再実行した。query raw受信は構造と要求との対応を検査し、結果の実行認証やBindingAnalysisの意味proofを発行しない。rename・全LSP統合・T06全受入条件の完成とは区別する。

最終のWASI独立実行はbinding関連33件すべて成功した。上記Custom9件とquery6件に加え、既存の可視性・Global・AnalysisKey・portable原因・停止時閉包も含む。実行は `cargo test --locked -p nepl3-tools --test grammar binding:: --target wasm32-wasip2 --target-dir .tmp/review-target -- --test-threads=1`、runnerは `wasmtime run`。host processを起動するseed一致だけは従来どおりnativeへ分離されており、通常解析回帰はWASIで実行される。

次のRecursive Customは旧原文 `recursive cons customdef x nil x` のUnsupportedPlan・callback0・有効な部分FactSetを新API実装前に独立再採取した。spec04から固定した9原文を新しいFactsPhaseへ接続すると、Header全収集後のBody、相互参照、static/custom混合、Sequentialの非前方可視性、同plan二回の別Entity、同名Ambiguous、Foreignの同alias独立root、日本語の元byte位置が実解析で一致した。headerで受理したEntity列とbody receiptは同じIDを持つ。補助は `.tmp/recursive-custom-expectations.json`、`build_recursive_custom_probe.py` と `recursive_custom` である。同じ9原文を最終差分でも再実行した。別の独立補助 `recursive_boundary` はHeader Reference拒否、Bodyの受理済みID再追加拒否、別ID同名のBody内Ambiguousとexportされない宣言の外側非公開、cancel後の受理済みEntityと診断/event保持を確認した。管理対象4件をnative/WASI双方で独立実行し、5原文の実request/reply CBOR・同callbackによるFactSet/stage/history/report一致、型正当なgroup/provider/target/ID列改変の拒否、7資源の部分結果と通常BindingReply codecを確認した。private memoはgroup/target/BindingIdと受理順へ束縛し、使用済みheaderを再使用しない。同期transportは同じBudget/Admissionを共有する。raw receiptだけの実行認証や、別processへのquota貸出・精算はこの成功範囲に含めない。

Docの実装前期待は元spec05とDG01〜06からSentence20件・arena14件を固定した。公開validate_shapeの独立native/WASI試験では、共有DAGの最長経路3をDepth2で拒否し、共有順反転10例でもcaller7を加えたpeakが一致した。VariantへのParagraph/Parallel代入、cycle、未到達node、u64最大参照を型付きで拒否し、空Sentenceと空翻訳は許可、EN/enの重複と空Concat由来の注釈部分は拒否した。100k深いarenaの受理、Clone/Eq/dropとDepth64・型不一致時cleanupも成功した。補助は `.tmp/independent-doc/src/main.rs`。DocViewは個々のheadと局所ViewBundleを保持する列へ訂正され、単一global ViewRef空間へflattenしない。これらはgraph/category/local shapeの証拠であり、source/view閉包・ラベル・foreign解決を済ませた意味proofとは区別する。

Sentence専用の公開readへは20期待のうちliteral17件を接続し、native/WASI双方で成功した。Ruby/Annoの入れ子構造・note順、escapeから生じたdelimiterを再解析しないこと、top-level slash、原文Unicode/CRLFの失敗位置を検査した。UnclosedAnnotationのopening4..5とterminal15、成功入力の各scalar境界でのNeedMore、SourceBytes一度分、返却ViewBundle/OriginGraphの共通検査とhead内範囲も確認した。残る3件はprefix構文専用で、このreadの成功に含めない。補助は `.tmp/build_doc_sentence_probe.py` と `.tmp/independent-doc/src/bin/sentence.rs`。初回Doc管理対象7件の独立native確認に続き、source/codec接続後の管理対象14件も独立native/WASI成功した。

Docの公開normalize/portableには別の独立入力を接続した。元source由来Textとsource-less Textの両順、およびOriginなし・SpanのみのTextでも既知DirectとSyntheticをCompositeへ保持し、結合Textへ偽の全体Spanを与えない。正規化の再実行で同値、元入力とtoken局所Viewは不変で、実CBOR後にも保持した。生成sourceとExact SourceMapを加えた原文では、source表順だけを共通wireの正準順へ揃え、残りのDoc/Origin/View/Map全体を比較した。native/encodeのsource欠損、同表重複、不正OriginRef、head外View、Exact不整合は拒否した。型正当NDFからsource宣言を除く受信負例も拒否し、ambient storeで宣言不足を補えなかった。fresh Work/Allocation/Nodes/Depth/Sourceの30点は成功10・元原因のsticky停止20で、成功結果の全体一致を確認した。補助 `structure` はnative/WASI双方で成功した。 補助 `helpers` は8種類の補助rootとSome Row/Sentenceの2例を公開shape/structure/normalize/CBORへ通し、Inlineへの型偽装を拒否した。補助 `foreign` はDoc Code包絡へ非空owner/guest Env9とOrigin0の別内容を組み込み、元sender scope破棄・ambient空で実CBOR受信し2byteを一度計上、owner欠損・guest環境代入・偽digestを拒否した。両補助もnative/WASI成功した。補助sourceは `.tmp/independent-doc/src/bin/` にある。これらは型付きconstructorと共通syntax guestからの検証であり、補助構文やDocGuestの実source parse/lower、Articleラベル解決、描画の成功を表さない。

Doc拡張表層の独立構造監査では、forms正本と元syntax.neplgの64 form、およびsignature表のkind・field順・読取指定・arityが一致した。10補助categoryを確認し、元ASTのDocGuest.syntaxは明示Foreign(alias Doc, category Article)である。生成signatureの日本語が実byteで疑問符257個へ置換されていた途中差分は担当へ戻し、UTF-8の正式生成器と元日本語へ修復後に再監査した。実Grammar AST validateは成功し、当時の標準bootstrap catalogによるcompileは未登録DocSentence extensionの156..214を持つMissingExtensionとなった。このcatalog未接続をDoc文法の実parse/lower成功へ数えず、正式adapter接続後の再検証点とする。補助は `.tmp/audit_doc_surface.py` と `doc_compile`。

その後のDoc専用catalogでは、正式syntax.neplgを実seed adapter・typed Documentからcompileし64 forms・20 categoriesを得た。Doc/Math/Circuit/Grammarの4つの管理下seedも元source bytesと全AST metadataを実adapter出力へ照合し一致した。独立補助 `doc_prefix` はこれらの実compiler、4 alias Profile、各checked環境、ParseSessionのread/resume/reserve、公開lower::prefixへ接続した。元prefix3入力の `text "[a/b]"` は注釈へ再解析せず単一Text、空ConcatをbaseにしたRubyはEmptyAnnotationPart、空notesのAnnoはAnnotationNotesとして拒否した。8種類の補助categoryを実sourceからstandalone rootへlowerし、Orderedのu64最大値を保持した。別の静的branch監査では64宣言の全kindに対応する52種類のForm名をlower内で確認したが、これは全constructorを実原文で通した試験数ではない。共通descriptor生成器のstorage関数は0項・1倍項を整理しただけで、各countとWork/Allocationの算式を変更しないことも差分から確認した。補助 `doc_boundaries` はNat上限超過、短い/nonhex asset digest、Table列数不一致を型付きで拒否し、64hexの正確な32byte digest、0列・空rowを受理した。実Rubyのlowerではcaller7を含むpeakが基準+7となり、Depthの11点は元理由のsticky停止10・全体一致の成功1であった。これらはnative/WASI双方で独立実行した。

独立補助 `doc_guest` では、空notesのAnnoを持つArticleは直接lowerでShape失敗する一方、同じ原文を `code Doc` へ入れると意味lowerせずCodeのForeignClosureへ保持した。同aliasの二重入れ子でも後続host Paragraphへ復帰し、guest Articleは別SyntaxBundleのまま残り、Doc意味arenaへAnnoを混入しなかった。実CBORは共通wireのguest NodeRef正準化を前提に再encode一致を検査し、Doc nodes/origins/views/mapsも保持した。native/WASI双方で成功した。補助sourceは `.tmp/independent-engine/src/bin/doc_prefix.rs`、`doc_boundaries.rs`、`doc_guest.rs`。この新しい実source経路は先の型付きconstructorだけの証拠と区別する。SentenceLiteral payloadをprefixへ混在させるadapter、Articleのlabel/embed意味解決、全formの往復・描画までを完了とはしていない。

非空の環境を持つDoc埋め込み閉包の準備では、既存共通wireにR046を実再現した。core syntaxのNamespaceRefはschema/nameの組であるのに、environment encoderはfacts用U64参照の同名recordへ2fieldsを出し、実Environment digest計算がSchema(FieldCount)で失敗する。空環境の従来試験はこの経路を通っていなかった。環境側をEnvironmentNamespaceRefへ分離後、元補助を変更せず再実行して成功した。実 `lambda x guest x` のowner Env9/Origin0はHOST＋専用source、guest側の同番号はGUEST＋別sourceとし、元owner破棄後・ambient空の実CBOR受信で両arenaとdigestを保持、sourceを一度だけ計上した。owner環境をguest環境へ置き換える、外schemaとguest root schemaを違える、owner sourceを消す（guest側に同snapshotがあっても補えない）、両digestを同じ偽値に変える、NDFのowner source列を消す各負例を拒否した。管理対象wire syntax8件（foreign2件を含む）も独立native/WASI成功し、この具体範囲でR046をcorrectedとした。補助は `.tmp/build_foreign_closure_probe.py` と `foreign_closure`。ForeignClosureの構造・所有・digest検査は、Doc/guestの意味lower・label解決・表示許可の証明とは別である。

R047は新Doc decoderのWork停止試験から、既存共通codecの原因抽出不足として切り分けた。旧validation依存の実 `encode_checked(Unit, Unit, Work0)` は `Schema(Stopped(WorkLimit))` を返すが、`FoundationCodecError::stop_reason` はNoneとなる。修正前後で同じ補助sourceを実行し、修正後は元nested payloadとBudgetの停止を保持してSome(WorkLimit)を取得、通常WrongTypeはNoneのままであることを確認した。管理対象wire stop2件はnative/WASI双方で成功し、9停止理由の各typed wrapperと非停止原因を区別した。補助は `.tmp/run_r047_probe.py`、元全出力は `.tmp/r047-probe/before.log`、修正後は `after.log`。この修正は共通原因抽出だけの証拠であり、後続Doc実装の完成を表さない。

仕様の通読、具体例による矛盾の確認、公開規格との照合を行った。r4では上記のcore/wire公開APIとNDF intrinsic roundtripに加え、標準Grammar原文のnative bootstrapを実行した。4言語全formのRust parse/lower、browser描画、回路実行、LSP、portable operation provider、および全要求targetでのconformanceはまだこのレビューの実行範囲に含まれない。対応する実装が存在する段階で、implementation-status.jsonの未実行記録を実行証拠とともに更新する。

renameの独立レビューでは、元Grammar入力を実compiler・CompletedParse・bindingへ通し、候補編集をprivate draft内で実parse・再解析してから受理した。Letの元byte位置4..5/14..15/16..17、Lambdaのshadowing、Unicode名、CRLFとForeign root分離を照合した。外側の自由参照捕捉・内側宣言による捕捉・予約headは拒否され、対象namespaceと関係しない別言語だけのheadは拒否理由にならない。別の実parseが完全同じtreeを返しても、元prepared proofと異なるCompletedParseの組合せは拒否される。raw cloneや受信NDFから完了parse proofを再発行する入口はない。

保存前の独立入力で、SourceMap.targetという理由だけで補助source全体を比較から除く初稿を検出した。`lambda x x` のxをzへ変更し、部分Exactの外にある補助source `l a` を `l b` へ改変しても旧入口が受理した。修正は全宣言sourceのbytes・revision・URIを比較し、候補treeを実ParseSession由来のCompletedParseへ限定する。旧raw入口の原probeとfull logを保全し、新APIでは補助sourceを実provider返信で宣言するよう適応した。未改変補助sourceは成功、末尾改変・triviaだけの変更・URI変更はRequestMismatchとなる。API変更を挟んだため、旧probeの無変更再実行とは記録しない。

隣接Exactの独立入力 `lambda ab ab` は、補助source `ab` の同一変位による連続した1byte区間を一意逆写像として扱うべきだが、初稿は単一mapだけを要求して拒否した。全区間の穴なし被覆へ修正した後も、実候補parse後のacceptがShapeChangedを返す別の不足を検出し、公開root editに加えて内部派生editを形状比較へ接続した。修正後はwhole・split・逆順のmap表で実acceptまで成功し、穴・異source・異変位・曖昧な逆写像・Transformed混在は拒否した。`ab→xyz`、`あい→漢`、`あい→漢字仮` のwhole/split計6例では、新readerが返すsegment境界と表順も変えて成功し、公開編集は元補助sourceへの1件に限られる。

同6例を実request/reply CBORで初回受信し、元storeを共有しないreceiverで再encode一致とsource bytes onceを確認した。writable除去、source宣言欠落、expectedDigest・replacement改変は拒否し、6資源のdecode停止は元理由を保持した。候補parse・binding後にWorkを使い切る場合とcancelする場合は、acceptが編集を公開せずStoppedと累積Usageを保持する。現在のrename ReportはUsageだけを返し、各parse/analyzeの正式Reportはcallerが保持する二段入口である。全段共通Report包絡、任意encoder、raw外部実行の認証はこの範囲の完了に含めない。

renameに付随して公開した `NdfValue::equal_with_budget` は旧private comparatorの算法を引き継いでおり、Work=1で100000子をqueueへ積んでから停止することを実測した。Allocation自体は課金されるが、幅比例の走査が次のWork課金より前に起き、native AllocationUnitsは2400024だった。子enqueue前のWork課金修正後、原probe無変更でWorkLimitを保ち、queueはroot1件の24bytes（WASI16bytes）だけとなった。追加課金により元controlのWork=400000は不足するため停止する。別の高予算controlではWork=400001でtrueとなることもnative/WASIで確認した。新規rename退行とは断定せず、保存前補修と旧private由来を分けて記録する。

独立rename補助11実行はnativeで成功し、原入力・修正前後のfull process log・sourceとbinaryのSHAを `.tmp/review-rename-evidence/manifest.json` と `.tmp/review-compare-frontier/manifest.json` に保存した。保存済みrename補助binaryは各時点の成功buildであり、全最終snapshotとのlink対応は未証明である。管理対象rename9件はnative/WASIで独立成功し、比較境界のcore value2件も両targetで独立成功した。元parse/head completionの専用2件と全workspace最終gateは統括の固定snapshot検査範囲として区別する。今回レビューはrenameと付随比較の範囲であり、後続Doc SentenceLiteral混在lowerやR006等の未達を完了扱いにしない。

続くDoc混在lowerは、rename保存後の固定baseと凍結8fileを隔離workspaceで照合し、その固定依存先へ独立補助をnative/WASIとも新規buildして確認した。元の独立入力3対は、CRLF/Unicodeを含む入れ子Ruby/Annoとprefixの混在、escapeで得た括弧を注釈へ再解釈しない場合、空Sentenceである。実compiler・ParseSession・`lower::document` の意味正規形はそれぞれ明示prefixと一致し、元host Origin列、token-local View列、操作内source bytes onceと初回CBOR後の保持を確認した。

追加の構造合法payloadでは、Composite内部の別token Direct Origin、別tokenを指すSynthetic anchor、意味nodeのSpan欠落、自tokenと別tokenの両方へ戻るgenerated位置を拒否した。共通SourceMapの全経路包含を用い、Viewだけ一致させても意味位置の閉包を省略しない。位置とViewを保ったままTextの意味内容だけを変更する正例は受理される。これは受理済みreader payloadの位置契約を検査する入口であり、元文字列の再parseによってreader実行の真正性を証明するものではない。

別の独立原文で5資源それぞれの実消費量を基準に0・必要量直前・必要量ちょうどを試し、両targetとも10停止/5成功となった。caller深さ0のpeak14がcaller7では21となり、停止・cancelでも元の構文木は全Eqで保持された。Codeの前後にhost literalを置いた追加例は、guestを単独lowerすればAnnotationNotesとなる同言語Articleを、hostの意味値へ混ぜずForeignClosureの構文として保持し、元Origin/Viewと初回CBOR正準再encodeを保った。これら独立3補助と管理対象mixed4件はnative/WASIで成功した。source・固定snapshot各SHA・新buildしたbinary・6本のfull process logは `.tmp/review-doc-mixed-fixed/manifest.json` に保存した。Doc label解決、Region selector、printer、CheckedArticle/PreparedArticle、正式lower操作のReport包絡はこの変更の完了範囲に含めない。

Doc labelsは独立6原文を実compiler・ParseSession・混在lower・`labels::check`へ通し、前方Reference、Unicode/CRLF、SectionとAnchorの同名重複、未知名、Foreign内だけにある定義、hostとguestの同名を確認した。名前の選択位置は固定UTF-8 byte期待と一致し、表示labelや見出し内に同じ綴りがあっても名前operandと取り違えない。初回CBOR受信後の再checkでも、同じ定義ID・参照先・位置と型付き失敗を保持した。guest内の重複をhostのDocLabel空間へ持ち込まない。

共有DAGの契約は単一nodeの共有と表示上のanchor出現を区別する。独立のsource-less Doc arenaで、1層および200層の二重参照parentを構成し、共有AnchorをDuplicateOccurrenceとすること、返された二つのowner/child/target経路が実際のArticleからAnchorへの辺列であることを検証した。1層の経路は固定子index列 `[1,0,0,0]` と `[1,0,1,0]` に一致した。名前のない同じDAGは受理される。200層も指数的な表示展開を作らず、原node ID・二経路を通常Diagnostic引数とCBOR Reportに保ち、source-less診断のprimary/related位置を捏造しない。

operand位置の独立native/NDF負例は、別constructorのDocField、重複field、cover外のSpan、無関係なOrigin、範囲外Originを構造検査と初回CBORの両入口で拒否した。Originを指定しない合法位置、および明示Exactで生成sourceへ写像した位置は受理される。複数のOrigin寄与のうち正当なcauseが選択範囲を支えるCompositeも保持する。一方、別範囲へ戻る追加mapと、ambient storeにsourceがあってもpayload宣言を欠く値は拒否した。fieldの寄与関係とSentenceLiteralの全token内包含は別の契約として照合し、raw位置情報を元reader実行の認証proofとは扱わない。

独立の5資源境界は実必要量の0・直前・ちょうどで10停止/5成功となり、caller深さ0のpeak6がcaller7では13になった。元DocumentSyntaxは不変だった。通常DuplicateLabel診断でも元primary/relatedの名前operand位置を確認し、4資源の必要量直前で停止しても借用元の型付き失敗を保持した。固定labels24fileとmodule順だけのfmt追補へ依存先を束縛し、独立4補助と管理対象labels7件をnative/WASIで成功確認した。新buildしたsource/binary SHA、8本のfull process log、固定snapshot SHAは `.tmp/review-doc-labels-fixed/manifest.json` に保存した。この範囲はArticle内DocLabelとその位置・診断・DAG出現の検査であり、外部page/asset解決、Region selector、printer、CheckedArticle/PreparedArticle全体の完了ではない。

Doc plain_textは、独立3原文を正式4言語packageの実compiler・ParseSession・混在lowerへ通し、各3policyの9出力を固定期待と比較した。RubyのbaseにAnno、そのnoteに別Rubyを置いた例では入れ子全体に同じpolicyが適用され、WithAllNotesの `漢{義[ぎ]}[かん]字{//注[ちゅう]}[x]{}` と末尾LFを保持した。slashを含むnote、InlineCodeのdelimiter、hostが明示したMath表示値 `七/[x]` をescape・再parseせず、空Sentenceの出力は全policyで空文字となる。

identityは公開定数を期待値として流用せず、仕様のASCII domainと単一ゼロbyte、および正式DocumentSyntax/ForeignClosure codecのcanonical CBORを独立に連結してSHA-256と照合した。Mathのreading/notesを隠すpolicyは未提供値を要求せず、提供済みの不正guest identityと重複EmbedRefは非表示でも拒否し、表示対象の未提供値だけUnresolvedEmbedとなった。この値はhostが供給する表示textであり、guestの意味解析・評価成功の認証ではない。

元要求をCBORへ変換して破棄し、空SourceStoreと新admissionを持つ受信codecで要求を復元して公開plain_textを実行した。元入力へのambient参照を用いず同じ出力になり、decodeと実行で同snapshotを二重計上しない。返信も実CBOR往復でoutcomeと正式Reportを保持した。独立の9要求それぞれでSource上限0、Work/Allocation/Output/Depthの実必要量直前を試し、元要求を変更せず元停止理由・sticky Budget・累積Usageを保持した。caller深さ7を加えた実行のpeakは0の場合からちょうど7増えた。

固定plain_text15fileへ依存先を束縛した独立補助と管理対象text5件はnative/WASI双方で成功した。管理対象の別原文再parse、guest交換、owner環境resource変更、非Stopped overflow、digest byte長、32bitで切り捨てられ得るSentenceRefもコードと実結果を確認した。固定source SHA、元補助source、新buildした両binary、2本のfull process logは `.tmp/review-doc-text-fixed/manifest.json` に保存した。今回の範囲はplain_text操作の明示policy・入力閉包・typed request/replyであり、一般printer、外部asset/page解決、Region/query、CheckedArticle/PreparedArticle全体の完成とは扱わない。

一般Region selectorの保存前レビューでは、元 `pair 4 6` のWord tokenが保持する `[4,5)` Whitespaceを、sidecarだけ `[0,4)` Skippedへ差し替え、その範囲にCaptureを足すと、native prepareと実CBOR受信の両方が受理する不整合を再現した。Token.leadingTriviaとbatch.triviaは03章の正式再掲契約であり、先行するsignificant headをskipと自己申告してよい境界ではない。実装修正後は元probeを変更せずnativeで拒否を確認し、固定版でもnative/WASIで再確認した。正常sidecarをencodeした後のschema-valid NDF改変も実CBOR受信で拒否し、正常値は保持した。元source・失敗binary・full before logと修正後の証拠は `.tmp/review-region-sidecar/` に分けて保存した。これは未freeze実装の補修であり、catalogの新規採番や別の未達項目の削除には用いない。

reader artifactの契約はCaptureとPresentationを分けて照合した。実Builtin readerが `lambda x x` の先頭 `[0,6)` を読み、明示Transformed mapで生成したsource上のCapture、および未読body `[9,10)` のPresentationを返す独立正例は、実ParseSessionに受理されRegion native/CBORでも保持された。同じ外部範囲へのCaptureは拒否した。Captureはowner内のSourceMap包含、Presentation/Relationは宣言source/schemaの検査という既存Reader規則を維持し、View名からCaptureを推測したり、raw sidecarをprovider実行の認証proofと見なしたりしない。

Foreign境界は実 `lambda x guest x` のguest bundleだけに補助sourceからhost全域へのExact mapを置いて検証した。typed tree再検査を通るこの入力でも、補助sourceへのRegion投影はguest `[15,16)` とそのForeign fieldだけとなり、hostのroot/headへguest mapを流用しなかった。先頭0とEOF16は非選択、15は選択され、caller深さ0のpeak6はcaller7で13となった。回復の実 `lambda あ` ではMissing `[10,10)` をRecoveryとして保持して選択せず、scalar内部byte8/9を拒否した。正常 `lambda あ あ` のEOF14も非選択で、回復nodeに架空のfield/childを追加しない。

順位の独立期待は、同範囲でpriorityの高いparent field、priority同値で内側node、より狭いReading Viewの順に確認した。実 `pair 5678 90` のReading `[6,9)` はpriority9の親fieldより短いため優先された。同長・同priority・同depthとなる二fieldを一つの補助source位置へExact投影した別入力では、map表順を反転しても宣言first fieldを選んだ。Source/Work/Allocation/Depth/Cancelの停止では不完全な候補やsource列を成功結果にせず、正式Usageとsticky停止を保持した。native型付き候補とtyped reply変換はOutputBytes上限0でも成功し、sidecar準備のhash用canonical CBORと実返信CBOR encodeはOutputLimitで停止した。生成前のserialization byte数をnative候補へ推計加算せず、Reportの計量時点を後続encode/transportまでの総費用と混同しない。

Region57fileにOutput契約の文書追補とCIのWASI実行1行だけを加えた固定snapshotへ依存先を束縛し、独立8補助と管理対象region8件をnative/WASI双方で成功確認した。正式Grammar原文から初期adapterで生成した同じseed bytesを両targetの実compilerへ渡し、元反例の入力と期待を維持した。source・seed・固定snapshotのSHA、新buildした16binaryと16本のfull process logは `.tmp/review-region-fixed/manifest.json` に保存した。通常10Bの全Grammar bootstrapを除外する変更はなく、全体実行は統括の別gateで確認する。今回の完了範囲は一般region候補・選択・明示sidecar・raw codecであり、既存definition/references queryへの接続、LSP adapter、Doc printer、raw provider実行認証は含めない。

Name/Langの共通core抽出は、旧Lang実装と移設後をimport・公開可視性の2箇所以外で全byte照合し、ABNFの認識処理と計量を変更していないことを確認した。Name reader、Number/Langの識別子境界、Reader CharClassの差分は同じcore文字述語への委譲であり、pinned `unicode-ident = 1.0.18` の依存ownerをreaderからcoreへ移した。固定crateの実生成表headerがUnicode 16.0.0を参照することと `default-features = false`、coreのno_stdを確認した。内部workspace依存の逆流やdomain同士の依存は加えていない。

独立補助では全1,112,064 Unicode scalarについて旧pinned述語・新core・CharClassの開始/継続判定を照合した。source-lessの全Name正負入力と4scalarのWork境界は、SourceBytesや割当を要求せず型付き停止を保った。Langは14固定正負入力×6予算で旧関数とのResult・Usage・sticky停止の全一致を確認した。登録を伴わない8字言語名や重複extension singletonもABNF上のwell-formedとして扱い、registry-validや重複禁止の別条件を暗黙に追加しない。実ReaderではName/Langの各scalar切断位置でNeedMore、finalと区切りで元綴り・byte終端の最大一致、Number/Langの直後のUnicode識別子文字でBoundaryMismatchを確認した。

固定lexical12fileへ依存を束縛した独立補助、管理対象core lexical3件、既存reader builtin17件はnative/WASI双方で成功した。原Lang source、比較用補助、固定Unicode表とsnapshotのSHA、新buildした両binaryと2本のfull process logは `.tmp/review-lexical-fixed/manifest.json` に保存した。全綴り検査は正規化、選択言語の予約head判定、BCP47 registry照会を行わず、readerの非final/最大一致契約を置き換えない。Doc printerやRegion queryの後続実装はこの抽出の完成範囲に含めない。

Doc printerの固定差分は、design/forms.json、正式syntax.neplg、手書きprint fixtureの(category, kind)集合を独立に照合し、20 category・64 formの過不足がないことを確認した。補助fragmentも具体的な再parse entryを返し、guestはMathGuest / CircuitGuest / Guestの3入口とMath / Circuit / Grammar / Docの4 wrapperを保持する。MathGuest/CircuitGuestのtyped rootには言語制約があり、GrammarGuest/DocGuestを架空のcategoryへ拡張しない。

独立9原文×Prefix/Compactでは、空Sentence、角括弧・波括弧・slash・引用符・backslash・CRLF・tabを含むText、Rubyのreading内Anno、BreakとStrongによるprefix保持、Inline/Flow root、RawCode、Someの空Text、予約headと同綴りのName operandを実parse→lower→print→再parse/lowerで照合した。初回CBOR受信は新しい空SourceStore/admissionで行い、印字outcomeと正式返信CBORの全値を保持した。Compactの出力を再び注釈として解釈して内容を変えたり、InlineをSentence entryへ置換したりしなかった。

guestの独立5入力は、Sentence内Mathと後続host Text、MathGuest、CircuitGuest、Grammar wrapper、意味的に不正な注釈を含むDoc wrapperを扱った。保持原文を候補として取得した後、選択済み言語の実parser・tree検査・generic engine printerを実行してからhost textを提供し、両印字modeと初回CBOR受信後のguest構文を照合した。document/guestのdomain byte列とcanonical NDF CBORからdigestを独立計算し、owner閉包込みのidentityとの一致を確認した。異schemaを同じGuestLanguageへ割り当てるalias競合、別guest digest、32bitを超えるEmbedRefは型付き拒否となった。

PrintedGuest.textの意味は両digestで認証されない。同じidentityのままtextを `@` に変えた初回受信要求はDoc組立ではCompleteとなるが、実parserはその出力を拒否した。別の合法Math textもそのまま採用された。この挙動は05章8.2に明記したhost assertionの境界であり、通常roundtrip保証には同guestを扱う実printerの正当出力を供給する前提がある。raw要求、Complete SourceArtifact、原文取得helperをguest実行proofへ昇格させず、Code内Docの意味lowerを保証の前提にしない。

Source/Work/Nodes/Allocation/Depth/Outputの6資源を0・必要量−1・必要量で検査し、停止理由、sticky Budget、Report Usage、実返信CBORと元要求不変を確認した。元sourceの合計byte数だけをSourceBytesへonce計上し、caller深さ7はpeakへ合成された。source-lessの512段Strongは反復印字で全内容を保持し、Depth64で型付き停止した。source-less Nameの空・先頭結合文字・数字・空白は置換せず失敗とし、Unicode名と予約headのoperandは保持した。

Doc printer28fileと対応表1fileだけの固定snapshotに依存先を束縛し、独立3補助および管理対象printer7件をnative/WASI双方で成功確認した。管理対象には全64form×2modeとguest6fragment、共有DAG出力停止、型正当なdigest長/overflow等の受信境界を含む。固定source、入力、補助、6binary、全process logとSHAは `.tmp/review-doc-print-fixed/manifest.json` に保存した。今回の範囲はDoc printerの明示guest供給を伴う操作であり、render、PreparedArticle全体、RegionQuery、外部guest実行認証の完了を主張しない。

R048は後続RegionQueryのowner監査から見つかった既存renameの別不具合である。実 `lambda x guest x` のguest reader返信だけが補助source `x` からhost宣言 `[7,8)` へのExact mapを持つ入力を、通常のParseSessionで受理させた。host bundleのmapは0件、guestは1件なのに、補助sourceだけをwritableとしてhost名を `z` へrenameすると、prepare・実候補parse・binding・acceptを経て補助source `[0,1)` の編集がCompleteとして公開された。mapなしcontrolは正しくhost原文を編集した。raw treeやsealの書換えはなく、renameが平坦なfacts.source_mapsをinverse/派生編集/比較に使うことで所有を失う実再現である。元probeとfull before出力は `.tmp/review-rename-owner/before/` に保持する。初回の全source記録は後続Query snapshot取込み後の採取であり、最初のbinaryとその全依存sourceが一致した証拠とは扱わない。修正後の元入力再試験が済むまではopenとし、先に保存済みのDoc printer結果へ混ぜない。

RegionQueryの固定27fileとR048修正10file（重複を除き34file）を独立に検査した。選択領域のlogical spanと正準bundle所有者を、実Bindingが発行したbundleScopesのrootへ結び付け、名前文字列の再検索でなく最終Occurrence/Resolutionを参照する。Foreign fieldの子位置はguest所有で扱い、返却regionの表示座標をhost定義の根拠へ流用しない。mapはそのbundleの局所表と、Custom受理時に同時公開したcustomSourceMaps行に限定される。

独立原文の6位置では、`let x x apply x x` の初期化式を未解決、bodyの2参照を宣言 `[4,5)` へ、入れ子Lambdaを内側宣言 `[16,17)` へ対応させた。同じsource内のguest自由名へhost宣言は漏れず、guest内Lambdaはguest宣言 `[22,23)` へ対応した。`lambda あ apply あ あ` はUTF-8 byteの宣言 `[7,10)` と参照17/21を保持した。各入力をBindingRequest/sidecar/RegionQueryRequestの実CBORへ通し、空storeの初回receiverで実Bindingを再実行した結果ともDefinition/Referencesを照合した。Unicode scalar内部はInvalid、半開終端は選択なしであり、5資源の0上限とCancelは正式停止・空source返信・実Usageを保持した。

独立Custom入力 `custom 名 guest custom 名 名` は、host/guestで同じ生成sourceを共有しながら疎Entity ID 5000/7000を発行した。元byte7はhost Entity、24/28はguest Entityへ対応し、guestだけのmapをhost keywordへ適用しなかった。FactsEmitterと検査済みdeltaの両経路、初回CBOR受信後の実再binding、正式返信CBORが一致した。生成map受理後の取消ではmap・owner行・eventを保持し、未受理deltaを採用しなかった。source-less定義位置はNoneのまま保持した。

raw bundle ledgerの番号範囲外、root重複、map範囲外、同owner重複、owner間の同index重複を型正当CBORから拒否した。一方、構造上合法なowner行の交換と空の所有index列はrawデータとして復元可能である。04章の規範どおり、これだけで実行済みBindingAnalysisやBoundBindingReplyを発行せず、元要求・Profileに結び付く実bindingが意味proofの根拠となる。raw構造検査を通信認証やCustom授権へ拡大して主張しない。

R048の修正後は、各Entity/Occurrenceのnamespace rootからmap所有者を選び、前向き導出も各元編集とownerの組ごとに閉包をたどる。再解析比較はold/newそれぞれの所有者、構文位置は正準bundleを用いる。元反例のprobe source SHAを変えず、guest mapしかない補助sourceへの書込要求がNotWritableとなることをnative/WASI双方で確認した。mapなしcontrolの実parse・binding・acceptは元host `[7,8)` 編集として成功した。別の独立 `custom 名 名` はemitter/delta双方で同ownerのExact逆写像を保持し、元source2箇所を `字` へ変更、派生source revision1を実再生成・sealed parse・binding後にacceptして成功した。関連管理rename11件も両targetで成功しており、この元所有混同の範囲でR048をcorrectedとする。

R048のbeforeはnative実行である。初回採取の制限は前段落のまま残し、別途 `.tmp/review-rename-owner/before-query27/manifest.json` に固定Query27の全SHAを実行前後に確認した無変更probeの失敗（終了1）を保存した。afterは固定34fileを前後確認し、独立4補助をnative/WASI双方で再build・実行した。関連管理17件も各targetで成功した。全process log、8binary、元入力・補助source・固定SHA・before参照は `.tmp/review-region-query-fixed/manifest.json` に保存する。統括の全workspace/品質gateは別記録であり、今回の限定検査から全T06、外部実行認証、Mathの完了を推定しない。

Mathの最初の表示構造差分は、06章、正式forms/syntax、Math schema、生成adapter、公開arena/check/lower/codecを独立に照合した。Expr / Row / DocGuestの3カテゴリ、29 form、Numberを加えた30 MathKindと、bare SymbolNameを含む31元入力fixtureの集合は一致した。Math coreのproduction依存はfoundation coreとno_std数値libraryであり、Docの意味coreやtoolsへ依存しない。Symbol/Let/Sum/Integralの位置は閉じたfield型、guestはowner環境を含むForeignClosureとして保持し、構造proofをbinding/evaluate済みのCheckedExpressionへ読み替えない。

独立10原文は、`frac 2 4` の非簡約、`add 0.10 0.20` の正確な1/10と1/5および原綴り、scriptsのbase/sub/sup順、Unicode名とCRLF、escapeしたSymbol、単独の空Row、非zeroの負root次数、片側Unicode fence、意味上不正な空Annoを含むDoc Label、DocGuest literalを扱った。Let名 `[4,7)` とbody名 `[15,18)` の原byte位置を固定期待として照合し、元parse treeは不変だった。root0と不等列Matrix・空rowを取り込むMatrixの独立3失敗は、破棄したMath arenaの番号でなく元構文NodeRefへ帰属した。表示構造の段階で負rootを評価domain違反にしたり、Doc annotationを意味lowerしたりしなかった。

元送信storeを共有しない初回CBOR受信では、Math node/field位置・Origin・View・SourceMapを保持し、guest込みのcanonical CBOR再encodeが一致した。13章に従いsource表とguest構文は正準化されるため、nativeの配列順全Eqを保証とはせず、sourceのidentity/URI/bytes集合と位置列のexact一致を別に検査した。escape由来sourceの入場もonceだった。ambient sourceが存在してもMath宣言表の欠落は拒否し、guestのowner source表欠落とcategory変更もnative/初回CBORで拒否した。

型が正しいNDFから、Numberの1/3、Let operand位置のbody側への差替え、異なるfield型、同field重複を拒否した。別の有限Number意味値と元spelling位置の組は構造上受理される。06章の規範どおり、spellingは位置の情報であり、そのbyte列と数値payloadが意味上一致した証明ではない。valid入力のWork sweepは成功とWorkLimitの両方を実行し、他の意味エラーへ化けず、消費量はcap以内だった。Source/Work/Nodes/Allocation/Depthの0上限とCancelは原停止理由とsticky Budgetを保持した。

数値helperは小さい符号付き有理数100演算を独立な整数交差積とgcdの期待値へ比較し、256bit整数の演算でも切捨てがなかった。約分後に2/5以外の分母因子が残るNumberは拒否し、明示constructorは有限ならNumber、その他は整数2子のFracを生成した。rootが先に実測・修正したconstructor Depth0不足について、独立に必要深さ1/2とcaller7、不足時の原DepthLimit、元値不変を再確認した。修正前logはroot提供資料であり、独立の修正前実行とは扱わない。MAX/32bit超の参照、cycle、未到達node、共有DAGの最長経路、10万段Negの検査と破棄もnative/WASIで確認した。

最終固定66file＋追補8行は重複を除き68fileである。全SHAを実行前後で照合し、独立2補助とMath core8件・実parserを使うtools Math2件をnative/WASI双方で成功確認した。4binary、全process log、元入力、補助source、固定manifestと支持資料は `.tmp/review-math-fixed/manifest.json` に保存する。今回はarena/shape/lower/初回CBORと正確な有理数演算・新規notation constructorの範囲であり、Math binding、evaluate、print、render、HTML/KaTeX方針の受入完了を主張しない。今回の独立試験から新しいproduction不一致は確認されず、既存review findingの状態は変更しない。
