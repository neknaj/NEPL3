# Docページ集合とリンク解決

`nepl3_doc_core::pages` は明示されたページ集合内でリンク先を検査する。
filesystem、URLへの接続、HTML生成、guest評価は行わない。
ページの意味上の存在と、特定の出力でanchorが表示されることは別の条件である。

`interfaces/doc.json` の順序付きrecordを交換契約とする。
`PageRegistration` は安定した `id`、入力の論理 `source` path、出力artifact内の
`route` を持つ。`PageDocument` はregistrationと完全な `DocumentSyntax` を持ち、
`PageSet` は文書の順序付き非空list `pages` と、非Doc fileの順序付きlist `files` を持つ。
`PageFile` はregistrationと `content: Bytes` を持ち、元byte列をそのまま交換する。
文書とfileの登録順は別のindex名前空間とし、`PageDestination` の
`Page { index }` / `File { index }` で区別する。空のDocをfileの代わりに登録しない。

- idは空でないASCII英数字・`-_.` の一segmentで、`.` と `..` は禁止する。
- sourceはUTF-8の相対file path。segmentは空でなく、`.`、`..`、制御文字、
  `\ : ? # %` を含めない。URI decodeやOS固有のcase foldingは行わない。
- routeはartifact rootからのASCII相対file path。各segmentはidと同じ文字集合。
- idの重複、source/routeの重複とfile/directory衝突を拒否する。
  例：`docs/a` と `docs/a/index.html` は同時に登録できない。

`resolve` は全documentを既存の構造・source・label検査へ通す。同じ操作内で同じ
source/revisionを別の内容へ置き換えることはできない。文書間で同じlabel名を使う
ことは許す。未選択のParallel variantも含めて文書全体を検査する。

同じ変更不能なPageSetの解決中は、検査済みのNDF表現とlabel検査結果を再利用できる。
CheckedPagesは登録順の各document digestを保持し、同じ入力を描画するbackendへ渡す。
PageSetとDocumentのdigestは、それぞれ既定のdomainとcanonical NDF byte列から求める。
hashの合成への置換、外部から受信したplanの信用、別の入力への検査省略は行わない。
全documentの構造・境界検査、PageSetのhash、各documentのlabel・hash・要求発見、
全リンクの解決という順序を保ち、資源消費と停止は同じ操作へ累積する。

開発用Doc source hostのdecode先IDは、元source IDのUTF-8 byte長、ID、revision、
予約counterを `length:id:revision:counter` と連結する。同じsourceを同じ設定で
読み直した場合は同じIDを生成し、別sourceのdecode結果を単独counterで衝突させない。
入力bundle側が予約namespaceと衝突すれば既存source検査で拒否する。短い表現を使い、
長い解説例もWork 100M / Allocation 500Mの既存上限で検証する。

## HTMLページ集合

`nepl3_doc_html::pages::render_pages` の入力はPageSetと共通RenderOptions。
全ページを解決してから同じbackendで生成する。外部リンクは19章の既存Markup URI
profile（http/https/mailto）で検査してHrefとして生成する。これは参照文字列の生成であり、
接続・到達性確認・リンク先の処理は実行しない。未選択variant内のURIも検査し、不正なら
`InvalidExternalUri { page, node }` で拒否する。schemeの推測、空白除去、相対URIからの
外部URL補完を行わない。許可集合はMarkup validatorと同じ実装を使用する。
画像・foreign等の未解決要求があれば元のplanを返して停止する。このplanは意味側の
要求を保持するため、既に字句検査した外部リンクも含む。空の表示で代替しない。
結果のfragmentsは登録順であり、
PageSet identityと各document digest、実際のoptions、markup、origin対応を保持する。

ページ集合内のリンクは `BetweenArtifacts` として実際の元/先routeを結び、fragmentを共通のhex ID
規則で変換する。全ページの生成後、出力された各hrefのfragmentがリンク先HTMLに
実在することを検査する。Single表示で隠れたanchorを参照した場合は
`MissingOutputAnchor { page, node, target }` とし、壊れたリンクを成功で返さない。
リンク元自身が非表示の場合は出力hrefがないため、この表示上の失敗にはしない。

