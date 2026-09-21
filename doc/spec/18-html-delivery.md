<!-- Generated from doc/spec/18&#45;html&#45;delivery.nepld; renderer nepl3-tools.markdown-annotated/4; source SHA-256 ccb7ab9cf4ea857f1c37e9fb4d2c168ad0699f7ad033f3bd977ff5f5744e62db; alias input SHA-256 8cbc6c3d55e45caec8c46dd1fca71807673ada9181eafbee1d94c020a2b31556. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="18-htmlを先行利用する実装段階"></a>

# 18\. HTMLを<ruby>先行利用<rt>せんこうりよう</rt></ruby>する<ruby>実装段階<rt>じっそうだんかい</rt></ruby>

[正本（NEPL3d）](<18-html-delivery.nepld>)

この<ruby>章<rt>しょう</rt></ruby>はHTML<ruby>生成<rt>せいせい</rt></ruby>の<ruby>各層<rt>かくそう</rt></ruby>の<ruby>接続<rt>せつぞく</rt></ruby>と、<ruby>先行利用<rt>せんこうりよう</rt></ruby>の<ruby>範囲<rt>はんい</rt></ruby>を<ruby>定<rt>さだ</rt></ruby>める。<ruby>構文<rt>こうぶん</rt></ruby>を<ruby>印字<rt>いんじ</rt></ruby>するsource printerをHTML rendererへ<ruby>読<rt>よ</rt></ruby>み<ruby>替<rt>か</rt></ruby>えない。<ruby>採用<rt>さいよう</rt></ruby>の<ruby>経緯<rt>けいい</rt></ruby>は<ruby>設計判断<rt>せっけいはんだん</rt></ruby>0006、<ruby>現在<rt>げんざい</rt></ruby>の<ruby>実装状態<rt>じっそうじょうたい</rt></ruby>はimplementation\-status\.jsonを<ruby>参照<rt>さんしょう</rt></ruby>する。

<a name="n-696d706c656d656e746174696f6e5f646570656e64656e63696573"></a>

<a name="実装依存"></a>

## <ruby>実装依存<rt>じっそういぞん</rt></ruby>

H0〜H3はT22〜T25に<ruby>対応<rt>たいおう</rt></ruby>する<ruby>作業段階<rt>さぎょうだんかい</rt></ruby>であり、<ruby>順序<rt>じゅんじょ</rt></ruby>・<ruby>成果物<rt>せいかぶつ</rt></ruby>・<ruby>受入条件<rt>うけいれじょうけん</rt></ruby>の<ruby>正本<rt>せいほん</rt></ruby>はdesign\/tasks\.jsonとする。<ruby>各段階<rt>かくだんかい</rt></ruby>では<ruby>利用<rt>りよう</rt></ruby>する<ruby>前段<rt>ぜんだん</rt></ruby>の<ruby>実<rt>じつ</rt></ruby>APIを<ruby>検証<rt>けんしょう</rt></ruby>し、<ruby>空<rt>から</rt></ruby>のfragmentで<ruby>未実装<rt>みじっそう</rt></ruby>を<ruby>隠<rt>かく</rt></ruby>さない。T22はT01〜T09の<ruby>全体完了<rt>ぜんたいかんりょう</rt></ruby>を<ruby>着手条件<rt>ちゃくしゅじょうけん</rt></ruby>にせず、T23では<ruby>未対応<rt>みたいおう</rt></ruby>embedを<ruby>型付<rt>かたつ</rt></ruby>きで<ruby>拒否<rt>きょひ</rt></ruby>し、T24でMathの<ruby>実解決<rt>じつかいけつ</rt></ruby>を<ruby>追加<rt>ついか</rt></ruby>する。

