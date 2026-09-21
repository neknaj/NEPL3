<!-- Generated from doc/spec/22&#45;external&#45;extensions.nepld; renderer nepl3-tools.markdown-annotated/3; source SHA-256 0a4b8bf7f98cfc2c75ef1e96cd4bf8e9a6dd84cb070aed7ff5f3ff70c55a64fa; alias input SHA-256 132807f48bf3acceac6beacff6fd690cac164b5d0701ac40b0f0a03c7abfc923; document digest e4f95c527031332a55b17ff175627960bb35ea6d67625b7bf8a282a519f90cb1. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="22-外部言語の追加とrepository分離条件"></a>

# 22\. <ruby>外部言語<rt>がいぶげんご</rt></ruby>の<ruby>追加<rt>ついか</rt></ruby>とrepository<ruby>分離条件<rt>ぶんりじょうけん</rt></ruby>

[正本（NEPL3d）](<22-external-extensions.nepld>)

<a name="n-626f756e64617279"></a>

<a name="1-目的と境界"></a>

## 1\. <ruby>目的<rt>もくてき</rt></ruby>と<ruby>境界<rt>きょうかい</rt></ruby>

NEPL3のfoundationを、<ruby>個別言語<rt>こべつげんご</rt></ruby>の<ruby>追加<rt>ついか</rt></ruby>で<ruby>変更<rt>へんこう</rt></ruby>しないことをアーキテクチャの<ruby>不変条件<rt>ふへんじょうけん</rt></ruby>とする。これは<ruby>既存<rt>きそん</rt></ruby>の<ruby>公開契約<rt>こうかいけいやく</rt></ruby>を<ruby>使<rt>つか</rt></ruby>う<ruby>拡張<rt>かくちょう</rt></ruby>についての<ruby>条件<rt>じょうけん</rt></ruby>であり、<ruby>共通機能<rt>きょうつうきのう</rt></ruby>の<ruby>欠陥修正<rt>けっかんしゅうせい</rt></ruby>や、<ruby>理由<rt>りゆう</rt></ruby>を<ruby>記録<rt>きろく</rt></ruby>した<ruby>互換性変更<rt>ごかんせいへんこう</rt></ruby>を<ruby>永久<rt>えいきゅう</rt></ruby>に<ruby>禁止<rt>きんし</rt></ruby>するものではない。<ruby>現時点<rt>げんじてん</rt></ruby>ではmonorepoを<ruby>維持<rt>いじ</rt></ruby>する。<ruby>実際<rt>じっさい</rt></ruby>のrepository<ruby>作成<rt>さくせい</rt></ruby>、crate<ruby>公開<rt>こうかい</rt></ruby>、<ruby>権限変更<rt>けんげんへんこう</rt></ruby>はこの<ruby>仕様<rt>しよう</rt></ruby>だけで<ruby>実施<rt>じっし</rt></ruby>しない。

<ruby>責務<rt>せきむ</rt></ruby>は<ruby>次<rt>つぎ</rt></ruby>の<ruby>五層<rt>ごそう</rt></ruby>に<ruby>分<rt>わ</rt></ruby>ける。

| <ruby>層<rt>そう</rt></ruby> | <ruby>所有<rt>しょゆう</rt></ruby>するもの |
| --- | --- |
| Foundation | source、Origin、<ruby>診断<rt>しんだん</rt></ruby>、schema、<ruby>値<rt>あたい</rt></ruby>、<ruby>予算<rt>よさん</rt></ruby>、<ruby>言語中立<rt>げんごちゅうりつ</rt></ruby>の<ruby>交換境界<rt>こうかんきょうかい</rt></ruby> |
| Language infrastructure | reader、prefix engine、<ruby>共通<rt>きょうつう</rt></ruby>エディタ<ruby>機構<rt>きこう</rt></ruby> |
| Domain \/ Language package | Grammar、Doc、Math、Circuitおよび<ruby>外部言語<rt>がいぶげんご</rt></ruby>の<ruby>意味<rt>いみ</rt></ruby>・<ruby>構文定義<rt>こうぶんていぎ</rt></ruby> |
| Backend \/ Adapter | HTML・MathML・SVG<ruby>等<rt>とう</rt></ruby>の<ruby>出力<rt>しゅつりょく</rt></ruby>、<ruby>任意<rt>にんい</rt></ruby>のdomain<ruby>間接続<rt>かんせつぞく</rt></ruby> |
| Composition | suite、CLI、provider host、LSP、Web<ruby>製品<rt>せいひん</rt></ruby>の<ruby>構成<rt>こうせい</rt></ruby> |