portableは全PagesHtmlRequestから生成を再実行し、RenderedPages全体を照合する。
route、options、source、targetの変更後に古い結果を採用しない。この結果はHTML
fragment集合であり、fileの保存完了や公開確認の証拠ではない。

## 開発hostでの書き出し

`nepl3-tools doc-html pages <manifest.json> <new-directory>` は、manifestのdirectoryを
入力rootとし、version 1、pagesの各id/source/routeから文書を読む。sourceの絶対path、
親directory移動、root外を指すsymlinkを拒否する。入力manifestは64KiB、ページ数は128、
全入力UTF-8 byte合計は10MBまで。現在の表示はRowsに固定する。

各pageには省略可能な `input` を指定できる。これはmanifestのdirectoryから実際に読む
Doc原稿の相対file pathであり、リンク解決用の論理 `source` とは区別する。
省略またはnullなら従来どおりsourceを読む。明示したinputは非空・4096 UTF-8 byte以下とし、
空segment、`.`、`..`、制御文字、`\ : ? # %` を拒否する。絶対pathやroot外へ出るsymlinkも
許さない。見つからないinputをsourceへfallbackせず、出力directoryを作る前に失敗する。

例えばsourceを `docs/intro.md`、inputを `drafts/intro.nepld` とした場合、
本文の `guide.md` は `docs/guide.md` の登録へ解決する。drafts directoryへ読み替えたり、
原稿のリンク文字列や拡張子を書き換えたりしない。source名が `.md` でも、inputから読む
本文はDoc DSLである。これはMarkdown parserを呼ぶ設定ではない。
成功manifestは実際に選択したinputと論理sourceをともに記録し、読んだbyte列のdigestを結ぶ。
物理的な配置名だけを変更して同じDoc byte列を渡した場合、PageSetの意味identityは変えない。
未登録の論理ページや未解決assetをinput設定だけで解決したとは扱わない。

非Doc fileはmanifestの省略可能な `files` listへ、pageと同じid/source/route/inputで
明示登録する（省略は空、nullは禁止）。最大128件で、Doc原稿と合わせた実入力byte数は
10MBまで。入力path・root境界の検査はpageと共通で、fileの内容はUTF-8へ変換せず保持する。
`generate_with_resources` も明示されたEntryとbyte列だけを受ける。linkを根拠に周囲のfileを
探索しない。返却HTMLが参照するfileを、hostは同じ出力集合へbyte完全一致で保存する。
manifestには登録元・実input・route・byte数・SHA-256を記録し、fileのMIMEは
`application/octet-stream` とする。これは任意fileを検査済みHTML・CSS・画像として
認定するAPIではない。公開host側のMIME配信設定や内容実行の許可は別契約である。

文書とfileを合わせてid/source/routeの衝突を検査する。衝突診断のregistration indexは
pagesの後にfilesを連結した順序。`LinkTarget.Page` はDocだけを検索し、`Relative` は
正規化後のsourceが一致するDocまたはfileを検索する。fileにはDoc anchorがないため、
fragment付きは `FileFragment { page, node, file }` として拒否する。fragmentの文字列を
勝手に落とさない。fileへのHTML hrefは実route間の相対参照となる。

PageSetのcanonical NDFはfileのregistrationと実byte列を含む。byte列・route・登録順が
変わればidentityが変わり、portable receiverは古いplan/HTMLを再利用できない。fileの
境界codec・hash・出力コピーも有限の共通output Budgetに累積し、cancel/停止を引き継ぐ。
生成するHTML、stylesheet、完了manifestとのpath衝突は保存前に拒否し、stylesheetと
byte列が偶然一致してもfile登録を共有扱いにしない。この拡張はDoc schemaのdigestを
更新するため、旧digestのPageSetを新しい2-field recordとしてdecodeしない。