<ruby>各層<rt>かくそう</rt></ruby>の<ruby>詳細契約<rt>しょうさいけいやく</rt></ruby>は、Docの<ruby>意味<rt>いみ</rt></ruby>・<ruby>構造<rt>こうぞう</rt></ruby>が05<ruby>章<rt>しょう</rt></ruby>、<ruby>数式表示<rt>すうしきひょうじ</rt></ruby>が17<ruby>章<rt>しょう</rt></ruby>、<ruby>型付<rt>かたつ</rt></ruby>きmarkupとserializerが19<ruby>章<rt>しょう</rt></ruby>、DocのprepareとHTML<ruby>変換<rt>へんかん</rt></ruby>が20<ruby>章<rt>しょう</rt></ruby>、<ruby>集合<rt>しゅうごう</rt></ruby>のpage・anchor<ruby>解決<rt>かいけつ</rt></ruby>が21<ruby>章<rt>しょう</rt></ruby>である。15<ruby>章<rt>しょう</rt></ruby>は<ruby>生成済<rt>せいせいず</rt></ruby>み<ruby>文書<rt>ぶんしょ</rt></ruby>の<ruby>配置<rt>はいち</rt></ruby>と<ruby>公開<rt>こうかい</rt></ruby>、16<ruby>章<rt>しょう</rt></ruby>は<ruby>正式文書<rt>せいしきぶんしょ</rt></ruby>の<ruby>正本切替<rt>せいほんきりかえ</rt></ruby>を<ruby>所有<rt>しょゆう</rt></ruby>する。<ruby>以下<rt>いか</rt></ruby>はそれらの<ruby>接続<rt>せつぞく</rt></ruby>に<ruby>必要<rt>ひつよう</rt></ruby>な<ruby>保証<rt>ほしょう</rt></ruby>の<ruby>要約<rt>ようやく</rt></ruby>であり、<ruby>各層<rt>かくそう</rt></ruby>の<ruby>独立<rt>どくりつ</rt></ruby>した<ruby>検査<rt>けんさ</rt></ruby>を<ruby>置<rt>お</rt></ruby>き<ruby>換<rt>か</rt></ruby>えない。

T10・T11・T13・T15・T17・T18は<ruby>先行経路<rt>せんこうけいろ</rt></ruby>を<ruby>再利用<rt>さいりよう</rt></ruby>して、<ruby>当初<rt>とうしょ</rt></ruby>の<ruby>全責務<rt>ぜんせきむ</rt></ruby>を<ruby>実装<rt>じっそう</rt></ruby>する。Math evaluate、Circuit、LSP、<ruby>汎用<rt>はんよう</rt></ruby>provider、reference profileのUI、Pages<ruby>全受入<rt>ぜんうけいれ</rt></ruby>、T21<ruby>文書移行<rt>ぶんしょいこう</rt></ruby>の<ruby>完成<rt>かんせい</rt></ruby>を<ruby>先行利用<rt>せんこうりよう</rt></ruby>の<ruby>成功<rt>せいこう</rt></ruby>から<ruby>推定<rt>すいてい</rt></ruby>しない。T16はT25と<ruby>既存<rt>きそん</rt></ruby>の<ruby>最終依存<rt>さいしゅういぞん</rt></ruby>、すべてのrequired<ruby>受入条件<rt>うけいれじょうけん</rt></ruby>を<ruby>要求<rt>ようきゅう</rt></ruby>する。

<a name="n-646f63756d656e745f7072657061726174696f6e"></a>

<a name="h0h1の契約"></a>

## H0\/H1の<ruby>契約<rt>けいやく</rt></ruby>

Docの<ruby>既存<rt>きそん</rt></ruby>arenaは<ruby>表<rt>ひょう</rt></ruby>・list・page link・relative link・external link・imageなどを<ruby>保持<rt>ほじ</rt></ruby>するが、<ruby>構造<rt>こうぞう</rt></ruby>proofやCheckedLabelsは<ruby>完全<rt>かんぜん</rt></ruby>なPreparedArticleではない。<ruby>文書検査<rt>ぶんしょけんさ</rt></ruby>は、<ruby>表示対象<rt>ひょうじたいしょう</rt></ruby>language・parallel policy、<ruby>外部<rt>がいぶ</rt></ruby>page・asset・foreign requirementsを<ruby>明示<rt>めいじ</rt></ruby>する。prepareは<ruby>対象<rt>たいしょう</rt></ruby>documentのidentity、<ruby>全要求<rt>ぜんようきゅう</rt></ruby>、<ruby>解決結果<rt>かいけつけっか</rt></ruby>の<ruby>型<rt>かた</rt></ruby>・<ruby>位置<rt>いち</rt></ruby>・<ruby>資源閉包<rt>しげんへいほう</rt></ruby>\{resource closure\}を<ruby>照合<rt>しょうごう</rt></ruby>する。backendへ<ruby>未解決<rt>みかいけつ</rt></ruby>slotやhostの<ruby>任意<rt>にんい</rt></ruby>HTMLを<ruby>渡<rt>わた</rt></ruby>さない。