Foundationとreader\/engineのproduction・build<ruby>依存<rt>いぞん</rt></ruby>からdomain、<ruby>出力<rt>しゅつりょく</rt></ruby>backend、apps、toolsへの<ruby>逆依存<rt>ぎゃくいぞん</rt></ruby>を<ruby>禁止<rt>きんし</rt></ruby>する。Doc・Math・Circuit core<ruby>間<rt>かん</rt></ruby>の<ruby>依存禁止<rt>いぞんきんし</rt></ruby>と、`no_std + alloc`、hostへのI\/O<ruby>集約<rt>しゅうやく</rt></ruby>を<ruby>維持<rt>いじ</rt></ruby>する。markupはDoc<ruby>固有<rt>こゆう</rt></ruby>の<ruby>意味<rt>いみ</rt></ruby>を<ruby>持<rt>も</rt></ruby>たず、<ruby>複数<rt>ふくすう</rt></ruby>backendが<ruby>共有<rt>きょうゆう</rt></ruby>する<ruby>検査済<rt>けんさず</rt></ruby>み<ruby>出力契約<rt>しゅつりょくけいやく</rt></ruby>を<ruby>所有<rt>しょゆう</rt></ruby>する。

<ruby>新<rt>あたら</rt></ruby>しい<ruby>構文<rt>こうぶん</rt></ruby>を<ruby>既存<rt>きそん</rt></ruby>の<ruby>意味<rt>いみ</rt></ruby>モデルへlowerすることと、<ruby>新<rt>あたら</rt></ruby>しい<ruby>意味領域<rt>いみりょういき</rt></ruby>を<ruby>作<rt>つく</rt></ruby>ることを<ruby>分<rt>わ</rt></ruby>ける。<ruby>別<rt>べつ</rt></ruby>frontendやbackendの<ruby>接続可能性<rt>せつぞくかのうせい</rt></ruby>は、Markdown frontendやPDF backendの<ruby>実装済<rt>じっそうず</rt></ruby>み<ruby>宣言<rt>せんげん</rt></ruby>ではない。

<a name="n-756e697473"></a>

<a name="2-拡張単位"></a>

## 2\. <ruby>拡張単位<rt>かくちょうたんい</rt></ruby>

<ruby>共通基盤<rt>きょうつうきばん</rt></ruby>へ<ruby>言語名<rt>げんごめい</rt></ruby>を<ruby>列挙<rt>れっきょ</rt></ruby>するenumや、<ruby>全言語<rt>ぜんげんご</rt></ruby>へcompile・evaluate・render<ruby>等<rt>とう</rt></ruby>を<ruby>強制<rt>きょうせい</rt></ruby>する<ruby>巨大<rt>きょだい</rt></ruby>traitを<ruby>設<rt>もう</rt></ruby>けない。<ruby>各<rt>かく</rt></ruby>packageがschemaと<ruby>必要<rt>ひつよう</rt></ruby>な<ruby>操作<rt>そうさ</rt></ruby>を<ruby>所有<rt>しょゆう</rt></ruby>し、hostが<ruby>明示登録<rt>めいじとうろく</rt></ruby>したpackage・provider・<ruby>資源<rt>しげん</rt></ruby>からProfileを<ruby>解決<rt>かいけつ</rt></ruby>する。<ruby>公式四言語<rt>こうしきよんげんご</rt></ruby>も<ruby>同<rt>おな</rt></ruby>じ<ruby>公開契約<rt>こうかいけいやく</rt></ruby>を<ruby>使<rt>つか</rt></ruby>うreference extensionとし、<ruby>言語名<rt>げんごめい</rt></ruby>による<ruby>特例<rt>とくれい</rt></ruby>をfoundationへ<ruby>入<rt>い</rt></ruby>れない。<ruby>操作<rt>そうさ</rt></ruby>の<ruby>意味<rt>いみ</rt></ruby>、<ruby>能力不足<rt>のうりょくぶそく</rt></ruby>、<ruby>署名違反<rt>しょめいいはん</rt></ruby>、Invalid、Stopped、Awaitを<ruby>区別<rt>くべつ</rt></ruby>する。<ruby>任意<rt>にんい</rt></ruby>の<ruby>未知<rt>みち</rt></ruby>operationを<ruby>成功<rt>せいこう</rt></ruby>として<ruby>扱<rt>あつか</rt></ruby>わず、<ruby>要求<rt>ようきゅう</rt></ruby>と<ruby>登録済<rt>とうろくず</rt></ruby>みschema\/signatureを<ruby>照合<rt>しょうごう</rt></ruby>する。

