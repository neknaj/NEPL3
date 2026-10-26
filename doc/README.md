<!-- Generated from doc/README.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page documentation&#45;index; source SHA-256 d3739b817aaae7d31cbfdb6a65b6277b0bbb5f6cd15ccdd6d94c74a03a1c7568; alias input SHA-256 322fb47ddef121f15a46c7f2dd4f1ba6af5baba49af2d973d5ab96a8491b9ba7; page input SHA-256 1a6ea77c5fd510dc64720d5623fea6e06c2946215d38c5fc17c919bb45b309f1. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="nepl3-ドキュメント"></a>

# NEPL3 ドキュメント

[正本（NEPL3d）](<README.nepld>)

<ruby>正式<rt>せいしき</rt></ruby>な<ruby>仕様<rt>しよう</rt></ruby>・<ruby>開発情報<rt>かいはつじょうほう</rt></ruby>の<ruby>入口<rt>いりぐち</rt></ruby>です。<ruby>会話<rt>かいわ</rt></ruby>やローカルの `.tmp/` を<ruby>参照<rt>さんしょう</rt></ruby>しなくても、ここから<ruby>実装契約<rt>じっそうけいやく</rt></ruby>と<ruby>現在<rt>げんざい</rt></ruby>の<ruby>状態<rt>じょうたい</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>できます。

[<ruby>正本<rt>せいほん</rt></ruby>registry](<canonical\.json>)に<ruby>登録<rt>とうろく</rt></ruby>したページはnepldを<ruby>編集<rt>へんしゅう</rt></ruby>し、Markdownは<ruby>生成物<rt>せいせいぶつ</rt></ruby>として<ruby>扱<rt>あつか</rt></ruby>います。[<ruby>再現性<rt>さいげんせい</rt></ruby>の<ruby>仕様<rt>しよう</rt></ruby>](<https\:\/\/github\.com\/neknaj\/NEPL3\/blob\/main\/doc\/spec\/13\-reproducibility\.nepld>)はこの<ruby>方式<rt>ほうしき</rt></ruby>へ<ruby>移行<rt>いこう</rt></ruby>し、<ruby>既存<rt>きそん</rt></ruby>のMarkdownのURLも<ruby>生成<rt>せいせい</rt></ruby>projectionで<ruby>維持<rt>いじ</rt></ruby>します。<ruby>未登録<rt>みとうろく</rt></ruby>ページは<ruby>引<rt>ひ</rt></ruby>き<ruby>続<rt>つづ</rt></ruby>きMarkdownが<ruby>正本<rt>せいほん</rt></ruby>です。`migration/authored/` の<ruby>原稿<rt>げんこう</rt></ruby>が<ruby>存在<rt>そんざい</rt></ruby>するだけでは<ruby>移行済<rt>いこうず</rt></ruby>みとしません。

Docで<ruby>例<rt>れい</rt></ruby>や<ruby>解説<rt>かいせつ</rt></ruby>を<ruby>書<rt>か</rt></ruby>く<ruby>際<rt>さい</rt></ruby>は、[<ruby>文書<rt>ぶんしょ</rt></ruby>の<ruby>執筆指針<rt>しっぴつししん</rt></ruby>](<authoring\.md>)のsentence literal・<ruby>前置構築<rt>ぜんちこうちく</rt></ruby>・parallel・Ruby\/Annoの<ruby>使<rt>つか</rt></ruby>い<ruby>分<rt>わ</rt></ruby>けに<ruby>従<rt>したが</rt></ruby>ってください。

<a name="n-707572706f7365"></a>

<a name="目的から探す"></a>

## <ruby>目的<rt>もくてき</rt></ruby>から<ruby>探<rt>さが</rt></ruby>す

