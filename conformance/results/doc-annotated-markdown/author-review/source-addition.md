`nepl3-tools doc-markdown annotated <source.nepld> <aliases.json> <new-file.md>` は、
Doc正本の読み・注記をGitHub等の閲覧用Markdownへ明示する別profileである。
既存の `doc-markdown` の互換規則は変更しない。renderer識別子は
`nepl3-tools.markdown-annotated/1` とする。

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

Bodyには導入のParagraph/RawCodeと、その後にSection列を置ける。SectionのBodyも
同じ規則で再帰的に扱い、Articleの見出しを第1段として第6段まで生成する。
同じBodyでSectionの後に通常blockが現れる場合は、Markdownで所属が変わるため拒否する。
RawCodeのbyte列・末尾LF・hint・fenceの制約は既存projectionと共通である。
この初期profileはlist、table、Parallel、画像、参照、inline Anchorを扱わない。
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