markupは、table・header・cell、ordered・unordered listとstart・checkbox、img・alt・caption、<ruby>一般<rt>いっぱん</rt></ruby>linkに<ruby>必要<rt>ひつよう</rt></ruby>な<ruby>要素<rt>ようそ</rt></ruby>・<ruby>属性<rt>ぞくせい</rt></ruby>・<ruby>内容<rt>ないよう</rt></ruby>モデルを、<ruby>閉<rt>と</rt></ruby>じた<ruby>型<rt>かた</rt></ruby>として<ruby>持<rt>も</rt></ruby>つ。<ruby>同<rt>どう</rt></ruby>artifactのanchor、page ID、<ruby>相対<rt>そうたい</rt></ruby>path、external URI、asset<ruby>参照<rt>さんしょう</rt></ruby>を<ruby>区別<rt>くべつ</rt></ruby>する。<ruby>危険<rt>きけん</rt></ruby>scheme、path traversal、<ruby>重複<rt>ちょうふく</rt></ruby>path、<ruby>未宣言<rt>みせんげん</rt></ruby>asset、<ruby>偽<rt>にせ</rt></ruby>digest、wrong slotを<ruby>拒否<rt>きょひ</rt></ruby>し、I\/O<ruby>権限<rt>けんげん</rt></ruby>はhostに<ruby>残<rt>のこ</rt></ruby>す。

Paragraphを<ruby>入<rt>い</rt></ruby>れ<ruby>子<rt>こ</rt></ruby>のpへ<ruby>機械変換<rt>きかいへんかん</rt></ruby>せず、<ruby>文<rt>ぶん</rt></ruby>のrunと<ruby>子<rt>こ</rt></ruby>blockのwrapperを<ruby>使<rt>つか</rt></ruby>う。Sentence<ruby>間<rt>かん</rt></ruby>に<ruby>空白<rt>くうはく</rt></ruby>を<ruby>追加<rt>ついか</rt></ruby>しない。Ruby・Anno、Parallel、heading、<ruby>空<rt>から</rt></ruby>の<ruby>表<rt>ひょう</rt></ruby>、list start、codeの<ruby>改行<rt>かいぎょう</rt></ruby>、<ruby>画像<rt>がぞう</rt></ruby>の<ruby>代替説明<rt>だいたいせつめい</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>し、ブラウザparserのtree<ruby>補正<rt>ほせい</rt></ruby>で<ruby>意味<rt>いみ</rt></ruby>が<ruby>変<rt>か</rt></ruby>わっていないか<ruby>確認<rt>かくにん</rt></ruby>する。<ruby>一般<rt>いっぱん</rt></ruby>DocとKaTeXの<ruby>双方<rt>そうほう</rt></ruby>を、<ruby>検査済<rt>けんさず</rt></ruby>みmarkupとserializerへ<ruby>通<rt>とお</rt></ruby>す。

<a name="n-6d6174685f616e645f62726f77736572"></a>

<a name="h2h3の契約と検証"></a>

## H2\/H3の<ruby>契約<rt>けいやく</rt></ruby>と<ruby>検証<rt>けんしょう</rt></ruby>