| したいこと | <ruby>最初<rt>さいしょ</rt></ruby>に<ruby>読<rt>よ</rt></ruby>む<ruby>資料<rt>しりょう</rt></ruby> | <ruby>確認<rt>かくにん</rt></ruby>できること |
| --- | --- | --- |
| NEPL3を<ruby>理解<rt>りかい</rt></ruby>する | [README](<\.\.\/README\.md>)、[<ruby>共通契約<rt>きょうつうけいやく</rt></ruby>](<spec\/00\-contract\.md>)、[アーキテクチャ](<spec\/01\-architecture\.md>) | <ruby>多階層<rt>たかいそう</rt></ruby>の<ruby>言語<rt>げんご</rt></ruby><ruby>埋<rt>う</rt></ruby>め<ruby>込<rt>こ</rt></ruby>みと、<ruby>基盤<rt>きばん</rt></ruby>・<ruby>各言語<rt>かくげんご</rt></ruby>の<ruby>所有境界<rt>しょゆうきょうかい</rt></ruby> |
| <ruby>実在<rt>じつざい</rt></ruby>する<ruby>小<rt>ちい</rt></ruby>さな<ruby>言語<rt>げんご</rt></ruby>を<ruby>試<rt>ため</rt></ruby>す | [Hello](<tutorial\/hello\.md>)、[MiniExpr](<tutorial\/miniexpr\.md>)、[<ruby>二言語<rt>にげんご</rt></ruby>のcomposition](<tutorial\/composition\.md>)、[<ruby>外部<rt>がいぶ</rt></ruby>consumer](<\.\.\/conformance\/extensions\/hello\/README\.md>) | <ruby>入力<rt>にゅうりょく</rt></ruby>と<ruby>宣言<rt>せんげん</rt></ruby>を<ruby>変更<rt>へんこう</rt></ruby>し、<ruby>再帰構文<rt>さいきこうぶん</rt></ruby>・<ruby>言語境界<rt>げんごきょうかい</rt></ruby>・UTF\-8<ruby>位置<rt>いち</rt></ruby>・<ruby>診断<rt>しんだん</rt></ruby>を<ruby>観察<rt>かんさつ</rt></ruby>する |
| <ruby>構文<rt>こうぶん</rt></ruby>・<ruby>交換契約<rt>こうかんけいやく</rt></ruby>を<ruby>調<rt>しら</rt></ruby>べる | [Foundation](<spec\/02\-foundation\.md>)、[reader](<spec\/03\-reader\.md>)、[Grammar](<spec\/04\-grammar\.md>)、[<ruby>交換<rt>こうかん</rt></ruby>](<spec\/09\-portability\.md>)、[<ruby>統合<rt>とうごう</rt></ruby>](<spec\/10\-integration\.md>) | <ruby>入出力<rt>にゅうしゅつりょく</rt></ruby>・shape・source・<ruby>失敗条件<rt>しっぱいじょうけん</rt></ruby>。schema<ruby>正本<rt>せいほん</rt></ruby>は<ruby>下表<rt>かひょう</rt></ruby> |
| <ruby>文書<rt>ぶんしょ</rt></ruby>・<ruby>数式<rt>すうしき</rt></ruby>を<ruby>扱<rt>あつか</rt></ruby>う | [Doc](<spec\/05\-document\.md>)、[Math](<spec\/06\-math\.md>)、[Sentence\/annotation](<spec\/23\-sentence\-annotation\.md>)、[<ruby>執筆指針<rt>しっぴつししん</rt></ruby>](<authoring\.md>) | <ruby>意味<rt>いみ</rt></ruby>モデルと<ruby>文章表記<rt>ぶんしょうひょうき</rt></ruby>。Sentenceの<ruby>契約<rt>けいやく</rt></ruby>とconsumer<ruby>移行状態<rt>いこうじょうたい</rt></ruby>は<ruby>区別<rt>くべつ</rt></ruby>する |
| HTML・<ruby>公開<rt>こうかい</rt></ruby>の<ruby>境界<rt>きょうかい</rt></ruby>を<ruby>調<rt>しら</rt></ruby>べる | [<ruby>数式表示<rt>すうしきひょうじ</rt></ruby>](<spec\/17\-math\-html\.md>)、[markup](<spec\/19\-html\-fragment\.md>)、[Doc HTML](<spec\/20\-doc\-html\.md>)、[ページ<ruby>参照<rt>さんしょう</rt></ruby>](<spec\/21\-doc\-pages\.md>)、[site](<spec\/15\-site\.md>) | <ruby>各層<rt>かくそう</rt></ruby>の<ruby>独立<rt>どくりつ</rt></ruby>した<ruby>保証<rt>ほしょう</rt></ruby>。HTMLは<ruby>正式文書<rt>せいしきぶんしょ</rt></ruby>のprojection |
| <ruby>開発<rt>かいはつ</rt></ruby>・<ruby>検査<rt>けんさ</rt></ruby>する | [<ruby>開発手順<rt>かいはつてじゅん</rt></ruby>](<development\.md>)、[<ruby>受入条件<rt>うけいれじょうけん</rt></ruby>](<spec\/11\-conformance\.md>)、[モデル<ruby>不変条件<rt>ふへんじょうけん</rt></ruby>](<spec\/12\-model\-invariants\.md>)、[<ruby>再現性<rt>さいげんせい</rt></ruby>](<spec\/13\-reproducibility\.md>) | <ruby>変更<rt>へんこう</rt></ruby>に<ruby>対応<rt>たいおう</rt></ruby>する<ruby>検査<rt>けんさ</rt></ruby>と、<ruby>正式受入<rt>せいしきうけいれ</rt></ruby>との<ruby>区別<rt>くべつ</rt></ruby> |
| <ruby>編集先<rt>へんしゅうさき</rt></ruby>・<ruby>現在状態<rt>げんざいじょうたい</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する | [canonical registry](<canonical\.json>)、[<ruby>実装状態<rt>じっそうじょうたい</rt></ruby>](<\.\.\/implementation\-status\.json>)、[タスク<ruby>索引<rt>さくいん</rt></ruby>](<\.\.\/tasks\/README\.md>) | <ruby>正本<rt>せいほん</rt></ruby>、<ruby>実行済<rt>じっこうず</rt></ruby>み<ruby>範囲<rt>はんい</rt></ruby>、<ruby>依存<rt>いぞん</rt></ruby>する<ruby>成果物<rt>せいかぶつ</rt></ruby>と<ruby>残件<rt>ざんけん</rt></ruby> |

