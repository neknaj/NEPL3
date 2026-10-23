<!-- Generated from doc/migration/generated/README.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page migration&#45;generated; source SHA-256 3df76b0d4d35b0a32cd00f0a7a1d583cbeb160f73b7939dd2439706ccf72a014; alias input SHA-256 a0364998a9e47ae9f61279ed9ffbc164a365ab977ae428050157e87a3da49b47; page input SHA-256 ba1d21eb5ab7c879b60a3398fcfcf50ee56d062047047c3d45bad74886fa94bf. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="文書移行の生成監査"></a>

# <ruby>文書移行<rt>ぶんしょいこう</rt></ruby>の<ruby>生成監査<rt>せいせいかんさ</rt></ruby>

[正本（NEPL3d）](<README.nepld>)

`doc-inventory.json` は<ruby>過去<rt>かこ</rt></ruby>commitから<ruby>生成<rt>せいせい</rt></ruby>した<ruby>棚卸<rt>たなおろ</rt></ruby>しであり、<ruby>設計正本<rt>せっけいせいほん</rt></ruby>でも<ruby>現在<rt>げんざい</rt></ruby>の<ruby>実装状態<rt>じっそうじょうたい</rt></ruby>でもない。<ruby>手編集<rt>てへんしゅう</rt></ruby>しない。ページ・<ruby>意味要素<rt>いみようそ</rt></ruby>の<ruby>監査差分<rt>かんささぶん</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>できるよう<ruby>保存<rt>ほぞん</rt></ruby>しているが、<ruby>現在<rt>げんざい</rt></ruby>の<ruby>網羅性<rt>もうらせい</rt></ruby>や<ruby>受入合格<rt>うけいれごうかく</rt></ruby>には<ruby>転用<rt>てんよう</rt></ruby>しない。

<ruby>再生成<rt>さいせいせい</rt></ruby>と<ruby>照合<rt>しょうごう</rt></ruby>は [<ruby>文書<rt>ぶんしょ</rt></ruby>inventory](<\.\.\/\.\.\/doc\-inventory\.md>) の<ruby>明示的<rt>めいじてき</rt></ruby>なコマンドで<ruby>行<rt>おこな</rt></ruby>う。<ruby>通常<rt>つうじょう</rt></ruby>のrepository checkは<ruby>履歴監査<rt>りれきかんさ</rt></ruby>を<ruby>実行<rt>じっこう</rt></ruby>しない。<ruby>移行完了後<rt>いこうかんりょうご</rt></ruby>、<ruby>現行判断<rt>げんこうはんだん</rt></ruby>からの<ruby>参照<rt>さんしょう</rt></ruby>がなくなればこのbaselineも<ruby>歴史資料<rt>れきししりょう</rt></ruby>へ<ruby>移<rt>うつ</rt></ruby>し、<ruby>通常<rt>つうじょう</rt></ruby>treeから<ruby>除去<rt>じょきょ</rt></ruby>する。
