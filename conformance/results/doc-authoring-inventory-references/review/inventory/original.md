# 文書inventoryとDoc表現能力の早期監査

この監査は、Docの意味モデルと交換schemaを設計する前に、実在する文書が必要とする情報を確定するための入力である。Docへの変換やR014の解消、J01〜J04の受入成功を示すものではない。

## 監査した原本と分類

baselineは `25a096bc183c2b71200902884084cd4082d0aae3`。全trackedファイルを列挙し、57 Markdownページ、10 Rust source、その他の機械入力・履歴・assetを包含理由または除外理由付きで分類した。各ページとRust sourceのpath・SHA-256・byte長、および5つのDoc関連契約のSHA-256は [design/doc-inventory.json](../design/doc-inventory.json) にある。

| 分類 | 件数 | 正本と扱い |
| --- | ---: | --- |
| rootのGitHub/agent向け入口 | 5 | README、AGENTS、CODEX、CONTRIBUTING、SECURITY。現在は著者が編集するMarkdown。将来も入口を維持し、Docから生成するには先に対応契約が必要 |
| doc内の著者文書 | 25 | `doc/history/README.md` も変更可能な説明文書として含む。履歴JSONと混同して除外しない |
| signature表 | 4 | forms由来と宣言されているが、現在のtoolsに表の再生成器はない。再現可能な生成物として承認済みとは扱わない |
| taskの生成文書 | 22 | 21 taskと索引。正本はtasks/status JSONで、現行toolsの生成対象。Doc移行時もJSONからの生成を維持 |
| GitHub PR template | 1 | checkboxとリンクを含む運用template。プラットフォーム入力としてMarkdownを維持する明示的な除外 |
| Rust API解説の所有元 | 10 source | doc commentの正本はRust source。source digestを記録しただけで、rustdocの全API抽出・リンク・表示を検証したとはしない |

`doc/history/design-validation.json`、`source-manifest.json`、`web-tea-import.json` は元byte列を保存する履歴である。LICENSE、schema/config、言語source、例もMarkdown本文へ変換せず、元の契約とidentityを維持する。`.tmp/`、target、Git管理外の説明資料は監査入力にしない。

## 観測した要素