全pageを同じproduction APIで生成し、HTMLごとの相対 `assets/doc.css` を同梱する。
同じdirectoryのCSSは共有し、byte列の異なる同名fileやfile/directory衝突を拒否する。
WindowsとUnixで配置が変わるのを避けるため、共有path segmentの大小文字の不一致、
末尾dot、Windows予約device名も生成後・保存前に拒否する。公開URLをOSに合わせて
黙って改名しない。page routeは `.html` で終わらなければならない。

既存directoryへ上書きせず、全HTML・リンク・配置の検査後に新directoryを作る。
manifest.jsonは全file保存後に作り、PageSet identity、原稿/Profile、fileのdigest・MIME、
固定CSSのlicenseを記録する。I/O途中失敗は未完成directoryを残す場合があるが、成功の
manifestを付けない。この開発hostは静的文書生成までであり、公開サイトのnavigation、
旧URL互換、Pagesの配信・復旧操作は含まない。

`LinkTarget.Page` は登録idで検索する。`Relative` はリンク元sourceの親directory
から解決し、`.` を除去、`..` を一段ずつ戻す。明示したrootより上への移動、絶対path、
空segment、percent encoding、末尾のdirectory指定を拒否する。
正規化後のsourceと完全一致する登録だけを採用する。暗黙の拡張子追加、index補完、
最寄りページへのfallbackは行わない。fragmentはリンク先DocのSection/Anchorの
意味上のidと完全一致しなければならない。HTMLの `n-` hex IDへの変換はbackendの責務。

`Relative` のpathが空でfragmentが非空の場合だけ、現在の登録Docのsourceを参照先とする。
これは元文書の `#fragment` を保持するための自己参照であり、fileや親directoryの検索ではない。
空pathとNone、空pathと空fragmentは `InvalidRelative` のままとする。自己参照でも
明示IDの完全一致、未選択variantを含む意味検査、実際に出力されたanchorの検査を省略しない。
リンク先Docを推測したり、見出し本文からslugを自動生成してfragmentを書き換えたりしない。

`PageLinkPlan` のidentityは `NEPL3.Doc.Pages.v1\0` とPageSetのcanonical NDF byte列
を連結したSHA-256。登録順、route、各documentのsource・origin・内容を含む。
linksはページ順、その中でDoc arenaのnode順。各linkに元page/node、先page、
元のfragmentを保持する。外部URI、asset、foreignの要求は `remaining` に元pageと
一緒に保持し、成功した内部リンクへ読み替えない。

失敗は空集合、登録値不正（page/field）、衝突（page/previous/field）、不正な相対path
（page/node）、未登録ページ（page/node）、未存在fragment（page/node/target）、
既存の構造・label・codec失敗を区別する。全走査・複製・キュー拡張は同じBudgetを使い、
最初の停止を保持する。部分的なplanを成功として返さない。

portableのset receiverは型・source・Doc構造を検査したraw dataを返す。
`resolve` を呼ぶまでリンク検査済みではない。plan receiverは元PageSetに対して
解決を再実行し、identity・順序・全link・remainingを比較する。古いregistryの結果、
linkの省略・先の改変・偽造された成功を受け入れない。

この段階はDoc移行用の意味上のページ索引であり、完全なPreparedArticleではない。
HTML hostは実際の配置routeとの一致、選択言語でのtarget anchorの出力、全page fileの
存在を確認する。asset/guest解決、旧URL/anchor対応、一般Markdown projection、意味レビュー、
Pages配信と復旧の受入は別途必要である。文書inventoryの過去baselineをこの登録の
代わりに使わず、移行時の実際の文書集合から作成する。

## 最初のMarkdown互換projection