17<ruby>章<rt>しょう</rt></ruby>を<ruby>適用<rt>てきよう</rt></ruby>する。<ruby>数式<rt>すうしき</rt></ruby>の<ruby>正本<rt>せいほん</rt></ruby>は<ruby>構造<rt>こうぞう</rt></ruby>であり、<ruby>本文<rt>ほんぶん</rt></ruby>のdollar<ruby>探索<rt>たんさく</rt></ruby>、MathMLからのTeX<ruby>推測<rt>すいそく</rt></ruby>、source printer<ruby>文字列<rt>もじれつ</rt></ruby>を<ruby>数式<rt>すうしき</rt></ruby>として<ruby>利用<rt>りよう</rt></ruby>しない。<ruby>非結合<rt>ひけつごう</rt></ruby>operand、<ruby>負号<rt>ふごう</rt></ruby>と<ruby>累乗<rt>るいじょう</rt></ruby>、<ruby>多文字<rt>たもじ</rt></ruby>Unicode<ruby>記号<rt>きごう</rt></ruby>、<ruby>片側<rt>かたがわ</rt></ruby>fence、Doc<ruby>注記<rt>ちゅうき</rt></ruby>の<ruby>忠実性<rt>ちゅうじつせい</rt></ruby>を<ruby>共通試験<rt>きょうつうしけん</rt></ruby>へ<ruby>結<rt>むす</rt></ruby>び<ruby>付<rt>つ</rt></ruby>ける。<ruby>表示<rt>ひょうじ</rt></ruby>のためにevaluateを<ruby>実行<rt>じっこう</rt></ruby>しない。

<ruby>実<rt>じつ</rt></ruby>\.nepldからHTMLまでのpositive・negative、MathML\-only、KaTeX<ruby>不在<rt>ふざい</rt></ruby>、optional module<ruby>失敗<rt>しっぱい</rt></ruby>、<ruby>局所失敗<rt>きょくしょしっぱい</rt></ruby>と<ruby>全体<rt>ぜんたい</rt></ruby>Stopped、<ruby>安全性違反<rt>あんぜんせいいはん</rt></ruby>、asset<ruby>不足<rt>ふそく</rt></ruby>を<ruby>検証<rt>けんしょう</rt></ruby>する。NodeとDOMなしの<ruby>実<rt>じつ</rt></ruby>Workerで、<ruby>固定版<rt>こていばん</rt></ruby>・<ruby>入力<rt>にゅうりょく</rt></ruby>・<ruby>設定<rt>せってい</rt></ruby>が<ruby>同<rt>おな</rt></ruby>じ<ruby>正準<rt>せいじゅん</rt></ruby>markup・manifestを<ruby>生成<rt>せいせい</rt></ruby>することを<ruby>比較<rt>ひかく</rt></ruby>する。

Chromium・Firefox・WebKitで、script<ruby>禁止<rt>きんし</rt></ruby>かつsame\-origin<ruby>権限<rt>けんげん</rt></ruby>なしのpreview、CSP・CORS・font・<ruby>非<rt>ひ</rt></ruby>root path、<ruby>古<rt>ふる</rt></ruby>い<ruby>返信<rt>へんしん</rt></ruby>の<ruby>拒否<rt>きょひ</rt></ruby>、<ruby>停止<rt>ていし</rt></ruby>とepoch、<ruby>同<rt>どう</rt></ruby>artifactのexportを<ruby>実行<rt>じっこう</rt></ruby>する。<ruby>書<rt>か</rt></ruby>き<ruby>出<rt>だ</rt></ruby>し<ruby>済<rt>ず</rt></ruby>みHTMLのJavaScript<ruby>無効表示<rt>むこうひょうじ</rt></ruby>、offline bundle、zoom・<ruby>狭<rt>せま</rt></ruby>い<ruby>幅<rt>はば</rt></ruby>・<ruby>印刷<rt>いんさつ</rt></ruby>・アクセシビリティtreeを<ruby>確認<rt>かくにん</rt></ruby>し、DOMの<ruby>存在<rt>そんざい</rt></ruby>だけを<ruby>視覚組版<rt>しかくくみはん</rt></ruby>の<ruby>成功<rt>せいこう</rt></ruby>にしない。WebKit<ruby>自動試験<rt>じどうしけん</rt></ruby>とSafari・iOSの<ruby>実機確認<rt>じっきかくにん</rt></ruby>を<ruby>区別<rt>くべつ</rt></ruby>する。

