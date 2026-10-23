<!-- Generated from doc/decisions/pages&#45;information&#45;architecture.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page pages&#45;information&#45;architecture; source SHA-256 f68fe42e2003f48fb1ee86b81f7fe9647ef47a9497a821c5049d58ca4eac09a6; alias input SHA-256 3657eccbdd21b6ca645ab816e55d4804972b1e16c42a2c08efb428919a626f68; page input SHA-256 602485bc8c83d64eb95cfb6005941611af75f844e288fa205daefe6900a77b7f. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="github-pages-の情報設計と安定-url"></a>

# GitHub Pages の<ruby>情報設計<rt>じょうほうせっけい</rt></ruby>と<ruby>安定<rt>あんてい</rt></ruby> URL

[正本（NEPL3d）](<pages-information-architecture.nepld>)

Decision status\: adopted — PR \#169、<ruby>統合<rt>とうごう</rt></ruby>commit `e52fc8d8d5122b5f0977ee662dc85d2d56927a86`。

Implementation status\: partial — この<ruby>決定<rt>けってい</rt></ruby>の<ruby>採用<rt>さいよう</rt></ruby>は、Playground・Tutorial・repository<ruby>分割<rt>ぶんかつ</rt></ruby>・<ruby>全公開<rt>ぜんこうかい</rt></ruby>routeの<ruby>実装完了<rt>じっそうかんりょう</rt></ruby>を<ruby>意味<rt>いみ</rt></ruby>しない。<ruby>実装<rt>じっそう</rt></ruby>と<ruby>正式受入<rt>せいしきうけいれ</rt></ruby>の<ruby>状態<rt>じょうたい</rt></ruby>は [implementation\-status\.json](<\.\.\/\.\.\/implementation\-status\.json>) とT18〜T21・T26で<ruby>管理<rt>かんり</rt></ruby>する。

この<ruby>文書<rt>ぶんしょ</rt></ruby>は、NEPL3 ecosystem の GitHub Pages で<ruby>公開<rt>こうかい</rt></ruby>する<ruby>主要<rt>しゅよう</rt></ruby> surface の<ruby>安定<rt>あんてい</rt></ruby> URL と、その<ruby>役割<rt>やくわり</rt></ruby>を<ruby>定<rt>さだ</rt></ruby>める。Pages は NEPL3d <ruby>等<rt>とう</rt></ruby>の<ruby>正本<rt>せいほん</rt></ruby>から<ruby>得<rt>え</rt></ruby>られる<ruby>公開<rt>こうかい</rt></ruby> projection と interactive surface の<ruby>配布先<rt>はいふさき</rt></ruby>であり、HTML <ruby>自体<rt>じたい</rt></ruby>を<ruby>正式文書<rt>せいしきぶんしょ</rt></ruby>の<ruby>正本<rt>せいほん</rt></ruby>とはしない。

<a name="n-737461626c655f75726c73"></a>

<a name="1-安定-url"></a>

## 1\. <ruby>安定<rt>あんてい</rt></ruby> URL

<ruby>標準<rt>ひょうじゅん</rt></ruby> origin は `https://neknaj.github.io/NEPL3/` とし、NEPL3 <ruby>本体<rt>ほんたい</rt></ruby> repository の Pages では<ruby>次<rt>つぎ</rt></ruby>のトップレベル path を<ruby>長期<rt>ちょうき</rt></ruby>に<ruby>安定<rt>あんてい</rt></ruby>させる。