Rust<ruby>内<rt>ない</rt></ruby>の<ruby>型付<rt>かたつ</rt></ruby>き<ruby>直接呼出<rt>ちょくせつよびだ</rt></ruby>しと、NDFを<ruby>使<rt>つか</rt></ruby>うportable provider<ruby>境界<rt>きょうかい</rt></ruby>を<ruby>維持<rt>いじ</rt></ruby>する。Rustのpointer、allocator、メモリ<ruby>上<rt>じょう</rt></ruby>のenum<ruby>配置<rt>はいち</rt></ruby>、trait vtableを<ruby>外部<rt>がいぶ</rt></ruby>ABIにしない。optionalなDoc\/Math\/Circuit<ruby>連携<rt>れんけい</rt></ruby>はadapterまたはsuiteが<ruby>接続<rt>せつぞく</rt></ruby>し、<ruby>構文<rt>こうぶん</rt></ruby>を<ruby>埋<rt>う</rt></ruby>め<ruby>込<rt>こ</rt></ruby>んだだけではguestを<ruby>評価<rt>ひょうか</rt></ruby>しない。

<ruby>現在<rt>げんざい</rt></ruby>の<ruby>実<rt>じつ</rt></ruby>コードにはLanguagePackage、SchemaRegistry、ParseProfile\/RuntimeCatalog、reader providerとtyped adapterがある。これだけから、LanguagePackage\/Profileの<ruby>完全<rt>かんぜん</rt></ruby>な<ruby>交換<rt>こうかん</rt></ruby>・<ruby>動的<rt>どうてき</rt></ruby>ロードや、Invoke\/Resume\/Reply\/Cancel\/Closeを<ruby>通<rt>とお</rt></ruby>す<ruby>汎用<rt>はんよう</rt></ruby>process providerの<ruby>完成<rt>かんせい</rt></ruby>を<ruby>推定<rt>すいてい</rt></ruby>しない。<ruby>未実装経路<rt>みじっそうけいろ</rt></ruby>はT26の<ruby>未達範囲<rt>みたつはんい</rt></ruby>として<ruby>実装<rt>じっそう</rt></ruby>する。

<a name="n-636f6d7061746962696c697479"></a>

<a name="3-二つの互換性境界"></a>

## 3\. <ruby>二<rt>ふた</rt></ruby>つの<ruby>互換性境界<rt>ごかんせいきょうかい</rt></ruby>

