# 16. 正式文書のNEPL3 Doc DSL移行

## 1. 最終成果物と移行前の正本

未移行ページのMarkdownは正式な正本として維持する。移行済みページは `doc/canonical.json` にDoc正本と生成Markdownの対応を登録する。最終的には、仕様・設計・開発文書をNEPL3 Doc DSLへ移行し、同じDoc core/backendで公開文書を生成する。初回のWeb公開を全文書移行まで待つ必要はないが、T21と移行の受入J01〜J04をT16の必須条件にする。

文書ごとに正本を一つにする。変換の審査前はMarkdown、審査と受入後は.nepldを正本とし、手書きのMarkdownとDocを並行保守しない。GitHubが必要とするREADME、AGENTS、CONTRIBUTING、SECURITY、CODEX等のMarkdown入口は、移行後の正本から生成する閲覧用artifactとする。生成物は正本path・renderer版を示し、差分検査する。自動生成できる契約が完成するまで現在の入口を除去しない。

`doc/history/` の元manifest・検査報告等の保存byte列、LICENSE、JSON/schema、言語source、例、CI/templateの機械契約は歴史・機械入力として保持する。これは文書本文をMarkdownで二重保守する例外ではない。生成task文書もタスク定義の正本を維持し、最終的にはDocと必要なMarkdown閲覧物を同じ正本から生成する。

rustdocはRust sourceのdoc commentを正本とし、同じAPI解説を手書きDocへ複製しない。inventoryには包含・除外の理由を記録し、rootのGitHub/agent用入口を移行範囲から暗黙に落とさない。

## 2. 表現能力の監査が変換の前提

inventoryとgap auditはT21まで延期せず、Docに関係するT01の共通値・位置、T02のwire/schema、T07の文書意味APIを固定する前に実施する。対象ページ、原本commit/path/byte digest、意味要素、page ID、公開URL/旧URL、anchor、参照、コード例、表、箇条書き、図、数式、メタ情報、release対応を確認する。初回の具体的な監査は [文書inventoryとgap audit](../doc-inventory.md)、機械データは `design/doc-inventory.json` に記録した。

初回inventoryはcommitで固定した57 Markdownページの監査であり、現在の全ページの受入ではない。現在との差分は `doc-inventory --check` が追加・変更・削除として出力する。新しいAPI判断時にはその差分も確認し、現在の網羅性を根拠にする場合はcommit済みの新baselineへ更新して `doc-inventory --check-current` を通す。inventory自身のJSON、実行証拠、生成taskの更新によるhash循環を避けるため、HEADのcommit hashを現在の成果物へ自己参照させない。

早期監査の成果は、必要な意味情報と既存契約の不足を把握してAPI設計へ反映することである。T01/T02/T07にT21完了やbackend完成を前提として追加しない。R014全体の解消には後続schema・文法・backend・wire・conformanceの実装と検証が必要で、T21は早期inventoryを更新しながらページ変換を進める。現在のDocはsentence、節、Ruby/Anno、parallel、内部ref、Math/Circuit/4言語Codeを持つが、既存Markdownの全表現を収容する契約はまだない。

| 要素 | 現行契約と移行前に解消する課題 |
| --- | --- |
| 表・箇条書き | 意味型・文法・lower/print/portable境界は実装済み。移行先HTMLの構造・accessibilityと元cell/list情報の同等性を検証する。段落へ平坦化しない |
| 文書間・外部リンク | リンクの意味型と文書内ラベル検査は実装済み。page registryを使った文書間解決、URI・assetの検査、配布後のリンク検証を接続する |
| 汎用コードblock | 非評価の汎用コード構造と4言語ForeignSyntaxを区別する型・文法は実装済み。Rust、shell、JSON等の元byte列をHTML・移行projectionまで保存する |
| 図・画像・Mermaid等 | Circuit図だけでは一般の設計図を表せない。許可asset、digest、caption/代替説明、変換とsource対応を定める |
| 見出し・anchor・脚注 | 明示ID、旧anchorへの互換、脚注と参照の意味を確認し、安定URLを維持する |

不足はR014の設計blockerとする。schema・forms・syntax source・backend・wire・conformanceを一つの変更で整備し、独立レビューで解消してから該当ページを変換する。ここでは未定義constructorを追加したふりをせず、任意RawHtmlを抜け道にしない。等価な既存表現を選ぶ場合も、意味とaccessibilityの同等性を独立に確認する。

## 3. ページ単位の切替

移行原稿は [執筆指針](../authoring.md) に従う。本文は著者が定めた文単位のSentenceを
保ち、Text・Ruby・Annoだけで表せる文はsentence literal、参照・強調・code・break等を
含む文は明示的なsentence構築を選ぶ。日本語Rubyは漢字部分、語句全体の訳注はAnnoへ
置き、多言語の対応は文単位のparallelにする。変換器が段落を一つのSentenceへまとめた
候補は、対応単位を確認するまで正本にしない。句点だけによる自動分割や、互換出力の
未対応を理由とした注釈・Sentence境界の削除で移行を通さない。