| path | <ruby>役割<rt>やくわり</rt></ruby> |
| --- | --- |
| `/NEPL3/` | NEPL3 ecosystem と<ruby>共通基盤<rt>きょうつうきばん</rt></ruby>の<ruby>入口<rt>いりぐち</rt></ruby> |
| `/NEPL3/playground/` | NEPL3 <ruby>共通基盤<rt>きょうつうきばん</rt></ruby>と<ruby>言語<rt>げんご</rt></ruby> composition を<ruby>対話的<rt>たいわてき</rt></ruby>に<ruby>試<rt>ため</rt></ruby>す Playground |
| `/NEPL3/tutorial/` | NEPL3 の<ruby>共通原理<rt>きょうつうげんり</rt></ruby>と<ruby>言語作成<rt>げんごさくせい</rt></ruby>・composition を<ruby>段階的<rt>だんかいてき</rt></ruby>に<ruby>学<rt>まな</rt></ruby>ぶ Tutorial |
| `/NEPL3/docs/` | NEPL3 <ruby>共通基盤<rt>きょうつうきばん</rt></ruby>の<ruby>正式文書<rt>せいしきぶんしょ</rt></ruby>・<ruby>仕様<rt>しよう</rt></ruby>・<ruby>設計<rt>せっけい</rt></ruby>の<ruby>公開<rt>こうかい</rt></ruby> projection |
| `/NEPL3/api/rust/` | <ruby>現<rt>げん</rt></ruby> NEPL3 repository の Rust API reference。<ruby>利用者向<rt>りようしゃむ</rt></ruby>け Tutorial\/Docs とは<ruby>分離<rt>ぶんり</rt></ruby>する |

`playground`、`tutorial`、`docs` は NEPL3d、NEPL3c、NEPL3h <ruby>等<rt>とう</rt></ruby>の<ruby>個別<rt>こべつ</rt></ruby> guest language の<ruby>所有物<rt>しょゆうぶつ</rt></ruby>としない。NEPL3 <ruby>本体<rt>ほんたい</rt></ruby> repository は、<ruby>共通<rt>きょうつう</rt></ruby> infrastructure、language composition、<ruby>共通<rt>きょうつう</rt></ruby> protocol\/schema、ecosystem の<ruby>接続方法<rt>せつぞくほうほう</rt></ruby>を<ruby>主対象<rt>しゅたいしょう</rt></ruby>とする。

<ruby>既存計画<rt>きそんけいかく</rt></ruby>の `/NEPL3/docs/tutorials/` は<ruby>公開<rt>こうかい</rt></ruby> URL として<ruby>固定<rt>こてい</rt></ruby>せず、Tutorial の<ruby>安定入口<rt>あんていいりぐち</rt></ruby>は `/NEPL3/tutorial/` とする。<ruby>実装前<rt>じっそうまえ</rt></ruby>の planned route であるため、<ruby>公開互換性<rt>こうかいごかんせい</rt></ruby>のための<ruby>恒久<rt>こうきゅう</rt></ruby> alias とは<ruby>扱<rt>あつか</rt></ruby>わない。<ruby>公開後<rt>こうかいご</rt></ruby>に route を<ruby>変更<rt>へんこう</rt></ruby>する<ruby>場合<rt>ばあい</rt></ruby>は、<ruby>安定<rt>あんてい</rt></ruby> page ID と redirect\/alias を<ruby>明示<rt>めいじ</rt></ruby>して silent break を<ruby>避<rt>さ</rt></ruby>ける。

<a name="n-6f776e657273686970"></a>

<a name="2-repository-分割後の文書所有"></a>

## 2\. repository <ruby>分割後<rt>ぶんかつご</rt></ruby>の<ruby>文書所有<rt>ぶんしょしょゆう</rt></ruby>

NEPL3d、NEPL3c、NEPL3h、NEPL3g <ruby>等<rt>とう</rt></ruby>が<ruby>独立<rt>どくりつ</rt></ruby> repository へ<ruby>分割<rt>ぶんかつ</rt></ruby>された<ruby>後<rt>あと</rt></ruby>、<ruby>各言語固有<rt>かくげんごこゆう</rt></ruby>の<ruby>詳細<rt>しょうさい</rt></ruby> reference、<ruby>完全<rt>かんぜん</rt></ruby>な syntax\/semantics reference、<ruby>言語固有<rt>げんごこゆう</rt></ruby> tutorial、<ruby>言語固有<rt>げんごこゆう</rt></ruby> API\/CLI、<ruby>言語固有<rt>げんごこゆう</rt></ruby> examples は<ruby>各<rt>かく</rt></ruby> repository が<ruby>所有<rt>しょゆう</rt></ruby>する。