Cargoのpackage versionはRust source APIの<ruby>互換性<rt>ごかんせい</rt></ruby>を<ruby>表<rt>あらわ</rt></ruby>す。schemaのpackage\/revision\/digestはportable<ruby>契約<rt>けいやく</rt></ruby>の<ruby>識別<rt>しきべつ</rt></ruby>に<ruby>使<rt>つか</rt></ruby>う。Rust APIだけの<ruby>非互換変更<rt>ひごかんへんこう</rt></ruby>とwire\/schemaの<ruby>非互換変更<rt>ひごかんへんこう</rt></ruby>を<ruby>別々<rt>べつべつ</rt></ruby>に<ruby>検出<rt>けんしゅつ</rt></ruby>する。すべての<ruby>変更<rt>へんこう</rt></ruby>で<ruby>両方<rt>りょうほう</rt></ruby>の<ruby>版<rt>はん</rt></ruby>を<ruby>上<rt>あ</rt></ruby>げる<ruby>規則<rt>きそく</rt></ruby>にはしない。<ruby>両方<rt>りょうほう</rt></ruby>へ<ruby>影響<rt>えいきょう</rt></ruby>する<ruby>変更<rt>へんこう</rt></ruby>では<ruby>両方<rt>りょうほう</rt></ruby>の<ruby>移行条件<rt>いこうじょうけん</rt></ruby>を<ruby>記録<rt>きろく</rt></ruby>する。digest<ruby>不一致<rt>ふいっち</rt></ruby>は<ruby>同一<rt>どういつ</rt></ruby>でないことの<ruby>検出<rt>けんしゅつ</rt></ruby>であり、<ruby>変更<rt>へんこう</rt></ruby>の<ruby>互換性<rt>ごかんせい</rt></ruby>や<ruby>意味<rt>いみ</rt></ruby>の<ruby>正<rt>ただ</rt></ruby>しさの<ruby>証明<rt>しょうめい</rt></ruby>ではない。schemaを<ruby>変<rt>か</rt></ruby>えない<ruby>挙動<rt>きょどう</rt></ruby>の<ruby>回帰<rt>かいき</rt></ruby>もconformanceで<ruby>検査<rt>けんさ</rt></ruby>する。

<ruby>抽出前<rt>ちゅうしゅつまえ</rt></ruby>に、<ruby>固定旧版<rt>こていきゅうはん</rt></ruby>consumerのcompile<ruby>試験<rt>しけん</rt></ruby>、<ruby>変更<rt>へんこう</rt></ruby>したschema\/signatureの<ruby>拒否試験<rt>きょひしけん</rt></ruby>、<ruby>互換更新<rt>ごかんこうしん</rt></ruby>の<ruby>正例<rt>せいれい</rt></ruby>を<ruby>用意<rt>ようい</rt></ruby>する。path<ruby>依存<rt>いぞん</rt></ruby>や<ruby>固定<rt>こてい</rt></ruby>Git commitでの<ruby>成功<rt>せいこう</rt></ruby>を、<ruby>独立<rt>どくりつ</rt></ruby>SemVerリリースの<ruby>互換性検証<rt>ごかんせいけんしょう</rt></ruby>に<ruby>読<rt>よ</rt></ruby>み<ruby>替<rt>か</rt></ruby>えない。

<a name="n-737461676573"></a>

<a name="4-段階と分離の判定"></a>

## 4\. <ruby>段階<rt>だんかい</rt></ruby>と<ruby>分離<rt>ぶんり</rt></ruby>の<ruby>判定<rt>はんてい</rt></ruby>

T26はT01〜T05とT12の<ruby>成果物<rt>せいかぶつ</rt></ruby>を<ruby>使<rt>つか</rt></ruby>う<ruby>横断的<rt>おうだんてき</rt></ruby>な<ruby>実装<rt>じっそう</rt></ruby>・<ruby>受入<rt>うけいれ</rt></ruby>タスクとする。T01<ruby>等<rt>とう</rt></ruby>の<ruby>個別<rt>こべつ</rt></ruby>タスク<ruby>完了<rt>かんりょう</rt></ruby>をT26<ruby>完了<rt>かんりょう</rt></ruby>に<ruby>依存<rt>いぞん</rt></ruby>させず、foundationの<ruby>抽出可能性<rt>ちゅうしゅつかのうせい</rt></ruby>と<ruby>最終<rt>さいしゅう</rt></ruby>T16の<ruby>条件<rt>じょうけん</rt></ruby>にT26を<ruby>置<rt>お</rt></ruby>く。Doc<ruby>移行<rt>いこう</rt></ruby>・HTML・Pagesの<ruby>先行作業<rt>せんこうさぎょう</rt></ruby>をT26へ<ruby>依存<rt>いぞん</rt></ruby>させない。