1. inventoryとgap auditを承認可能な差分として作り、変換元のcommit・path・byte digestを固定する。
2. parser/meaning/backendの不足を埋め、失敗系・roundtrip・wire・表示の受入を実行する。
3. 同一page IDとURLを持つDoc sourceを生成・編集し、内容の落ち、表の対応、リンク、anchor、コードbyte列、数式構造、図の説明を独立に比較する。
4. 独立した意味同等性レビュー、HTML構造/accessibility、全リンク、同版例、決定的buildが通ったページのregistry正本をDocへ切り替える。
5. 旧Markdownは削除またはDocからの生成artifactへ変更し、二重編集を検出する。未移行ページはMarkdownを唯一の正本のままにする。

切替前に `task.spec`、operation.definition、acceptance catalogのspec参照とcheckerのMarkdown ID抽出を点検する。format中立のcanonical page IDへ移すか、Doc正本から検査済みMarkdown互換projectionを生成して既存readerへ渡す契約を実装する。単に.mdを削除して参照を壊さない。projectionだけを手で修正した場合は生成差分検査で拒否する。

意味同等性レビューは、執筆者とは別の担当が原文・執筆指針・変更原稿・実行結果を読んで行う。ユーザーが許可した独立subagentによるレビューも、この条件を満たす方法として使用する。subagentによる文面の確認、自動accessibility tree検査、人によるscreen reader実操作を別々に記録し、実施していない人の操作を成功としない。

### 正本registryと生成物の検査

`doc/canonical.json` のversion 1は、pageごとの安定したid、Doc source、生成projection、alias JSON、HTML route、renderer識別を持つ。未登録ページは移行済みと推定しない。sourceは.nepld、projectionは既存の.mdの場所とし、JSON自体は文書の内容を所有しない。旧原稿を移動した後に、同じDoc本文の手書き複製を移行候補ディレクトリへ残さない。

version 1のregistry pathとrouteはASCII英数字・hyphen・underscore・dotを持つcomponentをslashで結ぶ。空component、末尾dot、Windows予約名を拒否し、case-insensitiveに重複を検査する。registryのファイルはdoc/配下とする。これはrepositoryの生成先と入力名を複数OSで同じように扱うためのhost側の制約であり、Doc本文やfoundationのSource URIからUnicodeを除く規則ではない。fragment/percent表記をphysical inputへ混ぜず、Markdown検査だけ成功して同じsourceのHTML生成がpath違反になる不整合を防ぐ。

`nepl3-tools doc-canonical --check` は実際のDoc parser/lowerと検査済みannotated rendererでMarkdownを再生成し、source path・source digest・alias入力digest・document digestを含めたbyte列を既存projectionと比較する。手編集・古いsource・aliasの変更・異なる改行を黙って正規化せず、不一致で失敗する。未知のrenderer、重複identity/path、repository外のpath、symlink、過大入力も拒否する。この検査は明示的な文書生成段階であり、metadataだけを検査する `nepl3-tools check` やCargo build.rsへ混ぜない。

`nepl3-tools doc-canonical html <new-directory>` は同じregistryのDoc sourceを既存のpage-set generatorへ渡し、検査済みHTML・CSS・manifestを新しいディレクトリへ書く。relative linkの論理namespaceは従来のMarkdown path、physical inputはDoc sourceとして分けて記録する。生成MarkdownをHTML生成の入力にしない。失敗後に部分的な成功を返さず、既存の出力も上書きしない。

### ページ間参照を持つMarkdown projection

`nepl3-tools.markdown-annotated-pages/1` は、登録されたDoc間の参照を扱う別のrenderer識別子とする。
registry全体と各source・aliasのbyte列を一度読み、同じ固定入力から全ページを生成する。
page idを解析sourceの名前空間へ与え、論理sourceとMarkdown出力routeにはprojection pathを使う。
全ページの意味参照と実際に出力するanchorを検査し、全体が成功するまで成果物を返さない。
未登録のMarkdownをfilesystemから発見して補完することはない。この入口にpassive file登録はない。

旧 `nepl3-tools.markdown-annotated/2` と混在できる。旧ページは従来のsource名前空間と生成処理で
metadataを含めて同じbyte列を作り、ページ集合からの生成bodyとも一致することを要求する。
この照合により、新しいページの参照先anchorは旧ページの実際のprojectionにも存在する。
旧rendererだけの `--check` は従来のページごとに独立した処理・予算を維持する。

新rendererのmetadataにはページ自身のsource・alias・Document digestに加え、input PageSet digestと
input context digestを記録する。後者は以下のfieldをこの順に、各fieldのbyte長をu64 big-endianで
前置して連結し、SHA-256を求める。文字列はUTF-8、digest fieldは32 byteの値とする。

1. `nepl3.canonical-input-context/1` と末尾zero byte、renderer識別子、registry path、registryの元byte列。
2. input PageSet digest、ページ数を表すu64 big-endianの8 byte。
3. 登録順に各ページのid、physical source path、projection path、alias path、HTML route、renderer識別子。
4. 各ページの直前の6 fieldに続けて、sourceの元byte列のSHA-256、aliasの元byte列のSHA-256。