`nepl3-tools doc-markdown <source.nepld> <new-file.md>` は正式文書の最初の移行候補を
GitHub等でも閲覧するための限定したhost出力である。通常のDoc parse/lowerに続いて
`prepare::inspect` を実行し、未解決要求を持つ文書を拒否する。本文をsource文字列の
正規表現置換で取り出さず、検査済みのDoc arenaを読む。

対応するArticleは、同階層のSectionだけを持つもの、またはSectionを持たないもの。
段落は非空のSentence列、listは非空unordered・checkboxなし・各itemが単一段落である。
同じ段落のSentenceを宣言順に出力し、境界へ空白・改行・句点を補わない。著者が明示した
境界の空白は保持する。日本語の連続文と空白で区切った英文を同じ規則で扱い、文ごとに
Markdownの別段落を作らない。paragraphやparallelをSentenceとして平坦化しない。
Sentence内は非空TextとInlineCode、および本文途中のBreakを扱う。隣接するcode、隣接list、節の入れ子や
節外の後続blockは、Markdownでの再結合・所属変更を避けるため拒否する。注釈、parallel、
画像、表、リンク等は将来の契約・試験ができるまでUnsupportedとする。
これはDoc DSL自体の表現能力を縮小する制約ではない。

TextのASCII句読記号をescapeし、Codeのbacktickと前後空白を保持する。
制御文字、空のinline、見出しと段落の外端のText空白はこのprojectionでは拒否する。
同じ段落内のSentence境界ではText空白を許すが、隣接codeの拒否は境界を越えて適用する。
本文のBreakはbackslashと改行でCommonMarkのhard line breakへ写し、list内では
継続行をitem本文へindentする。見出し内、Sentence端、連続するBreak、Break前後の
Text空白は、Markdownで構造や空白が失われるため拒否する。DocのBreak自体を制約せず、
この閲覧用projectionの非対応として扱う。Text内の改行をBreakへ暗黙変換しない。
規則は [CommonMark 0.31.2](https://spec.commonmark.org/0.31.2/#code-spans) に従い、
実際のMarkdown parserで内容とblock/code event列を独立に比較する。

Body内のRawCodeは非評価のfenced code blockへ写す。本文内の最長backtick列より
長く、最低3文字のfenceを使い、TAB・空白・Unicode・LF・末尾の空行を保持する。
空本文は許す。非空本文は末尾LFを必須とし、CR/CRLFおよびLF/TAB以外の制御文字は
このprojectionではTextエラーとする。Markdownによる改行正規化や末尾LFの補充を
元のRawCodeと同一と扱わない。language hintはNoneまたは非空のASCII英数字と
`_+.-`のみを許し、曖昧なinfo stringを黙って変更しない。list内のRawCodeは未対応。
この制約はDocのRawCode自体やHTML出力には適用しない。
規則は [CommonMarkのfenced code block](https://spec.commonmark.org/0.31.2/#fenced-code-blocks)
に従い、生成Markdownを別parserで読み、コード本文byte列・hint・blockの分離を検査する。
CLIのrenderer識別子は `nepl3-tools.markdown/4` とする。

型付き `markdown` の準備と出力は同じBudgetを消費し、Work・AllocationUnitsに加え、
生成する各UTF-8 byteをOutputBytesへ先行計上する。本文上限は1MiB。途中停止から
部分Markdownを返さない。CLIは別途source上限10MBと入力pathのUTF-8/4096byte上限を
検査し、source path・digest・renderer版のcommentを付け、新fileへだけ保存する。
comment内のpathはdelimiterにならないようescapeする。I/O途中失敗は未完成fileを
残す場合があり、既存fileを置換して成功扱いにはしない。

開発hostのDoc操作はNodes上限を10,000,000とする。Nodesは最終ASTの大きさだけではなく、
型付き交換値等の検査訪問も含む累積量である。従来の1,000,000では、短いRuby付き512文を
128段落へ置いた約14KBの文書でも停止した。新上限はこの通常の文単位執筆を受け入れる
desktop hostの設定であり、coreの資源会計を省く規則ではない。Work上限100,000,000、
AllocationUnits上限500,000,000と他の上限は維持し、停止後の再試行でBudgetを増やさない。
bare-metal等の別hostへこの設定を強制しない。全ての大きい文書が処理可能になったとはせず、
残る資源停止と未解決リンクは区別して記録する。

文書一式のbuildでは、pages入力manifestの省略可能な `output_limits` に、
`source_bytes, work, depth, nodes, allocation_units, output_bytes, diagnostics, events`
の全8fieldを非負のu64整数で指定できる。省略時は上記の開発host既定値を用いる。
null、一部fieldだけの指定、未知field、負数、非整数は拒否する。0も有効な上限であり、
その資源が必要になれば停止する。無制限を表す値や自動増額は設けない。
これは実行前にhostが選択する出力操作の設定であり、入力本文やproviderが変更する権限ではない。
output_limits自体はparse/lower、bare-metal、previewの既定値やParseProfileのLimitsを変更しない。

型付きhost入口 `generate_with_output_budget` は呼出し側のBudgetを借り、全PageSetの
resolve/render/serializeへ同じBudgetを渡す。開始時に停止状態を確認し、既に消費したUsageを
保持する。失敗後に別予算へ切り替えず、部分成果物や成功manifestを返さない。
ファイル出力は全生成成功後に始める。I/O失敗時に残る未完成directoryと、生成操作の成功は別である。
この設定は累積する論理的な資源を制限し、物理的なpeak heapや壁時計の期限は保証しない。
package/Profile準備、hostのJSON・file容器・manifest生成・I/Oを共有出力Usageへ含めたとはしない。

成功manifestには各ページのparse/lowerの全Limits・全Usageと、共有出力の全Limits・
開始Usage・終了Usageを記録する。旧 `output_usage` は互換のため3資源の表示として残す。
PageSetの意味identityと別に `execution_identity` を記録し、出力予算を結び付ける。
計算はUTF-8の `nepl3.local-doc-pages.execution/1` とNUL、32byteの意味identity、
上記field順のLimits 8値、同順の開始Usage 8値を連結したSHA-256である。
各数値は8byte big-endianとし、JSON objectの列挙順へ依存しない。
同じ内容でも設定や開始Usageが異なれば実行identityは異なる。生成物の内容digestは変更しない。
新設定による成功を既定設定の停止記録の訂正や、文書移行・Pages公開の受入合格として扱わない。

pages入力manifestには `parse_limits` と `lower_limits` も指定できる。
いずれもoutput_limitsと同じ完全な8fieldのu64 recordであり、省略だけが既定値を選ぶ。
null・部分指定・未知field・重複field・負数・非整数・u64の範囲外を拒否する。
全ページに同じ設定を適用し、各ページのparseとlowerを別々の一操作として開始する。
出力は従来どおり全ページで一操作である。ページ数に応じてparse/lowerの合計許容量が
増えるため、これをbuild全体の単一予算とは呼ばない。128ページと全入力10MBの制限は残す。

型付き入口 `generate_with_phase_limits` はPhaseLimitsのparse/lower設定と呼出し側の
共有出力Budgetを受ける。source hostの `with_named_input_limits` はparse開始前に
同じLimitsを実BudgetとParseProfileへ設定する。解決済みProfileのdigestも実際の選択を含む。
この二つは設定を受ける入口であり、消費済みparse/lower Budgetの受領を表さない。
parse/lowerの開始Usageは0、出力Budgetは従来どおり開始Usageと停止状態を保持する。
parseにはsource admission・reader・prefix解析・完成木検査、lowerには文書lower全体を含め、
操作途中の再作成・増額・既定値へのfallbackはしない。失敗したページから後段やファイル生成へ
進まない。package・registry・Profile準備は別の既定予算を使う有限なsetup処理であり、
parseのWorkが0でもこのsetupが実行され得る。物理メモリや実時間の上限は保証しない。

各ページの記録はparse_limits、lower_limits、parse_initial_usage、lower_initial_usageと
既存の終了Usageを持つ。旧operation_limitsは両Limitsが等しい場合だけその値を持ち、
異なる場合はnullとする。lowerの設定をParseProfileへ混ぜない。既存execution_identityと
execution/1は出力操作の識別として維持し、別のphase_executionにcontractとidentityを記録する。
新しいSHA-256入力はUTF-8 `nepl3.local-doc-pages.phases/1` とNUL、32byteの既存
execution_identity、8byte big-endianのページ数、登録順のページrecordである。
各recordは32byteの実Profile digest、parse Limits 8値、parse開始Usage 8値、
lower Limits 8値、lower開始Usage 8値を既存field順の8byte big-endianで連結する。
この設定入口の開始Usageは全て0である。終了Usageを要求identityへ混ぜず実行記録へ残す。
物理input pathは配置情報として記録し、新identityへ追加しない。lower設定だけの変更でも
新identityは変わるが、同じ文書内容・出力予算の既存identityは変わらない。
parse設定変更はProfileへ反映されるため、文書identityが不変であるとは約束しない。

Sectionの明示ID、source/origin対応、Doc固有のSentence境界をMarkdownから復元する
一般的なroundtripではない。旧anchor対応・正本registry切替・人の意味レビューは別条件で、
限定projectionの成功だけで元Markdownを削除しない。

## 注釈付きMarkdown閲覧projection

`nepl3-tools doc-markdown annotated <source.nepld> <aliases.json> <new-file.md>` は、
Doc正本の読み・注記をGitHub等の閲覧用Markdownへ明示する別profileである。
既存の `doc-markdown` の互換規則は変更しない。renderer識別子は
`nepl3-tools.markdown-annotated/2` とする。版2はlistとtableの閲覧構造を追加する。
版1から引き継ぐ構造の表示規則は変えず、正本registryのrendererと生成metadataを
同じ版へ更新して再生成・差分検査する。古いrendererを新しい能力として広告しない。

Rubyは `本体[読み]`、Annoは `本体{注記1/注記2}` の表示へ写し、入れ子も型付きの
子要素を順に辿る。括弧等はMarkdownの構文として解釈されないようescapeする。
これは全注釈を読めるようにする表示上の約束であり、元から同じ括弧を含むTextと
一意に区別できる符号化ではない。Docへ復元するroundtrip形式や、Docの構造をすべて
保存した形式とは扱わない。正本のRuby・Anno・Sentenceの境界はDoc側に保持する。

通常のparse/lowerと `prepare::inspect` を通したArticleを入力とする。
Text、InlineCode、Concat、Ruby、Anno、Strong、Emphasis、外部Linkと本文途中のBreakを
扱う。Strong/Emphasisは固定の `<strong>` / `<em>` の開始・終了tagを生成し、著者が
任意のHTML・属性・styleを挿入する経路は設けない。外部URIは既存Markupの許可規則で
検査し、Markdown parserによるentity decodeでも元URIが変わらないようescapeする。
入れ子のLinkとLink内のBreakは拒否する。URIへの接続や到達性の確認は行わない。

ParagraphはSentence列を宣言順に表示し、Sentence境界へ空白や句点を補わない。
Concatは透明な構造として扱う。空TextはDocの意味正規化に従い表示文字を追加しない。
見出し・段落の可視内容全体が空の場合、外端のText空白、隣接するInlineCode、端または
連続するBreakとその前後のText空白を拒否する。透明なConcatや装飾でこの検査を回避
できない。見出しのBreakは拒否し、Text内のLFはBreakへ置換しない。

Bodyには導入のParagraph/RawCode/List/Tableと、その後にSection列を置ける。SectionのBodyも
同じ規則で再帰的に扱い、Articleの見出しを第1段として第6段まで生成する。
同じBodyでSectionの後に通常blockが現れる場合は、Markdownで所属が変わるため拒否する。
RawCodeのbyte列・末尾LF・hint・fenceの制約は既存projectionと共通である。
Listは非空のunordered/orderedを扱い、各itemは一つのParagraphからなるBodyを持つ。
Paragraph内のSentenceは通常本文と同じ注釈付き規則で連結する。checked/uncheckedは
GFMのtask markerへ写し、未指定はmarkerを追加しない。空のitem本文、複数段落、入れ子、
Paragraph以外のblockは拒否し、平坦化や仮の本文補充は行わない。orderedの開始値は
GFMの9桁以内（0〜999999999）に限り、各itemのmarkerへ同じ値を使う。以後の表示番号は
Markdownのlist表示に委ね、開始値を丸めたり加算で桁を超えたりしない。
Breakの継続行はmarker幅に合わせてindentする。隣接する独立ListはMarkdownで結合する
ため拒否する。checkboxに似た通常Textはescapeし、意図しないtask markerにしない。

Tableは非空のcolumnsと明示headerを持つ場合を扱う。headerなしの先頭rowを昇格したり、
架空のheaderを追加したりしない。headerだけでrowsが空の場合は許す。元のrow/cell順、
cell数とDefault/Left/Center/RightをGFM tableへ写す。空のSentence cellは空cellとして
保持するが、空の見出し・段落・InlineCodeを許す規則には拡張しない。cell内の注釈・装飾・
外部Link・InlineCodeは共通のinline処理を使う。Textとcodeにあるpipeをtable区切りにせず、
codeではpipeごとに追加する一つのescapeだけがGFM処理で消費されるようにする。
既存backslashやbacktickを含むcode内容も保持する。cell内のBreak、外端のText空白、
複数blockはこのprofileでは扱わず、空白除去や改行の黙殺で成功させない。
規則は [GFMの表](https://github.github.com/gfm/#tables-extension-) とlist/task listの
契約に従い、別parserで構造・文字列・checkbox・開始値を検査する。

このprofileはParallel、画像、参照、inline Anchorを扱わない。
Relative/Page link、asset、foreignの要求を含む入力はNeedsResolutionとし、
周辺fileの探索や仮のPageSet登録で成功させない。Doc/HTML側の対応範囲を狭めるものではない。

aliasesはJSONのrecord配列で、各recordの `name` は必須、`section` はSectionの意味ID、
nullまたは省略ならArticleを指す。未知field・重複fieldは拒否する。
nameは非空でUnicode英数字とASCIIの `-` / `_` のみを許す。nameの重複、存在しない
Section、生成時に到達しないSectionを拒否する。SectionにはHTML backendと同じ
`n-` とIDのUTF-8 byte列の小文字hexからなるanchorを付ける。追加aliasとこれらの
anchorの衝突も拒否する。固定の `<a name="...">` を見出し前に生成する。

aliasは移行担当が原文と対応付けて指定する。この生成操作だけではGitHub等が自動で
付ける見出しanchorとの衝突や、他ページからの旧URLの互換性を保証しない。
正本切替には、対象プラットフォームの生成結果で全anchorとリンクを独立に照合する。
自動slugを推測して旧fragmentを置き換えない。本文・コード・注釈の対応確認と、
実ブラウザでの表示・移動確認も別途必要である。

型付きrenderは同じ有限Budgetで準備・URI検査・注釈走査・出力を行う。
Work、Nodes、Depth、AllocationUnits、OutputBytesの停止とcancelを保持し、
部分Markdownや予算を初期化した再試行を成功として返さない。本文は1MiBまで。
CLIはsourceを10MB、aliases入力を1MiBまで読み、入力pathをUTF-8・4096byte以下に制限する。
source byte列、aliasesの元JSON byte列、Documentのdigestとrenderer版をcommentへ記録する。
すべての生成検査の後に新fileだけを作り、既存fileを上書きしない。I/O途中の失敗は
未完成fileを残す場合がある。公開・移行registryの切替・他ページの生成はこの操作に含めない。