<ruby>最初<rt>さいしょ</rt></ruby>の<ruby>実装段階<rt>じっそうだんかい</rt></ruby>では `conformance/extensions/hello/` の<ruby>独立<rt>どくりつ</rt></ruby>Cargo workspaceをリポジトリ<ruby>外<rt>がい</rt></ruby>へコピーし、<ruby>公開<rt>こうかい</rt></ruby>foundation APIだけで<ruby>動<rt>うご</rt></ruby>かす。`hello <name>` のschema・reader<ruby>設定<rt>せってい</rt></ruby>・formはconsumerが<ruby>定義<rt>ていぎ</rt></ruby>する。<ruby>解析結果<rt>かいせきけっか</rt></ruby>の<ruby>独立<rt>どくりつ</rt></ruby>した<ruby>期待値<rt>きたいち</rt></ruby>、UTF\-8 byte<ruby>位置<rt>いち</rt></ruby>、source revision、Origin、<ruby>診断<rt>しんだん</rt></ruby>、<ruby>未知<rt>みち</rt></ruby>head、NeedMore、<ruby>停止<rt>ていし</rt></ruby>、<ruby>不正<rt>ふせい</rt></ruby>NDF<ruby>拒否<rt>きょひ</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。typed ParseTreeをproduction wireで<ruby>交換<rt>こうかん</rt></ruby>し、canonicalな<ruby>意味値<rt>いみち</rt></ruby>の<ruby>一致<rt>いっち</rt></ruby>を<ruby>比較<rt>ひかく</rt></ruby>する。これは<ruby>別<rt>べつ</rt></ruby>process providerの<ruby>比較<rt>ひかく</rt></ruby>ではない。

`python tools/extensions/run.py` は<ruby>固定<rt>こてい</rt></ruby>toolchainとconsumerのCargo\.lockを<ruby>使<rt>つか</rt></ruby>い、<ruby>外部<rt>がいぶ</rt></ruby>workspaceのmember、foundation<ruby>依存<rt>いぞん</rt></ruby>の<ruby>実<rt>じつ</rt></ruby>path、<ruby>実行前後<rt>じっこうぜんご</rt></ruby>のfoundation<ruby>内容<rt>ないよう</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。format・Clippy・production API<ruby>試験<rt>しけん</rt></ruby>を<ruby>実行<rt>じっこう</rt></ruby>し、source hash、consumer hash、<ruby>版<rt>はん</rt></ruby>、command、<ruby>結果<rt>けっか</rt></ruby>、<ruby>生<rt>なま</rt></ruby>logを<ruby>保存<rt>ほぞん</rt></ruby>する。<ruby>現段階<rt>げんだんかい</rt></ruby>では<ruby>依存先<rt>いぞんさき</rt></ruby>のfoundationはmonorepoのpathであり、<ruby>親<rt>おや</rt></ruby>workspaceから<ruby>切<rt>き</rt></ruby>り<ruby>離<rt>はな</rt></ruby>した<ruby>配布物<rt>はいふぶつ</rt></ruby>のbuildを<ruby>証明<rt>しょうめい</rt></ruby>しない。

<ruby>実際<rt>じっさい</rt></ruby>のrepository<ruby>分離<rt>ぶんり</rt></ruby>には、<ruby>次<rt>つぎ</rt></ruby>の<ruby>全条件<rt>ぜんじょうけん</rt></ruby>を<ruby>必要<rt>ひつよう</rt></ruby>とする。

