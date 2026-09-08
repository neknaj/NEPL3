# Docページ集合とリンク解決

`nepl3_doc_core::pages` は明示されたページ集合内でリンク先を検査する。
filesystem、URLへの接続、HTML生成、guest評価は行わない。
ページの意味上の存在と、特定の出力でanchorが表示されることは別の条件である。

`interfaces/doc.json` の順序付きrecordを交換契約とする。
`PageRegistration` は安定した `id`、入力の論理 `source` path、出力artifact内の
`route` を持つ。`PageDocument` はregistrationと完全な `DocumentSyntax` を持ち、
`PageSet` はその順序付き非空listである。登録順が返却indexの名前空間となる。

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

Sectionの明示ID、source/origin対応、Doc固有のSentence境界をMarkdownから復元する
一般的なroundtripではない。旧anchor対応・正本registry切替・人の意味レビューは別条件で、
限定projectionの成功だけで元Markdownを削除しない。