NEPL3 <ruby>本体<rt>ほんたい</rt></ruby> repository の Pages に<ruby>各言語<rt>かくげんご</rt></ruby>の<ruby>詳細<rt>しょうさい</rt></ruby> reference を<ruby>複製<rt>ふくせい</rt></ruby>して<ruby>集約<rt>しゅうやく</rt></ruby>しない。NEPL3 <ruby>本体側<rt>ほんたいがわ</rt></ruby>には、<ruby>各<rt>かく</rt></ruby> guest language の<ruby>存在<rt>そんざい</rt></ruby>、<ruby>役割<rt>やくわり</rt></ruby>、<ruby>共通基盤<rt>きょうつうきばん</rt></ruby>との<ruby>接続点<rt>せつぞくてん</rt></ruby>、<ruby>互換性<rt>ごかんせい</rt></ruby>・package identity、<ruby>対応<rt>たいおう</rt></ruby> repository への<ruby>安定<rt>あんてい</rt></ruby> link など ecosystem\-level の<ruby>情報<rt>じょうほう</rt></ruby>だけを<ruby>置<rt>お</rt></ruby>く。

repository <ruby>分割前<rt>ぶんかつまえ</rt></ruby>に<ruby>同一<rt>どういつ</rt></ruby> repository <ruby>内<rt>ない</rt></ruby>へ<ruby>存在<rt>そんざい</rt></ruby>する<ruby>言語固有文書<rt>げんごこゆうぶんしょ</rt></ruby>についても、<ruby>将来<rt>しょうらい</rt></ruby>の<ruby>所有者<rt>しょゆうしゃ</rt></ruby>を<ruby>意識<rt>いしき</rt></ruby>し、NEPL3 <ruby>本体<rt>ほんたい</rt></ruby>の<ruby>共通文書<rt>きょうつうぶんしょ</rt></ruby>と<ruby>混同<rt>こんどう</rt></ruby>しない。<ruby>分割時<rt>ぶんかつじ</rt></ruby>に URL を<ruby>維持<rt>いじ</rt></ruby>する<ruby>必要<rt>ひつよう</rt></ruby>がある<ruby>公開済<rt>こうかいず</rt></ruby>み entry point は<ruby>明示的<rt>めいじてき</rt></ruby> redirect\/alias で<ruby>移行<rt>いこう</rt></ruby>する。

<a name="n-646f6373"></a>

<a name="3-docs-の位置付け"></a>

## 3\. Docs の<ruby>位置付<rt>いちづ</rt></ruby>け

<ruby>正式<rt>せいしき</rt></ruby>な<ruby>文書<rt>ぶんしょ</rt></ruby>の<ruby>正本<rt>せいほん</rt></ruby>は<ruby>最終的<rt>さいしゅうてき</rt></ruby>に NEPL3d source とする。GitHub Pages <ruby>上<rt>じょう</rt></ruby>の `/docs/` <ruby>以下<rt>いか</rt></ruby>の HTML は NEPL3d renderer が<ruby>生成<rt>せいせい</rt></ruby>する<ruby>公開<rt>こうかい</rt></ruby> projection であり、HTML を<ruby>別<rt>べつ</rt></ruby>の<ruby>正本<rt>せいほん</rt></ruby>として<ruby>手<rt>て</rt></ruby>で<ruby>保守<rt>ほしゅ</rt></ruby>しない。

Markdown から NEPL3d への<ruby>移行期間<rt>いこうきかん</rt></ruby>は、<ruby>既存<rt>きそん</rt></ruby> Markdown <ruby>正本<rt>せいほん</rt></ruby>と<ruby>移行済<rt>いこうず</rt></ruby>み NEPL3d <ruby>正本<rt>せいほん</rt></ruby>を<ruby>同<rt>おな</rt></ruby>じ<ruby>安定<rt>あんてい</rt></ruby> page ID \/ public route に<ruby>投影<rt>とうえい</rt></ruby>できるようにする。<ruby>移行<rt>いこう</rt></ruby>の<ruby>目的<rt>もくてき</rt></ruby>は HTML site <ruby>自体<rt>じたい</rt></ruby>の<ruby>高度化<rt>こうどか</rt></ruby>ではなく、NEPL3d が<ruby>実際<rt>じっさい</rt></ruby>の<ruby>仕様<rt>しよう</rt></ruby>・<ruby>設計文書<rt>せっけいぶんしょ</rt></ruby>を<ruby>損失<rt>そんしつ</rt></ruby>なく<ruby>表現<rt>ひょうげん</rt></ruby>し、NEPL3 ecosystem の<ruby>正式文書言語<rt>せいしきぶんしょげんご</rt></ruby>として<ruby>利用<rt>りよう</rt></ruby>できることを<ruby>成立<rt>せいりつ</rt></ruby>させることである。