固定したpulldown-cmark 0.13.4で、CommonMarkにtables、footnotes、strikethrough、tasklists、GFM alerts、mathを加えた設定を使う。独自Markdown parserではない。完全なGitHub表示互換、未定義reference link、裸URLの自動link、HTMLの意味、Rustdoc、将来のDoc rendererまで検査するものではない。[parserの公式仕様](https://docs.rs/pulldown-cmark/0.13.4/pulldown_cmark/struct.Parser.html) と [拡張設定](https://docs.rs/pulldown-cmark/0.13.4/pulldown_cmark/struct.Options.html) に従う。

| 要素 | baselineでの観測 | 具体例 |
| --- | ---: | --- |
| 見出し | 278 | 全57ページ。見出し文字列から恒久IDを推測しない |
| 表 | 44表・1089 cell | 四言語signature、CLI/操作表、受入・状態の対応表 |
| 箇条書き | 37 list・223 item | 手順、禁止事項、受入条件。paragraphのネストだけではlistの意味を保てない |
| task checkbox | 3 | PR templateの運用確認 |
| inline code | 695 | 型、field名、path、CLI、例の断片 |
| code block | 11 | sh 4、PowerShell 1、JSON 1、text 5。説明用文法・依存図を含む |
| link | 203 | root/doc間、例への参照、外部一次資料、badgeのリンク |
| image | 1 | READMEの外部CI badge。通常文書画像と運用projectionの方針を分けて判断する |
| strong | 3 | Doc.Strongが既に表現する範囲。ただし未実装 |
| HTML comment | 25 | 生成元表示など。任意RawHtmlをDoc coreへ追加する根拠にしない |
| HTMLとして読まれる断片 | 4 | foundationの裸の`<T>`が2箇所、Mathの裸の`<Q>`が2箇所 |

最後の4箇所はgeneric型の説明であり、HTMLの意図ではない。現在の `02-foundation.md` と `06-math.md` をinline codeへ訂正した。baselineは元のまま保持し、訂正はcurrent deltaへ出る。blockquote、脚注、strikethrough、TeX mathはこのbaselineでは観測0。将来の候補を実在ページの必須要素として水増ししない。

各code blockにはinfo文字列、raw source範囲とdigest、parserが返した内容のdigest/byte長を記録した。inline codeにもraw範囲とdigestがある。sourceはUTF-8 byte範囲であり、見た目の文字数ではない。JSONや説明用prefix例を評価せず、空白・改行や不完全例を保持する必要がある。リンク先の例もコピーせず元source identityを参照する。

## 見出し・リンク・画像altのinline投影契約

inventory schemaは `nepl3.doc-inventory/2`、inline抽出profileは `nepl3.markdown-inline-projection/1` とする。従来の抽出ではlink labelの改行が失われて単語が結合し、見出しとlink labelからmathが欠落した。修正に伴いschemaを更新し、同じbaseline commitから明示的に再生成した。parser版・元ファイルのdigest・対象ページは変更していない。

見出しの `text` とlink/imageの `label` は共通の投影処理で作る。それぞれに `inline_segments` を保持し、各segmentは `content.kind`、必要な場合の `content.value`、元sourceのUTF-8半開byte範囲 `source_bytes` を持つ。入れ子のimageに含まれる内容は、そのimage、外側のlink、包含する見出しへ同じ順序で一度ずつ反映する。

| kind | value | 要約文字列への投影 |
| --- | --- | --- |
| text / code | parserが返す内容 | 内容をそのまま追加。codeの元backtick等はsource範囲で保持 |
| soft-break | なし | 1個のspace |
| hard-break | なし | 1個のLF。soft-breakと同一視しない |
| inline-math | delimiterを除くmath payload | `$payload$` |
| display-math | delimiterを除くmath payload | `$$payload$$` |
| footnote-reference | footnote label | `[^label]`。表示番号を推測しない |

typed segmentを持つため、要約文字列に同じ`$`表記が現れても、通常Textとmathを混同せずに比較できる。CRLF、hard breakの2個のspaceやbackslash、非ASCII文字を含む元byte列はsegment範囲から取得できる。parserによるdecodeやcode内空白の正規化を、元sourceの書換えと見なさない。

これはinventoryの要約投影であり、rendererのplain text抽出や完全なMarkdown ASTではない。emphasis等のstyleタグはここで再構築しない。inline HTML/tag/commentは要約文字列へ解釈して追加せず、既存の `html_fragments` にliteralとsource範囲を保持する。例えば`<br>`をhard-breakへ推測変換しないため、HTMLを含むlabelの意味は要約だけでは判定できない。脚注解決、数式評価、altの実表示、リンクの有効性も別の監査対象である。

## 既存Doc契約との差分

監査の照合先はbaselineのforms、Doc syntax、model、contracts、markupである。以下の項目は必要情報と不足の記録であり、新constructorの承認済みsignatureではない。

| gap ID | 観測と必要な情報 | 現在の不足・設計への影響 |
| --- | --- | --- |
| DG01 | 表の行/cell順、header、列対応・alignment、cell内inline/参照 | Docにtable/cell型がなく、HTML allowlistにもtable/thead/th/tdがない。T01のtyped field/schema方針、T02の順序付き交換値、T07の文書型へ影響 |
| DG02 | ordered/unordered、開始番号、itemの入れ子、checkbox状態 | Paragraphの再帰だけではlistやtaskの意味を保持できず、ol/ul/li/inputも未定義。運用templateをprojectionとして残す選択も明記する |
| DG03 | page ID、相対/外部URI、fragment、表示label、許可scheme、存在検査 | Doc.Referenceはarticle内のNameだけ。markupのhrefは同artifact内のFragmentHrefだけ。page registry・resource解決・安全なURI契約が必要 |
| DG04 | 非評価のinline/block code、任意言語hint、元byte列、空白、Origin | Doc.Codeは四言語のForeignSyntax用。sh/PowerShell/JSON/図/EBNF/型断片をtokenizeや意味検査なしに表せる契約がない。T02のpayloadとT07のlower/printに影響 |
| DG05 | badge/画像assetのdigest、alt/caption、リンク先、許可入力 | CircuitFigureで一般画像を代用できず、img/SVG外部資源の契約もない。badgeをGitHub専用projection metadataとして保持するか、一般画像を設計するかを先に決める。裸genericはinline code訂正で扱う |
| DG06 | stable page ID、見出し階層、旧URL/anchorとの対応・衝突 | 現行Anchor/SectionのHTML idは`n-`+UTF-8 hexでGitHub見出しslugと異なる。各ページのID/公開URLは未割当として記録し、架空のURLや互換保証を作らない |
| DG07 | 数学的構造と元記述の対応 | baselineのTeX math観測0。現行Math bridgeは存在する設計だが、将来のTeX→Math変換を暗黙に保証しない |
| DG08 | strikethrough等の追加inline分類 | baseline観測0。必要ページが追加されたときに再監査し、現在の必須constructorに勝手に追加しない |
| DG09 | 生成元/renderer/schema版、正本path、生成済み表示 | 25 commentのうち意味本文と生成provenanceを分ける。メタ情報を消さず、RawHtmlや二重正本を導入せずgenerator/registryで扱う |

Doc.Text、Emphasis、Strong、Sentence、再帰Paragraph、Section、Ruby/Anno、parallelは既存の意味領域として利用できる。ただし「仕様に型がある」と「lower/check/print/wire/backendが検証済み」は別である。

R006では上述の意味値、source/Origin/resource bundle、操作入出力、field順序・variant識別を閉じたschemaへ整合させる必要がある。R009では実際のschema digest・provider manifestを含む解決済みProfileと文書rendererの版を固定する必要がある。inventoryに新しいDoc型や架空のdigestを足すことで解消したことにはしない。

T01/T02/T07はこの早期監査とcurrent deltaを読んでからDoc関連の公開契約を固定する。監査の確認はT21やT10 backendの完成を依存条件にしない。R014全体は、該当schema/forms/syntax/backend/wireと移行conformanceが同じ変更で整い、独立レビュー・実行検証が揃うまでopenである。

## 再現・更新と現在との差分

```sh
cargo run --locked -p nepl3-tools -- doc-inventory --check
cargo run --locked -p nepl3-tools -- doc-inventory --check-current
cargo run --locked -p nepl3-tools -- doc-inventory --write --commit 25a096bc183c2b71200902884084cd4082d0aae3
```

`--check` は完全な40桁commitからGit blobを再読込し、ページ欠落、digest、構造、分類の変更を保存inventoryと照合する。その後、現在のGit-visible Markdown、Rust source、Doc関連契約について追加・変更・削除を出力する。baselineが正しければcurrent deltaがあっても成功するが、現在の網羅性は未確認と明記する。`--check-current` はdeltaがあれば失敗する。通常のrepository `check` は前者を実行する。

この変更で追加した監査文書、訂正した仕様、tools、生成taskはbaselineに存在しないか内容が異なるので、現在のstrict確認は失敗するのが正しい。新しいAPI判断で現在の網羅性を主張する前に、対象の文書・契約をcommitし、そのcommitで `--write`、差分の独立レビュー、`--check-current` を実行する。新baselineを記録するJSON自身はMarkdown/Rust/API仕様のcurrent比較対象ではないため、自己hash更新を要求しない。生成taskは過去commitのbyte列を読み、現在のgeneratorを動かしてbaselineを書き換えない。

baseline commitを含む履歴が必要で、取得できない環境は失敗する。CIでは履歴を取得する。書き出したsource archiveだけでGit blob監査を実行したとは扱わない。抽出器の変更やparser版更新も新しい監査としてレビューする。見出しslug、文意、accessibility、将来Docの表示・wire等価性は別の手動/実行受入で確認する。