<ruby>個別<rt>こべつ</rt></ruby>の<ruby>設計対象<rt>せっけいたいしょう</rt></ruby>は [Circuit](<spec\/07\-circuit\.md>)、[editor](<spec\/08\-editor\.md>)、[Web UI](<spec\/14\-web\-ui\.md>) を<ruby>参照<rt>さんしょう</rt></ruby>してください。<ruby>文書移行<rt>ぶんしょいこう</rt></ruby>の<ruby>条件<rt>じょうけん</rt></ruby>は [<ruby>第<rt>だい</rt></ruby>16<ruby>章<rt>しょう</rt></ruby>](<spec\/16\-doc\-migration\.md>) にあります。<ruby>仕様全章<rt>しようぜんしょう</rt></ruby>や<ruby>過去<rt>かこ</rt></ruby>レビューの<ruby>通読<rt>つうどく</rt></ruby>は、<ruby>最小例<rt>さいしょうれい</rt></ruby>の<ruby>実行<rt>じっこう</rt></ruby>の<ruby>前提<rt>ぜんてい</rt></ruby>ではありません。

<a name="n-726174696f6e616c65"></a>

<a name="設計理由と実装記録"></a>

## <ruby>設計理由<rt>せっけいりゆう</rt></ruby>と<ruby>実装記録<rt>じっそうきろく</rt></ruby>

[<ruby>統合設計<rt>とうごうせっけい</rt></ruby>](<decisions\/multilanguage\-hca\.md>) と [NEPL3h<ruby>案<rt>あん</rt></ruby>](<decisions\/nepl3h\-ghc\-frontend\.md>) は<ruby>設計<rt>せっけい</rt></ruby>の<ruby>意図<rt>いと</rt></ruby>を<ruby>説明<rt>せつめい</rt></ruby>します。<ruby>実装契約<rt>じっそうけいやく</rt></ruby>は<ruby>仕様<rt>しよう</rt></ruby>・schema、<ruby>実装完了<rt>じっそうかんりょう</rt></ruby>は<ruby>状態正本<rt>じょうたいせいほん</rt></ruby>で<ruby>確認<rt>かくにん</rt></ruby>します。[Pages<ruby>情報設計<rt>じょうほうせっけい</rt></ruby>](<decisions\/pages\-information\-architecture\.md>) も<ruby>採用状態<rt>さいようじょうたい</rt></ruby>と<ruby>実装状態<rt>じっそうじょうたい</rt></ruby>を<ruby>分<rt>わ</rt></ruby>けています。

