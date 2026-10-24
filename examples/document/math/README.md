<!-- Generated from examples/document/math/README.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page math&#45;example&#45;guide; source SHA-256 046f032411f76abec049447c5c90e736654039c475ad6b000bd0e6d3f5f7f07f; alias input SHA-256 f30c05bb1197c1a613b457d59cdb2d84d91f421267824388f3de7455a5bd1efc; page input SHA-256 7ac950f979f8d3624a0fe5e4f29944ef85f30dad11952b0a141e4b8dbd9182cf. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="github-markdown向けmathの例"></a>

# GitHub Markdown<ruby>向<rt>む</rt></ruby>けMathの<ruby>例<rt>れい</rt></ruby>

[正本（NEPL3d）](<README.nepld>)

<ruby>例<rt>れい</rt></ruby>の<ruby>正本<rt>せいほん</rt></ruby>は `markdown.nepld`。<ruby>閲覧用<rt>えつらんよう</rt></ruby>の `markdown.md` は<ruby>次<rt>つぎ</rt></ruby>の<ruby>操作<rt>そうさ</rt></ruby>で<ruby>生成<rt>せいせい</rt></ruby>する。

```sh
cargo run --locked -p nepl3-tools -- doc-markdown footnotes-pages examples/document/math/markdown.pages.json /tmp/nepl3-math-example
```

<ruby>本文<rt>ほんぶん</rt></ruby>・Docの<ruby>脚注<rt>きゃくちゅう</rt></ruby>・display block・table cellの<ruby>数式<rt>すうしき</rt></ruby>を<ruby>扱<rt>あつか</rt></ruby>う。Math<ruby>内部<rt>ないぶ</rt></ruby>のSentence<ruby>注釈<rt>ちゅうしゃく</rt></ruby>など、<ruby>構造<rt>こうぞう</rt></ruby>TeXで<ruby>保持<rt>ほじ</rt></ruby>できない<ruby>内容<rt>ないよう</rt></ruby>は<ruby>拒否<rt>きょひ</rt></ruby>する。Mathの<ruby>評価<rt>ひょうか</rt></ruby>は<ruby>行<rt>おこな</rt></ruby>わず、GitHub<ruby>側<rt>がわ</rt></ruby>のMathJaxによる<ruby>表示<rt>ひょうじ</rt></ruby>を<ruby>利用<rt>りよう</rt></ruby>する。<ruby>生成物<rt>せいせいぶつ</rt></ruby>の<ruby>構造検査<rt>こうぞうけんさ</rt></ruby>は、GitHubでの<ruby>実描画<rt>じつびょうが</rt></ruby>・アクセシビリティ<ruby>検証<rt>けんしょう</rt></ruby>を<ruby>代替<rt>だいたい</rt></ruby>しない。
