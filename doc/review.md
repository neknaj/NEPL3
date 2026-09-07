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

統括から指摘されたAngleTag例について、補助 `build_angle_probe.py` / `grammar_angle` で元sourceの全ASTとbyte spansを構築し、production reader compilerがReader(OutputType)で拒否することを独立実行で確認した。ChoiceのSeq枝2つはList<NdfValue>、Scalar枝はTextとなり、同型の選択契約に合わない。各枝を明示Discardで包むtyped AST候補は同じcompilerで成功した。R031として正式例source・契約説明・元不一致の負例・quoted `>` を含むtoken境界の実行試験を併せた訂正を要求した。まだ候補ASTだけの成功であり、修正済み正式sourceやG04合格とは扱わない。

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

仕様の通読、具体例による矛盾の確認、公開規格との照合を行った。r4では上記のcore/wire公開APIとNDF intrinsic roundtripに加え、標準Grammar原文のnative bootstrapを実行した。4言語全formのRust parse/lower、browser描画、回路実行、LSP、portable operation provider、および全要求targetでのconformanceはまだこのレビューの実行範囲に含まれない。対応する実装が存在する段階で、implementation-status.jsonの未実行記録を実行証拠とともに更新する。