[Foundation](<progress\/foundation\-runtime\.md>)、[Doc](<progress\/doc\-runtime\.md>)、[Math](<progress\/math\-runtime\.md>) の<ruby>記録<rt>きろく</rt></ruby>は<ruby>能力<rt>のうりょく</rt></ruby>と<ruby>検証範囲<rt>けんしょうはんい</rt></ruby>を<ruby>説明<rt>せつめい</rt></ruby>します。<ruby>日時付<rt>にちじつ</rt></ruby>きの<ruby>結果<rt>けっか</rt></ruby>を<ruby>現在<rt>げんざい</rt></ruby>の<ruby>全体状態<rt>ぜんたいじょうたい</rt></ruby>と<ruby>同一視<rt>どういつし</rt></ruby>しません。[<ruby>初期<rt>しょき</rt></ruby>レビュー](<review\.md>)、[r2<ruby>訂正<rt>ていせい</rt></ruby>](<decisions\/0002\-design\-contract\-corrections\.md>)、[Web\/Doc<ruby>移行<rt>いこう</rt></ruby>の<ruby>判断<rt>はんだん</rt></ruby>](<decisions\/0003\-web\-tea\-doc\-migration\.md>)、[Pages<ruby>復旧<rt>ふっきゅう</rt></ruby>の<ruby>判断<rt>はんだん</rt></ruby>](<decisions\/0004\-pages\-recovery\-doc\-inventory\.md>)、[foundation<ruby>契約<rt>けいやく</rt></ruby>の<ruby>判断<rt>はんだん</rt></ruby>](<decisions\/0005\-foundation\-runtime\-contracts\.md>)、[inventory<ruby>監査<rt>かんさ</rt></ruby>](<doc\-inventory\.md>) は<ruby>経緯<rt>けいい</rt></ruby>や<ruby>根拠<rt>こんきょ</rt></ruby>を<ruby>調<rt>しら</rt></ruby>べる<ruby>入口<rt>いりぐち</rt></ruby>です。

<a name="n-617574686f72697479"></a>

<a name="正本と派生資料"></a>

## <ruby>正本<rt>せいほん</rt></ruby>と<ruby>派生資料<rt>はせいしりょう</rt></ruby>