HTML\/browser <ruby>側<rt>がわ</rt></ruby>の<ruby>検査<rt>けんさ</rt></ruby>は、<ruby>安全<rt>あんぜん</rt></ruby>な<ruby>出力<rt>しゅつりょく</rt></ruby>、<ruby>主要<rt>しゅよう</rt></ruby> route\/link、asset の<ruby>成立<rt>せいりつ</rt></ruby>、<ruby>主要<rt>しゅよう</rt></ruby> browser での<ruby>閲覧可能性<rt>えつらんかのうせい</rt></ruby>など<ruby>公開<rt>こうかい</rt></ruby>に<ruby>必要<rt>ひつよう</rt></ruby>な<ruby>範囲<rt>はんい</rt></ruby>を<ruby>扱<rt>あつか</rt></ruby>う。rustdoc や<ruby>最終<rt>さいしゅう</rt></ruby> HTML の<ruby>完全閉包検査<rt>かんぜんへいほうけんさ</rt></ruby>を、NEPL3d <ruby>本体<rt>ほんたい</rt></ruby>・<ruby>文書移行<rt>ぶんしょいこう</rt></ruby>の critical path に<ruby>置<rt>お</rt></ruby>かない。

<a name="n-706c617967726f756e64"></a>

<a name="4-playground-の立場"></a>

## 4\. Playground の<ruby>立場<rt>たちば</rt></ruby>

Playground は NEPL3h、NEPL3c、NEPL3d <ruby>等<rt>とう</rt></ruby>の<ruby>特定<rt>とくてい</rt></ruby> guest language の<ruby>専用<rt>せんよう</rt></ruby> IDE としない。NEPL3 の<ruby>共通基盤<rt>きょうつうきばん</rt></ruby>を<ruby>直接観察<rt>ちょくせつかんさつ</rt></ruby>・<ruby>試験<rt>しけん</rt></ruby>できる<ruby>中立的<rt>ちゅうりつてき</rt></ruby>な surface とする。

Playground では<ruby>少<rt>すく</rt></ruby>なくとも<ruby>次<rt>つぎ</rt></ruby>を<ruby>扱<rt>あつか</rt></ruby>えるようにする。

- <ruby>共通<rt>きょうつう</rt></ruby>の<ruby>括弧<rt>かっこ</rt></ruby>なし<ruby>前置記法<rt>ぜんちきほう</rt></ruby>と arity\/context による<ruby>構文確定<rt>こうぶんかくてい</rt></ruby>を<ruby>観察<rt>かんさつ</rt></ruby>する。
- <ruby>小<rt>ちい</rt></ruby>さな LanguagePackage \/ grammar を<ruby>定義<rt>ていぎ</rt></ruby>し、<ruby>最小<rt>さいしょう</rt></ruby>の NEPL3 guest language を<ruby>作<rt>つく</rt></ruby>る。
- <ruby>作成<rt>さくせい</rt></ruby>した<ruby>言語<rt>げんご</rt></ruby>を parse\/check\/print し、<ruby>構文<rt>こうぶん</rt></ruby>・<ruby>診断<rt>しんだん</rt></ruby>・Source\/Origin <ruby>等<rt>とう</rt></ruby>の<ruby>共通機構<rt>きょうつうきこう</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する。
- <ruby>別<rt>べつ</rt></ruby> LanguagePackage を import\/composition し、<ruby>一<rt>ひと</rt></ruby>つの source <ruby>内<rt>ない</rt></ruby>で<ruby>複数言語<rt>ふくすうげんご</rt></ruby>を<ruby>統合<rt>とうごう</rt></ruby>する。
- `annotation` のような<ruby>基礎的<rt>きそてき</rt></ruby>な<ruby>別言語<rt>べつげんご</rt></ruby>を import し、<ruby>自作<rt>じさく</rt></ruby> guest language の syntax に<ruby>対象付<rt>たいしょうつ</rt></ruby>き<ruby>注釈<rt>ちゅうしゃく</rt></ruby>を<ruby>組<rt>く</rt></ruby>み<ruby>込<rt>こ</rt></ruby>む。
- `sentence` のように<ruby>多数<rt>たすう</rt></ruby>の<ruby>言語<rt>げんご</rt></ruby>から<ruby>共有<rt>きょうゆう</rt></ruby>されることを<ruby>意図<rt>いと</rt></ruby>した<ruby>基礎言語<rt>きそげんご</rt></ruby>を<ruby>利用<rt>りよう</rt></ruby>する。