<ruby>各段階<rt>かくだんかい</rt></ruby>は、<ruby>実<rt>じつ</rt></ruby>API・command・source・spec・target・ログ・<ruby>独立<rt>どくりつ</rt></ruby>レビューと<ruby>未検証範囲<rt>みけんしょうはんい</rt></ruby>を<ruby>記録<rt>きろく</rt></ruby>する。<ruby>既存<rt>きそん</rt></ruby>bootstrap、Doc source printer、source・Origin、<ruby>予算<rt>よさん</rt></ruby>、no\_std・WASIの<ruby>回帰<rt>かいき</rt></ruby>を<ruby>維持<rt>いじ</rt></ruby>する。ローカルでの<ruby>成功<rt>せいこう</rt></ruby>は、Pages<ruby>公開<rt>こうかい</rt></ruby>の<ruby>証拠<rt>しょうこ</rt></ruby>ではない。

<a name="n-646f63756d656e746174696f6e5f6d6967726174696f6e"></a>

<a name="正式文書のdoc移行"></a>

## <ruby>正式文書<rt>せいしきぶんしょ</rt></ruby>のDoc<ruby>移行<rt>いこう</rt></ruby>

T23で<ruby>必要<rt>ひつよう</rt></ruby>な<ruby>表現<rt>ひょうげん</rt></ruby>が<ruby>使<rt>つか</rt></ruby>えるページから、T21の<ruby>棚卸<rt>たなおろ</rt></ruby>し・<ruby>変換<rt>へんかん</rt></ruby>を<ruby>進<rt>すす</rt></ruby>める。<ruby>数式<rt>すうしき</rt></ruby>を<ruby>含<rt>ふく</rt></ruby>むspecは、T24の<ruby>実生成経路<rt>じつせいせいけいろ</rt></ruby>を<ruby>使用<rt>しよう</rt></ruby>する。T21はT19・T24<ruby>全体<rt>ぜんたい</rt></ruby>の<ruby>完了<rt>かんりょう</rt></ruby>を<ruby>一括<rt>いっかつ</rt></ruby>の<ruby>前提<rt>ぜんてい</rt></ruby>にせず、ページが<ruby>使<rt>つか</rt></ruby>うDoc・Mathの<ruby>操作<rt>そうさ</rt></ruby>ごとに<ruby>必要<rt>ひつよう</rt></ruby>な<ruby>実装<rt>じっそう</rt></ruby>と<ruby>検査<rt>けんさ</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する。NEPL3dを<ruby>正式<rt>せいしき</rt></ruby>な<ruby>正本<rt>せいほん</rt></ruby>とし、HTMLは<ruby>再生成<rt>さいせいせい</rt></ruby>できる<ruby>表示用<rt>ひょうじよう</rt></ruby>の<ruby>出力<rt>しゅつりょく</rt></ruby>とする。Pages<ruby>公開<rt>こうかい</rt></ruby>とrustdocの<ruby>詳細監査<rt>しょうさいかんさ</rt></ruby>を、Docの<ruby>本体開発<rt>ほんたいかいはつ</rt></ruby>・<ruby>正本移行<rt>せいほんいこう</rt></ruby>を<ruby>止<rt>と</rt></ruby>める<ruby>条件<rt>じょうけん</rt></ruby>にしない。ページ<ruby>単位<rt>たんい</rt></ruby>の<ruby>意味<rt>いみ</rt></ruby>・リンク・<ruby>安定<rt>あんてい</rt></ruby>ID・<ruby>生成物対応<rt>せいせいぶつたいおう</rt></ruby>とbootstrapの<ruby>条件<rt>じょうけん</rt></ruby>は、16<ruby>章<rt>しょう</rt></ruby>のまま<ruby>維持<rt>いじ</rt></ruby>する。README・AGENTSなどのMarkdown<ruby>入口<rt>いりぐち</rt></ruby>は、<ruby>必要<rt>ひつよう</rt></ruby>に<ruby>応<rt>おう</rt></ruby>じて\.nepld<ruby>正本<rt>せいほん</rt></ruby>から<ruby>生成<rt>せいせい</rt></ruby>し、<ruby>二重<rt>にじゅう</rt></ruby><ruby>手書<rt>てが</rt></ruby>き<ruby>保守<rt>ほしゅ</rt></ruby>をしない。