| <ruby>場所<rt>ばしょ</rt></ruby> | <ruby>責務<rt>せきむ</rt></ruby> |
| --- | --- |
| `doc/spec/` | <ruby>構文<rt>こうぶん</rt></ruby>・<ruby>意味<rt>いみ</rt></ruby>・<ruby>操作<rt>そうさ</rt></ruby>・<ruby>失敗条件<rt>しっぱいじょうけん</rt></ruby>・<ruby>受入条件<rt>うけいれじょうけん</rt></ruby>の<ruby>文章仕様<rt>ぶんしょうしよう</rt></ruby> |
| [design\/forms\.json](<\.\.\/design\/forms\.json>) | formの<ruby>構文<rt>こうぶん</rt></ruby>signature |
| [design\/dependencies\.json](<\.\.\/design\/dependencies\.json>) | crate<ruby>責務<rt>せきむ</rt></ruby>と<ruby>依存<rt>いぞん</rt></ruby>の<ruby>許可集合<rt>きょかしゅうごう</rt></ruby> |
| [design\/tasks\.json](<\.\.\/design\/tasks\.json>) | タスクID、<ruby>依存<rt>いぞん</rt></ruby>、<ruby>成果物<rt>せいかぶつ</rt></ruby>、<ruby>受入条件<rt>うけいれじょうけん</rt></ruby>の<ruby>正本<rt>せいほん</rt></ruby> |
| [interfaces\/model\.json](<\.\.\/interfaces\/model\.json>)、[contracts\.json](<\.\.\/interfaces\/contracts\.json>) | <ruby>言語中立<rt>げんごちゅうりつ</rt></ruby>の<ruby>意味<rt>いみ</rt></ruby>モデル・<ruby>操作<rt>そうさ</rt></ruby>schema |
| [interfaces\/foundation\.json](<\.\.\/interfaces\/foundation\.json>) | contractsから<ruby>生成<rt>せいせい</rt></ruby>しproduction registryで<ruby>検査<rt>けんさ</rt></ruby>する<ruby>共通<rt>きょうつう</rt></ruby>package descriptor |
| [interfaces\/doc\.json](<\.\.\/interfaces\/doc\.json>)、[sentence\.json](<\.\.\/interfaces\/sentence\.json>)、[sentence\-reader\.json](<\.\.\/interfaces\/sentence\-reader\.json>) | Doc consumerのarena<ruby>値<rt>あたい</rt></ruby>と、<ruby>独立<rt>どくりつ</rt></ruby>Sentenceの<ruby>値<rt>あたい</rt></ruby>・readerの<ruby>実<rt>じつ</rt></ruby>schema |
| [interfaces\/math\.json](<\.\.\/interfaces\/math\.json>) | Mathの<ruby>表記<rt>ひょうき</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>するarenaとsource<ruby>閉包<rt>へいほう</rt></ruby>の<ruby>実<rt>じつ</rt></ruby>schema。<ruby>構造<rt>こうぞう</rt></ruby>proofと<ruby>式<rt>しき</rt></ruby>の<ruby>評価<rt>ひょうか</rt></ruby>を<ruby>分離<rt>ぶんり</rt></ruby>する |
| `languages/*/syntax.neplg` | <ruby>各<rt>かく</rt></ruby>LanguagePackageの<ruby>文法<rt>ぶんぽう</rt></ruby>source |
| [conformance\/cases\.json](<\.\.\/conformance\/cases\.json>)、[examples\/](<https\:\/\/github\.com\/neknaj\/NEPL3\/tree\/main\/examples>) | <ruby>受入条件<rt>うけいれじょうけん</rt></ruby>と<ruby>検証入力<rt>けんしょうにゅうりょく</rt></ruby> |
| [implementation\-status\.json](<\.\.\/implementation\-status\.json>) | <ruby>実装<rt>じっそう</rt></ruby>・<ruby>試験<rt>しけん</rt></ruby>の<ruby>実行状態<rt>じっこうじょうたい</rt></ruby>。<ruby>仕様定義<rt>しようていぎ</rt></ruby>と<ruby>分離<rt>ぶんり</rt></ruby>する |
| [design\/acceptance\.json](<\.\.\/design\/acceptance\.json>) | <ruby>必須受入群<rt>ひっすうけいれぐん</rt></ruby>と<ruby>必須<rt>ひっす</rt></ruby>targetのcatalog |
| [design\/ui\.json](<\.\.\/design\/ui\.json>)、[site\.json](<\.\.\/design\/site\.json>) | UI\/siteの<ruby>責務<rt>せきむ</rt></ruby>・<ruby>配置計画<rt>はいちけいかく</rt></ruby>。<ruby>実行可能<rt>じっこうかのう</rt></ruby>schemaではない |
| [tasks\/](<https\:\/\/github\.com\/neknaj\/NEPL3\/tree\/main\/tasks>) | タスク<ruby>正本<rt>せいほん</rt></ruby>から<ruby>生成<rt>せいせい</rt></ruby>する<ruby>読<rt>よ</rt></ruby>み<ruby>物<rt>もの</rt></ruby> |
| [history\/](<history\/README\.md>) | <ruby>取<rt>と</rt></ruby>り<ruby>込<rt>こ</rt></ruby>み<ruby>元<rt>もと</rt></ruby>の<ruby>由来<rt>ゆらい</rt></ruby>と<ruby>当時<rt>とうじ</rt></ruby>の<ruby>検査報告<rt>けんさほうこく</rt></ruby> |