<ruby>個別<rt>こべつ</rt></ruby> guest language は example\/profile として Playground から<ruby>選択<rt>せんたく</rt></ruby>できるが、Playground の navigation・<ruby>概念<rt>がいねん</rt></ruby>モデル・<ruby>操作契約<rt>そうさけいやく</rt></ruby>そのものを<ruby>一<rt>ひと</rt></ruby>つの guest language の semantics に<ruby>合<rt>あ</rt></ruby>わせない。

<a name="n-7475746f7269616c"></a>

<a name="5-tutorial-の学習目標"></a>

## 5\. Tutorial の<ruby>学習目標<rt>がくしゅうもくひょう</rt></ruby>

Tutorial も<ruby>特定<rt>とくてい</rt></ruby> guest language の<ruby>入門書<rt>にゅうもんしょ</rt></ruby>を<ruby>中心<rt>ちゅうしん</rt></ruby>にしない。NEPL3 の<ruby>共通原理<rt>きょうつうげんり</rt></ruby>から<ruby>始<rt>はじ</rt></ruby>め、<ruby>利用者自身<rt>りようしゃじしん</rt></ruby>が<ruby>言語<rt>げんご</rt></ruby>を<ruby>構成<rt>こうせい</rt></ruby>し、<ruby>他言語<rt>たげんご</rt></ruby>を composition するところまでを<ruby>基本課程<rt>きほんかてい</rt></ruby>とする。

<ruby>初期<rt>しょき</rt></ruby> Tutorial は<ruby>概<rt>おおむ</rt></ruby>ね<ruby>次<rt>つぎ</rt></ruby>の<ruby>順序<rt>じゅんじょ</rt></ruby>を<ruby>想定<rt>そうてい</rt></ruby>する。