1. foundation<ruby>四<rt>よん</rt></ruby>crateだけの<ruby>配布<rt>はいふ</rt></ruby>・build\/test\/conformanceが<ruby>成立<rt>せいりつ</rt></ruby>し、<ruby>親<rt>おや</rt></ruby>monorepoの<ruby>生成器<rt>せいせいき</rt></ruby>・domain・<ruby>私有<rt>しゆう</rt></ruby>fixtureを<ruby>要求<rt>ようきゅう</rt></ruby>しない。
1. <ruby>別<rt>べつ</rt></ruby>workspaceおよび<ruby>独立<rt>どくりつ</rt></ruby>repositoryにある<ruby>新言語<rt>しんげんご</rt></ruby>が<ruby>公開契約<rt>こうかいけいやく</rt></ruby>だけでparse・schema・source\/Origin・<ruby>診断<rt>しんだん</rt></ruby>・providerを<ruby>提供<rt>ていきょう</rt></ruby>する。<ruby>追加時<rt>ついかじ</rt></ruby>にfoundation sourceを<ruby>変更<rt>へんこう</rt></ruby>しない。
1. Rust<ruby>直接呼出<rt>ちょくせつよびだ</rt></ruby>し、NDF loopback、<ruby>実際<rt>じっさい</rt></ruby>の<ruby>別<rt>べつ</rt></ruby>process providerで、<ruby>意味結果<rt>いみけっか</rt></ruby>・<ruby>位置<rt>いち</rt></ruby>・<ruby>診断<rt>しんだん</rt></ruby>・<ruby>失敗<rt>しっぱい</rt></ruby>・<ruby>停止<rt>ていし</rt></ruby>が<ruby>一致<rt>いっち</rt></ruby>する。
1. schema\/package\/Profileの<ruby>交換<rt>こうかん</rt></ruby>と<ruby>解決<rt>かいけつ</rt></ruby>、<ruby>署名<rt>しょめい</rt></ruby>\/<ruby>版<rt>はん</rt></ruby>\/digest<ruby>不一致<rt>ふいっち</rt></ruby>、<ruby>未知<rt>みち</rt></ruby>operation、<ruby>資源<rt>しげん</rt></ruby>・<ruby>取消<rt>とりけ</rt></ruby>し・<ruby>継続<rt>けいぞく</rt></ruby>の<ruby>境界<rt>きょうかい</rt></ruby>を<ruby>検証<rt>けんしょう</rt></ruby>する。
1. Rust APIとportable<ruby>契約<rt>けいやく</rt></ruby>の<ruby>互換性検査<rt>ごかんせいけんさ</rt></ruby>が<ruby>別々<rt>べつべつ</rt></ruby>に<ruby>機能<rt>きのう</rt></ruby>する。
1. <ruby>正式<rt>せいしき</rt></ruby>catalogのX01とX02に<ruby>実行<rt>じっこう</rt></ruby>・<ruby>独立<rt>どくりつ</rt></ruby>レビュー<ruby>証拠<rt>しょうこ</rt></ruby>がある。

<ruby>条件成立後<rt>じょうけんせいりつご</rt></ruby>の<ruby>最初<rt>さいしょ</rt></ruby>の<ruby>抽出対象<rt>ちゅうしゅつたいしょう</rt></ruby>をDocとし、その<ruby>経験<rt>けいけん</rt></ruby>で<ruby>契約不足<rt>けいやくぶそく</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する。Circuitは<ruby>条件<rt>じょうけん</rt></ruby>が<ruby>整<rt>ととの</rt></ruby>った<ruby>時点<rt>じてん</rt></ruby>で<ruby>別<rt>べつ</rt></ruby>repositoryで<ruby>開始<rt>かいし</rt></ruby>する<ruby>計画<rt>けいかく</rt></ruby>とするが、<ruby>条件待<rt>じょうけんま</rt></ruby>ちを<ruby>理由<rt>りゆう</rt></ruby>に<ruby>既存<rt>きそん</rt></ruby>のCircuit<ruby>実装目標<rt>じっそうもくひょう</rt></ruby>を<ruby>放棄<rt>ほうき</rt></ruby>しない。<ruby>四言語<rt>よんげんご</rt></ruby>、CLI\/WASI、provider、エディタ、TEA Playground、Pages、Doc<ruby>文書移行<rt>ぶんしょいこう</rt></ruby>の<ruby>最終範囲<rt>さいしゅうはんい</rt></ruby>は<ruby>維持<rt>いじ</rt></ruby>する。