<ruby>仕様間<rt>しようかん</rt></ruby>の<ruby>矛盾<rt>むじゅん</rt></ruby>は<ruby>実装<rt>じっそう</rt></ruby>の<ruby>都合<rt>つごう</rt></ruby>で<ruby>読<rt>よ</rt></ruby>み<ruby>替<rt>か</rt></ruby>えず、<ruby>影響<rt>えいきょう</rt></ruby>するschema・<ruby>文法<rt>ぶんぽう</rt></ruby>・<ruby>例<rt>れい</rt></ruby>・<ruby>受入条件<rt>うけいれじょうけん</rt></ruby>を<ruby>合<rt>あ</rt></ruby>わせて<ruby>修正<rt>しゅうせい</rt></ruby>します。[<ruby>設計判断<rt>せっけいはんだん</rt></ruby>](<decisions\/0001\-repository\-foundation\.md>) に<ruby>理由<rt>りゆう</rt></ruby>と<ruby>検証範囲<rt>けんしょうはんい</rt></ruby>を<ruby>残<rt>のこ</rt></ruby>します。<ruby>文法<rt>ぶんぽう</rt></ruby>の<ruby>詳細表<rt>しょうさいひょう</rt></ruby>は [Grammar](<spec\/grammar\-signatures\.md>)、[Doc](<spec\/doc\-signatures\.md>)、[Math](<spec\/math\-signatures\.md>)、[Circuit](<spec\/circuit\-signatures\.md>) を<ruby>参照<rt>さんしょう</rt></ruby>してください。

<ruby>設計一式<rt>せっけいいっしき</rt></ruby>は<ruby>実装<rt>じっそう</rt></ruby>の<ruby>完成証拠<rt>かんせいしょうこ</rt></ruby>ではありません。<ruby>取<rt>と</rt></ruby>り<ruby>込<rt>こ</rt></ruby>み<ruby>元<rt>もと</rt></ruby>の<ruby>検査報告<rt>けんさほうこく</rt></ruby>をruntimeの<ruby>受入試験結果<rt>うけいれしけんけっか</rt></ruby>へ<ruby>転記<rt>てんき</rt></ruby>しません。<ruby>外部資料<rt>がいぶしりょう</rt></ruby>は [<ruby>参考文献<rt>さんこうぶんけん</rt></ruby>](<spec\/references\.md>) にまとめています。

HTMLの<ruby>先行利用<rt>せんこうりよう</rt></ruby>は [T22〜T25の<ruby>実装段階<rt>じっそうだんかい</rt></ruby>](<spec\/18\-html\-delivery\.md>) に<ruby>従<rt>したが</rt></ruby>います。<ruby>既存<rt>きそん</rt></ruby>のDoc<ruby>処理<rt>しょり</rt></ruby>を<ruby>再利用<rt>さいりよう</rt></ruby>し、Doc<ruby>単独<rt>たんどく</rt></ruby>HTML、<ruby>数式<rt>すうしき</rt></ruby>HTML、Web preview\/exportへ<ruby>接続<rt>せつぞく</rt></ruby>します。<ruby>全体<rt>ぜんたい</rt></ruby>の<ruby>最終範囲<rt>さいしゅうはんい</rt></ruby>は<ruby>維持<rt>いじ</rt></ruby>します。

<ruby>型付<rt>かたつ</rt></ruby>きHTML fragmentの<ruby>入力<rt>にゅうりょく</rt></ruby>・<ruby>検査<rt>けんさ</rt></ruby>・serializerは [19<ruby>章<rt>しょう</rt></ruby>](<spec\/19\-html\-fragment\.md>) と [markup schema](<\.\.\/interfaces\/markup\.json>) を<ruby>参照<rt>さんしょう</rt></ruby>してください。

- [Doc HTML<ruby>変換<rt>へんかん</rt></ruby>の<ruby>契約<rt>けいやく</rt></ruby>](<spec\/20\-doc\-html\.md>) — local<ruby>準備<rt>じゅんび</rt></ruby>・<ruby>表示<rt>ひょうじ</rt></ruby>・portable<ruby>再検査<rt>さいけんさ</rt></ruby>と<ruby>未解決資源<rt>みかいけつしげん</rt></ruby>の<ruby>扱<rt>あつか</rt></ruby>い。
- [Docページ<ruby>集合<rt>しゅうごう</rt></ruby>とリンク<ruby>解決<rt>かいけつ</rt></ruby>](<spec\/21\-doc\-pages\.md>) — <ruby>明示<rt>めいじ</rt></ruby>された<ruby>文書<rt>ぶんしょ</rt></ruby>と<ruby>配置<rt>はいち</rt></ruby>を<ruby>索引化<rt>さくいんか</rt></ruby>し、ページ・<ruby>見出<rt>みだ</rt></ruby>しへの<ruby>参照<rt>さんしょう</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。