1. NEPL3 の<ruby>目的<rt>もくてき</rt></ruby>\: <ruby>多数<rt>たすう</rt></ruby>の<ruby>異<rt>こと</rt></ruby>なる<ruby>言語<rt>げんご</rt></ruby>を、<ruby>共通<rt>きょうつう</rt></ruby>の<ruby>括弧<rt>かっこ</rt></ruby>なし<ruby>前置記法<rt>ぜんちきほう</rt></ruby>と<ruby>共通<rt>きょうつう</rt></ruby> infrastructure の<ruby>上<rt>うえ</rt></ruby>で<ruby>組<rt>く</rt></ruby>み<ruby>合<rt>あ</rt></ruby>わせる。
1. token、head、arity、context、prefix parse <ruby>等<rt>とう</rt></ruby>、<ruby>言語非依存<rt>げんごひいそん</rt></ruby>の<ruby>構文確定原理<rt>こうぶんかくていげんり</rt></ruby>を Playground で<ruby>確認<rt>かくにん</rt></ruby>する。
1. <ruby>最小<rt>さいしょう</rt></ruby>の guest language を<ruby>定義<rt>ていぎ</rt></ruby>し、literal と<ruby>少数<rt>しょうすう</rt></ruby>の prefix form を parse\/check\/print できるようにする。
1. LanguagePackage、schema、operation\/provider、Profile <ruby>等<rt>とう</rt></ruby>、<ruby>他言語<rt>たげんご</rt></ruby>と composition するための<ruby>共通契約<rt>きょうつうけいやく</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する。
1. `annotation` を<ruby>別言語<rt>べつげんご</rt></ruby>として import し、<ruby>自作言語<rt>じさくげんご</rt></ruby>に<ruby>対象付<rt>たいしょうつ</rt></ruby>き<ruby>注釈<rt>ちゅうしゃく</rt></ruby>を<ruby>組<rt>く</rt></ruby>み<ruby>込<rt>こ</rt></ruby>む。annotation を<ruby>自作言語<rt>じさくげんご</rt></ruby>の<ruby>特別構文<rt>とくべつこうぶん</rt></ruby>として<ruby>再実装<rt>さいじっそう</rt></ruby>しない。
1. `sentence` <ruby>等<rt>とう</rt></ruby>の<ruby>基礎言語<rt>きそげんご</rt></ruby>を<ruby>必要<rt>ひつよう</rt></ruby>に<ruby>応<rt>おう</rt></ruby>じて composition し、<ruby>複数言語<rt>ふくすうげんご</rt></ruby>が<ruby>一<rt>ひと</rt></ruby>つの source \/ document \/ structure <ruby>内<rt>ない</rt></ruby>で<ruby>共存<rt>きょうぞん</rt></ruby>する<ruby>例<rt>れい</rt></ruby>を<ruby>作<rt>つく</rt></ruby>る。
1. Source\/Origin、diagnostic、binding、portable representation など、composition <ruby>後<rt>あと</rt></ruby>も<ruby>共通<rt>きょうつう</rt></ruby> infrastructure が<ruby>維持<rt>いじ</rt></ruby>する<ruby>情報<rt>じょうほう</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する。
1. <ruby>実在<rt>じつざい</rt></ruby>する<ruby>個別<rt>こべつ</rt></ruby> guest language は<ruby>共通原理<rt>きょうつうげんり</rt></ruby>の<ruby>応用例<rt>おうようれい</rt></ruby>として<ruby>軽<rt>かる</rt></ruby>く<ruby>扱<rt>あつか</rt></ruby>い、<ruby>詳細学習<rt>しょうさいがくしゅう</rt></ruby>は<ruby>各言語<rt>かくげんご</rt></ruby> repository の<ruby>文書<rt>ぶんしょ</rt></ruby>へ<ruby>接続<rt>せつぞく</rt></ruby>する。

この Tutorial の<ruby>到達点<rt>とうたつてん</rt></ruby>は、<ruby>既存<rt>きそん</rt></ruby>の<ruby>一言語<rt>いちげんご</rt></ruby>を<ruby>使<rt>つか</rt></ruby>えるようになることだけではなく、NEPL3 <ruby>上<rt>じょう</rt></ruby>で<ruby>小<rt>ちい</rt></ruby>さな<ruby>言語<rt>げんご</rt></ruby>を<ruby>作<rt>つく</rt></ruby>り、<ruby>独立<rt>どくりつ</rt></ruby>した<ruby>別言語<rt>べつげんご</rt></ruby>を import して<ruby>統合<rt>とうごう</rt></ruby>できることとする。

<a name="n-666f756e646174696f6e5f6c616e677561676573"></a>

<a name="6-基礎言語の扱い"></a>

## 6\. <ruby>基礎言語<rt>きそげんご</rt></ruby>の<ruby>扱<rt>あつか</rt></ruby>い

`annotation`、`sentence` など、<ruby>多数<rt>たすう</rt></ruby>の guest language から<ruby>利用<rt>りよう</rt></ruby>されることを<ruby>意図<rt>いと</rt></ruby>する<ruby>基礎言語<rt>きそげんご</rt></ruby>は、<ruby>個別<rt>こべつ</rt></ruby> domain language より<ruby>広<rt>ひろ</rt></ruby>い<ruby>共通教材<rt>きょうつうきょうざい</rt></ruby>として Playground\/Tutorial から<ruby>利用可能<rt>りようかのう</rt></ruby>にする。