これは入力の同一性であり、完成した配布物のdigestではない。registryの空白・登録順・別ページの
alias入力だけの変更も、新rendererの生成物を古くする。生成Markdownのbyte列を入力digestへ
再帰的に含めない。旧rendererのmetadataへ、この新しい集合identityを後付けしない。

`nepl3-tools doc-canonical markdown <new-directory>` は、全ページの検証後にprojection pathを
保った新しい出力ディレクトリへ書き出す。最後のmanifestには出力byte数とdigestを記録する。
`--check` と同じ生成経路を使い、repository内の正本や古いprojectionを自動更新しない。
既存ディレクトリを拒否する。意味検査・生成の失敗では書込みを開始せず、OSの書込み失敗では
不完全な新規ディレクトリが残り得るが、成功のcompletion manifestを作らない。

新rendererを含む検査とMarkdownの集合書出しは128ページ、source合計10,000,000 byte、
alias合計1 MiB、registry 1 MiB、生成Markdownは1ページ2 MiBを上限とする。
読み込むhost入力はこの上限で制約する。parseとlowerは各ページの独立した有限operationとし、
resolve・render・anchor照合・digest・metadata・出力コピーには一つの消費済み状態を保持する
出力Budgetを用いる。旧rendererの混在ページを再生成するときも、この出力Budgetを共有する。
registryの任意field `output_limits` は既存のOutputLimitsと同じ8資源の上限を持ち、
集合の処理開始前に選択する。未指定なら既定のDoc host allowanceで開始する。
停止後の再試行・暗黙の増額・別予算による継続はしない。旧rendererだけの検査では
この集合設定を用いず従来のページ単位のoperationを維持する。出力manifestには実際に
選択した上限と開始時usage、receipt serialize直前のusageを別fieldで記録する。
最後のusageはreceipt自体のserialize・最終出力課金を含まない値として明示する。
host JSON・文字列整形の計量は保守的な論理allocation allowanceであり、物理heapの計測ではない。

HTML集合書出しの予算は、registryの任意field `html_output_limits` で別に選択する。
指定時はOutputLimitsの8資源をすべて記述し、null・部分指定・未知field・負数は拒否する。
未指定なら既存のHTML出力予算を維持し、Markdown用の `output_limits` を流用しない。
生成開始前に選んだ一つのBudgetを、既存のPageSetのresolve・render・serializeへ渡す。
各ページのparse・lowerは従来どおり独立した有限operationであり、HTML用設定で増額しない。
停止後の再生成やMarkdown予算へのfallbackは行わず、書込み開始前の停止では出力先を作らない。
HTML manifestの既存execution identity・選択上限・開始時usageへ実際の設定を記録する。
このfieldを追加したregistryの元byte列は、新Markdown profileのinput contextにも反映される。

互換aliasとGitHubの自動見出しIDは独立して発生するため、実際のGitHub表示で重複と移動先を確認する。13章では旧8見出しのうち6つを明示alias、`1-digest` と `3-normal-formとartifact` を同名の自動見出しで維持する。全7Sectionの明示IDも保持する。この判断は13章の実際の見出しに対する対応表であり、任意のMarkdownに対するslug推測をfoundationへ追加するものではない。

文書をrenderしただけで埋め込まれた例を評価しない。失敗を説明する例や注釈内のcodeはsource表示のまま保存する。リンク先の例を試す操作も例ID・revision・profileを照合する。新しい文法の受入が通る前に文書を新constructorへ一括変換しない。

## 4. Bootstrapと再現性

文書生成をCargo build.rsやcompilerのbuild dependencyへ入れない。言語packageのbootstrapと文書サイトの生成を別の明示tools段階にする。compilerをbuildするのに新compilerで文書を読む必要がある循環を作らない。

compiler開発で現行rendererが壊れた場合にも仕様を読めるよう、最後に動作検証済みの文書renderer/package/assetをversionとdigestで固定して利用できる。新しいDoc sourceはそのrendererで受理できる版かを確認し、非互換なら停止して互換なrendererの確立を先に行う。失敗後に黙って旧rendererへfallbackしない。明示選択した旧rendererでの文書buildと、同revisionの処理系がDocを正しく扱うruntime受入を別の結果として記録する。

配布済み文書snapshotの閲覧にcompilerを要求しない。過去releaseの文書・asset・例はそのreleaseのidentityで固定する。source、renderer、schema、registry、assetが同じなら生成byte列と意味構造が再現できることを検査し、時刻やnetworkによる差を持ち込まない。

## 5. 完了条件

J01〜J04がすべてpassedで、inventoryの対象ページがDoc正本へ移り、機械入力・byte保存履歴を除く手書きMarkdown本文の二重管理がなくなった時点をT21の完了とする。変換率、未対応表現、未移行ページ、未実行試験を記録し、サイトが表示できることだけで移行完了としない。