ただし、それらを NEPL3 core に<ruby>暗黙統合<rt>あんもくとうごう</rt></ruby>された<ruby>特別構文<rt>とくべつこうぶん</rt></ruby>として<ruby>扱<rt>あつか</rt></ruby>わない。Tutorial でも「<ruby>別<rt>べつ</rt></ruby> LanguagePackage を composition する」という NEPL3 の<ruby>通常<rt>つうじょう</rt></ruby>の<ruby>仕組<rt>しく</rt></ruby>みを<ruby>見<rt>み</rt></ruby>せる。これにより、NEPL3 の<ruby>共通基盤<rt>きょうつうきばん</rt></ruby>が<ruby>特定<rt>とくてい</rt></ruby> language の syntax\/semantics を<ruby>内蔵<rt>ないぞう</rt></ruby>しなくても language integration を<ruby>成立<rt>せいりつ</rt></ruby>させられることを<ruby>教材自体<rt>きょうざいじたい</rt></ruby>で<ruby>検証<rt>けんしょう</rt></ruby>する。

<a name="n-75726c5f7072696e6369706c6573"></a>

<a name="7-url-安定性の原則"></a>

## 7\. URL <ruby>安定性<rt>あんていせい</rt></ruby>の<ruby>原則<rt>げんそく</rt></ruby>

- public route は<ruby>表示名<rt>ひょうじめい</rt></ruby>や repository <ruby>内<rt>ない</rt></ruby> source path から<ruby>自動生成<rt>じどうせいせい</rt></ruby>しない。
- page \/ tutorial lesson \/ playground preset には<ruby>安定<rt>あんてい</rt></ruby> ID を<ruby>与<rt>あた</rt></ruby>える。
- source file の<ruby>移動<rt>いどう</rt></ruby>・<ruby>正本形式<rt>せいほんけいしき</rt></ruby>の Markdown から NEPL3d への<ruby>変更<rt>へんこう</rt></ruby>で public URL を<ruby>変更<rt>へんこう</rt></ruby>しない。
- Playground の<ruby>内部状態<rt>ないぶじょうたい</rt></ruby>は shareable な deep link を<ruby>持<rt>も</rt></ruby>てるようにするが、トップレベル `/playground/` は<ruby>固定<rt>こてい</rt></ruby>する。
- Tutorial の lesson URL も<ruby>安定<rt>あんてい</rt></ruby> ID を<ruby>基準<rt>きじゅん</rt></ruby>にし、<ruby>見出<rt>みだ</rt></ruby>し<ruby>変更<rt>へんこう</rt></ruby>だけで URL を<ruby>破壊<rt>はかい</rt></ruby>しない。
- repository <ruby>分割<rt>ぶんかつ</rt></ruby>に<ruby>伴<rt>ともな</rt></ruby>い<ruby>個別<rt>こべつ</rt></ruby> guest language の<ruby>詳細文書<rt>しょうさいぶんしょ</rt></ruby>を<ruby>移動<rt>いどう</rt></ruby>する<ruby>場合<rt>ばあい</rt></ruby>は、<ruby>既<rt>すで</rt></ruby>に<ruby>公開済<rt>こうかいず</rt></ruby>みの<ruby>入口<rt>いりぐち</rt></ruby>について<ruby>明示的<rt>めいじてき</rt></ruby>な<ruby>移行先<rt>いこうさき</rt></ruby>を<ruby>定<rt>さだ</rt></ruby>める。

この URL <ruby>設計<rt>せっけい</rt></ruby>は Pages <ruby>実装<rt>じっそう</rt></ruby>を<ruby>先行<rt>せんこう</rt></ruby>させるためのものではない。NEPL3d <ruby>文書移行<rt>ぶんしょいこう</rt></ruby>、Playground、Tutorial、<ruby>各<rt>かく</rt></ruby> repository <ruby>分割<rt>ぶんかつ</rt></ruby>が<ruby>進<rt>すす</rt></ruby>んでも<ruby>公開<rt>こうかい</rt></ruby> path を<ruby>不用意<rt>ふようい</rt></ruby>に<ruby>変更<rt>へんこう</rt></ruby>しないため、<ruby>先<rt>さき</rt></ruby>に namespace を<ruby>予約<rt>よやく</rt></ruby>する<ruby>計画<rt>けいかく</rt></ruby>である。
