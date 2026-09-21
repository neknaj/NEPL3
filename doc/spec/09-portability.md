<!-- Generated from doc/spec/09&#45;portability.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page portability; source SHA-256 229b76ec982a2eb2585809ae138e865e137d31bfb9feb838416a45dd2dc9b465; alias input SHA-256 f6e5ea8b221e730a8d50ed28afc74cb0afd9831704b5f19b3729abf52cd8cd9d; page input SHA-256 9aa3793c9c8353d2d3201f238919679312ab0bc51bed4b4b58d6cc09af88f58d. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="09-rust以外へ置換するための契約"></a>

# 09\. Rust<ruby>以外<rt>いがい</rt></ruby>へ<ruby>置換<rt>ちかん</rt></ruby>するための<ruby>契約<rt>けいやく</rt></ruby>

[正本（NEPL3d）](<09-portability.nepld>)

この<ruby>章<rt>しょう</rt></ruby>は<ruby>公開境界<rt>こうかいきょうかい</rt></ruby>と<ruby>置換<rt>ちかん</rt></ruby>の<ruby>目標契約<rt>もくひょうけいやく</rt></ruby>を<ruby>定<rt>さだ</rt></ruby>める。<ruby>全<rt>ぜん</rt></ruby>provider・process transport・<ruby>全<rt>ぜん</rt></ruby>targetの<ruby>実装<rt>じっそう</rt></ruby>と<ruby>受入完了<rt>うけいれかんりょう</rt></ruby>を<ruby>宣言<rt>せんげん</rt></ruby>するものではない。<ruby>現在<rt>げんざい</rt></ruby>の<ruby>実装<rt>じっそう</rt></ruby>と<ruby>受入状態<rt>うけいれじょうたい</rt></ruby>はimplementation\-status\.jsonで<ruby>管理<rt>かんり</rt></ruby>する。

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## <ruby>方針<rt>ほうしん</rt></ruby>

<ruby>意味<rt>いみ</rt></ruby>モデル・<ruby>操作<rt>そうさ</rt></ruby>・<ruby>診断<rt>しんだん</rt></ruby>・source<ruby>対応<rt>たいおう</rt></ruby>のschemaを<ruby>公開境界<rt>こうかいきょうかい</rt></ruby>とする。Rustのメモリ<ruby>表現<rt>ひょうげん</rt></ruby>やtrait objectをABIにしない。native fast pathとportable pathを<ruby>同時<rt>どうじ</rt></ruby>に<ruby>提供<rt>ていきょう</rt></ruby>し、その<ruby>意味<rt>いみ</rt></ruby>を<ruby>比較<rt>ひかく</rt></ruby>する。

<a name="n-6c6179657273"></a>

<a name="1-三つの層"></a>

## 1\. <ruby>三<rt>みっ</rt></ruby>つの<ruby>層<rt>そう</rt></ruby>

1. <ruby>言語中立<rt>げんごちゅうりつ</rt></ruby>のschema\: field<ruby>順<rt>じゅん</rt></ruby>、variant、<ruby>必須制約<rt>ひっすせいやく</rt></ruby>、<ruby>整数<rt>せいすう</rt></ruby>\/<ruby>有理数<rt>ゆうりすう</rt></ruby>、<ruby>範囲<rt>はんい</rt></ruby>と<ruby>参照<rt>さんしょう</rt></ruby>、operation signature。
1. Rustの<ruby>型付<rt>かたつ</rt></ruby>きAPI\: schemaに<ruby>対応<rt>たいおう</rt></ruby>するstruct\/enumと<ruby>関数<rt>かんすう</rt></ruby>。Rust<ruby>内<rt>ない</rt></ruby>の<ruby>通常利用<rt>つうじょうりよう</rt></ruby>ではserialize<ruby>不要<rt>ふよう</rt></ruby>。
1. transport adapter\: schema<ruby>値<rt>あたい</rt></ruby>をNDF（<ruby>下記<rt>かき</rt></ruby>）でencode\/decodeし、<ruby>別<rt>べつ</rt></ruby>process\/<ruby>別実装<rt>べつじっそう</rt></ruby>へ<ruby>渡<rt>わた</rt></ruby>す。

この<ruby>分離<rt>ぶんり</rt></ruby>により、<ruby>将来<rt>しょうらい</rt></ruby>のNEPL3プログラミング<ruby>言語<rt>げんご</rt></ruby>は<ruby>同<rt>おな</rt></ruby>じschemaを<ruby>生成<rt>せいせい</rt></ruby>\/<ruby>消費<rt>しょうひ</rt></ruby>する<ruby>実装<rt>じっそう</rt></ruby>を<ruby>持<rt>も</rt></ruby>てばよい。<ruby>現在<rt>げんざい</rt></ruby>のRustのallocator、enum discriminant、pointer、Arc\/Rc、usize、trait vtableは<ruby>境界<rt>きょうかい</rt></ruby>を<ruby>越<rt>こ</rt></ruby>えない。

<a name="n-6e6466"></a>

<a name="2-ndf1"></a>

## 2\. NDF\/1

canonical<ruby>値<rt>あたい</rt></ruby>digestは `SHA-256(domain || canonical NDF/1 CBOR(value))` とする。digest<ruby>専用経路<rt>せんようけいろ</rt></ruby>は<ruby>通常<rt>つうじょう</rt></ruby>のencoderと<ruby>同<rt>おな</rt></ruby>じ<ruby>符号化規則<rt>ふごうかきそく</rt></ruby>でbyte<ruby>片<rt>へん</rt></ruby>を<ruby>順次<rt>じゅんじ</rt></ruby>hashへ<ruby>渡<rt>わた</rt></ruby>してよく、<ruby>完全<rt>かんぜん</rt></ruby>なCBOR bufferの<ruby>作成<rt>さくせい</rt></ruby>を<ruby>必須<rt>ひっす</rt></ruby>にしない。この<ruby>場合<rt>ばあい</rt></ruby>も<ruby>値<rt>あたい</rt></ruby>の<ruby>走査<rt>そうさ</rt></ruby>・<ruby>整数<rt>せいすう</rt></ruby>の<ruby>一時表現<rt>いちじひょうげん</rt></ruby>・stackの<ruby>割当<rt>わりあて</rt></ruby>と、<ruby>符号化<rt>ふごうか</rt></ruby>およびhashの<ruby>全<rt>ぜん</rt></ruby>byte<ruby>処理<rt>しょり</rt></ruby>を<ruby>同<rt>おな</rt></ruby>じBudgetへ<ruby>先行課金<rt>せんこうかきん</rt></ruby>し、<ruby>停止時<rt>ていしじ</rt></ruby>はdigestを<ruby>返<rt>かえ</rt></ruby>さない。<ruby>出力<rt>しゅつりょく</rt></ruby>は32byteのdigestとして<ruby>課金<rt>かきん</rt></ruby>する。CBOR bufferを<ruby>実際<rt>じっさい</rt></ruby>に<ruby>作<rt>つく</rt></ruby>る<ruby>経路<rt>けいろ</rt></ruby>ではその<ruby>出力<rt>しゅつりょく</rt></ruby>・<ruby>割当<rt>わりあて</rt></ruby>も<ruby>課金<rt>かきん</rt></ruby>し、<ruby>会計<rt>かいけい</rt></ruby>だけを<ruby>省略<rt>しょうりゃく</rt></ruby>してはならない。<ruby>通常<rt>つうじょう</rt></ruby>のNDF encodeは<ruby>引<rt>ひ</rt></ruby>き<ruby>続<rt>つづ</rt></ruby>き<ruby>生成<rt>せいせい</rt></ruby>する<ruby>全<rt>ぜん</rt></ruby>CBOR byteを<ruby>出力<rt>しゅつりょく</rt></ruby>として<ruby>課金<rt>かきん</rt></ruby>する。<ruby>文書<rt>ぶんしょ</rt></ruby>digestを<ruby>得<rt>え</rt></ruby>るための<ruby>一時<rt>いちじ</rt></ruby>CBORを<ruby>除去<rt>じょきょ</rt></ruby>することと、HTMLの<ruby>出力上限<rt>しゅつりょくじょうげん</rt></ruby>を<ruby>変更<rt>へんこう</rt></ruby>することは<ruby>別<rt>べつ</rt></ruby>である。

NDFは<ruby>本仕様<rt>ほんしよう</rt></ruby>の<ruby>型付<rt>かたつ</rt></ruby>き<ruby>値<rt>あたい</rt></ruby>をCBORで<ruby>運<rt>はこ</rt></ruby>ぶ<ruby>符号化<rt>ふごうか</rt></ruby>profile。RFC 8949のdefinite lengthと<ruby>最短<rt>さいたん</rt></ruby>の<ruby>整数<rt>せいすう</rt></ruby>\/<ruby>長<rt>なが</rt></ruby>さ<ruby>表現<rt>ひょうげん</rt></ruby>を<ruby>要求<rt>ようきゅう</rt></ruby>する。map、float、NaN、CBOR tag、null、indefinite lengthはNDFでは<ruby>使用<rt>しよう</rt></ruby>しない。CBORそのものの<ruby>汎用機能<rt>はんようきのう</rt></ruby>を<ruby>全部許可<rt>ぜんぶきょか</rt></ruby>するわけではない。

<ruby>値<rt>あたい</rt></ruby>は<ruby>次<rt>つぎ</rt></ruby>のtagged arrayで<ruby>表<rt>あらわ</rt></ruby>す。

| tag | <ruby>配列<rt>はいれつ</rt></ruby> | <ruby>型<rt>かた</rt></ruby> |
| ---: | --- | --- |
| 0 | `[0]` | Unit |
| 1 | `[1, bool]` | Bool |
| 2 | `[2, uint64]` | Offset\/Index<ruby>等<rt>など</rt></ruby>の<ruby>有限<rt>ゆうげん</rt></ruby>unsigned |
| 3 | `[3, negative:bool, magnitude:bytes]` | <ruby>任意精度<rt>にんいせいど</rt></ruby>Integer |
| 4 | `[4, numerator:Value(tag3), denominator:bytes]` | Rational |
| 5 | `[5, text]` | Text（valid UTF\-8） |
| 6 | `[6, bytes]` | Bytes |
| 7 | `[7, [Value...]]` | List |
| 8 | `[8]` | None |
| 9 | `[9, Value]` | Some |
| 10 | `[10, SchemaRef, KindName, [Value...]]` | Record、field<ruby>順<rt>じゅん</rt></ruby>はschema |
| 11 | `[11, SchemaRef, TypeName, VariantName, [Value...]]` | Variant |

SchemaRefは `[packageName:text, revision:uint64, digest:bytes32]`。<ruby>型名<rt>かためい</rt></ruby>\/kind<ruby>名<rt>めい</rt></ruby>は<ruby>登録済<rt>とうろくず</rt></ruby>みschemaに<ruby>照合<rt>しょうごう</rt></ruby>する。naturalはtag3のnonnegativeを<ruby>要求<rt>ようきゅう</rt></ruby>するfield<ruby>制約<rt>せいやく</rt></ruby>。integer magnitudeはbig\-endian、<ruby>先頭<rt>せんとう</rt></ruby>zeroなし、zeroはempty bytesかつnegative\=false。rational denominatorは<ruby>正<rt>せい</rt></ruby>で<ruby>先頭<rt>せんとう</rt></ruby>zeroなし、gcd\=1、zero numeratorはdenominator\=1。

NodeId\/EntityId<ruby>等<rt>など</rt></ruby>は<ruby>公開<rt>こうかい</rt></ruby>record<ruby>内<rt>ない</rt></ruby>のindexとしてencodeし、<ruby>同<rt>おな</rt></ruby>じbundleに<ruby>対応<rt>たいおう</rt></ruby>するtableとschemaを<ruby>含<rt>ふく</rt></ruby>める。<ruby>未定義参照<rt>みていぎさんしょう</rt></ruby>、<ruby>循環禁止構造<rt>じゅんかんきんしこうぞう</rt></ruby>の<ruby>循環<rt>じゅんかん</rt></ruby>、<ruby>重複<rt>じゅうふく</rt></ruby>ID、<ruby>範囲外<rt>はんいがい</rt></ruby>spanはdecode<ruby>直後<rt>ちょくご</rt></ruby>に<ruby>拒否<rt>きょひ</rt></ruby>する。<ruby>文字列型<rt>もじれつがた</rt></ruby>の<ruby>違反<rt>いはん</rt></ruby>を<ruby>遅<rt>おく</rt></ruby>れてRustのdowncastで<ruby>検出<rt>けんしゅつ</rt></ruby>する<ruby>方式<rt>ほうしき</rt></ruby>にしない。

`interfaces/contracts.json` と<ruby>各言語<rt>かくげんご</rt></ruby>constructor<ruby>表<rt>ひょう</rt></ruby>からcodecの<ruby>網羅性検査<rt>もうらせいけんさ</rt></ruby>を<ruby>生成<rt>せいせい</rt></ruby>する。serdeのderive<ruby>既定表現<rt>きていひょうげん</rt></ruby>を<ruby>契約<rt>けいやく</rt></ruby>にせず、custom codecでNDFへ<ruby>写<rt>うつ</rt></ruby>す。<ruby>内部<rt>ないぶ</rt></ruby>でserdeを<ruby>使<rt>つか</rt></ruby>う<ruby>場合<rt>ばあい</rt></ruby>も<ruby>明示<rt>めいじ</rt></ruby>したarray\/schemaに<ruby>固定<rt>こてい</rt></ruby>する。

<a name="n-696e7472696e736963"></a>

<a name="21-intrinsic値の閉じた記述"></a>

### 2\.1 intrinsic<ruby>値<rt>あたい</rt></ruby>の<ruby>閉<rt>と</rt></ruby>じた<ruby>記述<rt>きじゅつ</rt></ruby>

`interfaces/contracts.json` の `intrinsic_types` は、NDF\/1をdecodeした<ruby>論理値<rt>ろんりち</rt></ruby>の<ruby>型<rt>かた</rt></ruby>を<ruby>定義<rt>ていぎ</rt></ruby>する。`NdfValue` の12caseは<ruby>上表<rt>じょうひょう</rt></ruby>のtag 0–11に<ruby>一対一<rt>いちたいいち</rt></ruby>で<ruby>対応<rt>たいおう</rt></ruby>する。`NdfScalar` はUnit、Bool、U64、Integer、Rational、Text、Bytesの<ruby>部分型<rt>ぶぶんがた</rt></ruby>、`TypedValue` はRecord、Variantだけの<ruby>部分型<rt>ぶぶんがた</rt></ruby>である。subsetのcase<ruby>列<rt>れつ</rt></ruby>は<ruby>集合<rt>しゅうごう</rt></ruby>であり、<ruby>順序<rt>じゅんじょ</rt></ruby>をwire IDにしない。Unitは<ruby>追加<rt>ついか</rt></ruby>payloadを<ruby>持<rt>も</rt></ruby>たず、Noneとは<ruby>異<rt>こと</rt></ruby>なる。NaturalはIntegerの<ruby>非負制約<rt>ひふせいやく</rt></ruby>、Bytes32はBytesの<ruby>長<rt>なが</rt></ruby>さ<ruby>制約<rt>せいやく</rt></ruby>であり、<ruby>追加<rt>ついか</rt></ruby>のwire tagを<ruby>持<rt>も</rt></ruby>たない。

このintrinsic<ruby>記述<rt>きじゅつ</rt></ruby>は<ruby>通常<rt>つうじょう</rt></ruby>のdomain sumではない。たとえば `NdfValue.Integer(value: Integer)` の<ruby>論理<rt>ろんり</rt></ruby>fieldを<ruby>一般<rt>いっぱん</rt></ruby>のVariantとしてtag11で<ruby>包<rt>つつ</rt></ruby>まず、<ruby>上表<rt>じょうひょう</rt></ruby>のtag3へ<ruby>直接写<rt>ちょくせつうつ</rt></ruby>す。Integerのnegative\/magnitude、Rationalの<ruby>分子<rt>ぶんし</rt></ruby>tag3と<ruby>分母<rt>ぶんぼ</rt></ruby>bytesは<ruby>上表<rt>じょうひょう</rt></ruby>の<ruby>専用表現<rt>せんようひょうげん</rt></ruby>を<ruby>使<rt>つか</rt></ruby>う。tag10\/11のfieldsは<ruby>裸<rt>はだか</rt></ruby>のCBOR arrayであり、Listのtag7を<ruby>付<rt>つ</rt></ruby>けない。そのheader<ruby>内<rt>ない</rt></ruby>のSchemaRefも<ruby>裸<rt>はだか</rt></ruby>の `[text,uint64,bytes32]` であり、<ruby>通常<rt>つうじょう</rt></ruby>のRecordのtag10を<ruby>付<rt>つ</rt></ruby>けない。<ruby>論理<rt>ろんり</rt></ruby>fieldのTextやU64も、<ruby>上表<rt>じょうひょう</rt></ruby>で<ruby>裸<rt>はだか</rt></ruby>のCBOR text\/uint64を<ruby>指定<rt>してい</rt></ruby>する<ruby>位置<rt>いち</rt></ruby>へ<ruby>余分<rt>よぶん</rt></ruby>なNDF tagを<ruby>付<rt>つ</rt></ruby>けない。codecの<ruby>物理表現<rt>ぶつりひょうげん</rt></ruby>は<ruby>上表<rt>じょうひょう</rt></ruby>を<ruby>正本<rt>せいほん</rt></ruby>とする。

intrinsicの<ruby>識別子<rt>しきべつし</rt></ruby> `nepl3.ndf/1` は<ruby>本符号化<rt>ほんふごうか</rt></ruby>profileに<ruby>組<rt>く</rt></ruby>み<ruby>込<rt>こ</rt></ruby>まれた<ruby>固定識別子<rt>こていしきべつし</rt></ruby>である。intrinsic<ruby>自体<rt>じたい</rt></ruby>へdomain SchemaRefや<ruby>自己<rt>じこ</rt></ruby>hashを<ruby>要求<rt>ようきゅう</rt></ruby>しない。Record\/Variant headerのSchemaRefは<ruby>運<rt>はこ</rt></ruby>ばれるdomain<ruby>値<rt>あたい</rt></ruby>のdescriptorを<ruby>識別<rt>しきべつ</rt></ruby>し、intrinsicの<ruby>識別子<rt>しきべつし</rt></ruby>とは<ruby>別物<rt>べつもの</rt></ruby>である。domain descriptorの<ruby>登録<rt>とうろく</rt></ruby>・digest<ruby>検証<rt>けんしょう</rt></ruby>は<ruby>必要<rt>ひつよう</rt></ruby>であり、TypedValueという<ruby>型名<rt>かためい</rt></ruby>やtag10\/11であることだけでは<ruby>検査済<rt>けんさず</rt></ruby>みにならない。<ruby>通常<rt>つうじょう</rt></ruby>のfieldとしてのSchemaRefは `nepl3.foundation` revision 1のSchemaRef recordをtag10で<ruby>運<rt>はこ</rt></ruby>ぶ。そのheaderのSchemaRefだけが<ruby>裸<rt>はだか</rt></ruby>のtupleとなるため、<ruby>無限<rt>むげん</rt></ruby>のwrapper<ruby>再帰<rt>さいき</rt></ruby>は<ruby>生<rt>しょう</rt></ruby>じない。

<a name="n-7265666572656e636573"></a>

<a name="22-descriptorの型参照検査と残る契約"></a>

### 2\.2 descriptorの<ruby>型参照検査<rt>かたさんしょうけんさ</rt></ruby>と<ruby>残<rt>のこ</rt></ruby>る<ruby>契約<rt>けいやく</rt></ruby>

fieldとunion<ruby>参照<rt>さんしょう</rt></ruby>の<ruby>型式<rt>かたしき</rt></ruby>は `Name | List<Type> | Option<Type>` とする。NameはASCII<ruby>英字<rt>えいじ</rt></ruby>で<ruby>始<rt>はじ</rt></ruby>まる<ruby>英数字<rt>えいすうじ</rt></ruby>\/underscoreのsegmentを `:` または `/` で<ruby>接続<rt>せつぞく</rt></ruby>する。<ruby>空<rt>くう</rt></ruby>segment、<ruby>空白<rt>くうはく</rt></ruby>、<ruby>余分<rt>よぶん</rt></ruby>なtoken、<ruby>未宣言<rt>みせんげん</rt></ruby>のgeneric、<ruby>引数<rt>ひきすう</rt></ruby>の<ruby>過不足<rt>かふそく</rt></ruby>は<ruby>拒否<rt>きょひ</rt></ruby>する。List\/Optionは<ruby>予約<rt>よやく</rt></ruby>された1<ruby>引数<rt>ひきすう</rt></ruby>constructorであり、<ruby>名義型<rt>めいぎがた</rt></ruby>として<ruby>宣言<rt>せんげん</rt></ruby>できない。<ruby>開発<rt>かいはつ</rt></ruby>checkerは128<ruby>階層<rt>かいそう</rt></ruby>を<ruby>超<rt>こ</rt></ruby>える<ruby>型式<rt>かたしき</rt></ruby>を<ruby>拒否<rt>きょひ</rt></ruby>する。この<ruby>型名文法<rt>かためいぶんぽう</rt></ruby>と<ruby>上限<rt>じょうげん</rt></ruby>は<ruby>設計<rt>せっけい</rt></ruby>descriptorの<ruby>記述<rt>きじゅつ</rt></ruby>に<ruby>限<rt>かぎ</rt></ruby>り、<ruby>利用者<rt>りようしゃ</rt></ruby>の<ruby>言語中<rt>げんごちゅう</rt></ruby>の<ruby>名前<rt>なまえ</rt></ruby>・kind<ruby>文字列<rt>もじれつ</rt></ruby>・source・NDF Textの<ruby>文字集合<rt>もじしゅうごう</rt></ruby>を<ruby>制限<rt>せいげん</rt></ruby>しない。runtime<ruby>入力<rt>にゅうりょく</rt></ruby>のLimitsも<ruby>代替<rt>だいたい</rt></ruby>しない。

builtinはUnit、Bool、U64、Integer、Natural、Rational、Text、Bytes、Bytes32である。<ruby>所有者<rt>しょゆうしゃ</rt></ruby>は<ruby>本<rt>ほん</rt></ruby>profileであり、modelのscalar\_typesとcontractsのscalar\_aliasesはそれへの<ruby>参照<rt>さんしょう</rt></ruby>・<ruby>説明<rt>せつめい</rt></ruby>である。model\.types、contracts\.records\/enums\/intrinsic\_typesの<ruby>名義型定義<rt>めいぎがたていぎ</rt></ruby>は<ruby>重複<rt>じゅうふく</rt></ruby>を<ruby>許<rt>ゆる</rt></ruby>さない。modelの<ruby>外部参照<rt>がいぶさんしょう</rt></ruby>はexternal\_typesへ<ruby>明示<rt>めいじ</rt></ruby>し、<ruby>存在<rt>そんざい</rt></ruby>する<ruby>外部所有型<rt>がいぶしょゆうがた</rt></ruby>へ<ruby>解決<rt>かいけつ</rt></ruby>する。modelの<ruby>可視名<rt>かしめい</rt></ruby>は<ruby>自身<rt>じしん</rt></ruby>の<ruby>定義<rt>ていぎ</rt></ruby>と<ruby>明示<rt>めいじ</rt></ruby>したscalar\/external importに<ruby>限<rt>かぎ</rt></ruby>る。<ruby>既存<rt>きそん</rt></ruby>の<ruby>再帰的<rt>さいきてき</rt></ruby>な<ruby>型<rt>かた</rt></ruby>graphは<ruby>許<rt>ゆる</rt></ruby>すが、<ruby>値<rt>あたい</rt></ruby>の<ruby>循環可否<rt>じゅんかんかひ</rt></ruby>・source\/Origin tableの<ruby>整合<rt>せいごう</rt></ruby>は<ruby>操作<rt>そうさ</rt></ruby>ごとの<ruby>値検査<rt>あたいけんさ</rt></ruby>で<ruby>別<rt>べつ</rt></ruby>に<ruby>判定<rt>はんてい</rt></ruby>する。

<ruby>型名<rt>かためい</rt></ruby>がすべて<ruby>解決<rt>かいけつ</rt></ruby>することと<ruby>公開操作契約<rt>こうかいそうさけいやく</rt></ruby>が<ruby>完成<rt>かんせい</rt></ruby>することを<ruby>区別<rt>くべつ</rt></ruby>する。27<ruby>操作<rt>そうさ</rt></ruby>のinput\/outputには<ruby>説明用<rt>せつめいよう</rt></ruby>の<ruby>式<rt>しき</rt></ruby>が<ruby>残<rt>のこ</rt></ruby>り、<ruby>型付<rt>かたつ</rt></ruby>きの<ruby>具体化<rt>ぐたいか</rt></ruby>・provider frame・Profile・Doc<ruby>移行<rt>いこう</rt></ruby>のDG01–DG06は<ruby>対応<rt>たいおう</rt></ruby>する<ruby>実装<rt>じっそう</rt></ruby>とともに<ruby>完成<rt>かんせい</rt></ruby>させる。<ruby>進捗<rt>しんちょく</rt></ruby>と<ruby>実行証拠<rt>じっこうしょうこ</rt></ruby>は [<ruby>実装記録<rt>じっそうきろく</rt></ruby>](<\.\.\/progress\/foundation\-runtime\.md>) を<ruby>参照<rt>さんしょう</rt></ruby>する。

<a name="n-64657363726970746f72"></a>

<a name="23-実行可能なschema-descriptor"></a>

### 2\.3 <ruby>実行可能<rt>じっこうかのう</rt></ruby>なschema descriptor

`interfaces/contracts.json` のTypeDescriptorはUnit\/Bool\/U64\/Integer\/Natural\/Rational\/Text\/Bytes\/Bytes32、intrinsicのNdfValue\/NdfScalar\/TypedValue、List、Option、Namedを<ruby>区別<rt>くべつ</rt></ruby>する。Namedはpackage\/revision\/nameの<ruby>記号参照<rt>きごうさんしょう</rt></ruby>でありdigestを<ruby>入<rt>い</rt></ruby>れない。Recordは<ruby>順序付<rt>じゅんじょつ</rt></ruby>きfield、Variantは<ruby>名前付<rt>なまえつ</rt></ruby>きのvariantと<ruby>順序付<rt>じゅんじょつ</rt></ruby>きpayloadを<ruby>持<rt>も</rt></ruby>つ。constraintsはschema<ruby>所有<rt>しょゆう</rt></ruby>の<ruby>意味制約<rt>いみせいやく</rt></ruby>IDの<ruby>集合<rt>しゅうごう</rt></ruby>である。

SchemaDescriptorはpackage、revision、types、operationsを<ruby>持<rt>も</rt></ruby>つ。canonical JSONではtypesとoperationsを<ruby>名前<rt>なまえ</rt></ruby>keyのobject、<ruby>型定義<rt>かたていぎ</rt></ruby>を `{constraints,record}` または `{constraints,variant}` とし、fieldを `[name,TypeJSON]` の<ruby>順序付<rt>じゅんじょつ</rt></ruby>きarrayにする。TypeJSONはbuiltin\/intrinsic<ruby>名<rt>めい</rt></ruby>の<ruby>文字列<rt>もじれつ</rt></ruby>、`{list:TypeJSON}`、`{option:TypeJSON}`、`{named:{name,package,revision}}` のいずれか。operationは `{input,output,pure}`。constraintsは<ruby>重複<rt>じゅうふく</rt></ruby>を<ruby>拒否<rt>きょひ</rt></ruby>して<ruby>名前順<rt>なまえじゅん</rt></ruby>に<ruby>並<rt>なら</rt></ruby>べる。13<ruby>章<rt>しょう</rt></ruby>のkey<ruby>順<rt>じゅん</rt></ruby>・escape・domain separatorでSHA\-256を<ruby>求<rt>もと</rt></ruby>める。

<ruby>登録<rt>とうろく</rt></ruby>は<ruby>期待<rt>きたい</rt></ruby>するSchemaRefとdescriptorの<ruby>計算<rt>けいさん</rt></ruby>digestを<ruby>照合<rt>しょうごう</rt></ruby>する。<ruby>一<rt>ひと</rt></ruby>つのregistryで<ruby>同<rt>おな</rt></ruby>じpackage\/revisionに<ruby>異<rt>こと</rt></ruby>なるdigestを<ruby>同時選択<rt>どうじせんたく</rt></ruby>しない。<ruby>全<rt>ぜん</rt></ruby>packageを<ruby>登録後<rt>とうろくご</rt></ruby>にfinalizeし、<ruby>使用<rt>しよう</rt></ruby>されていないvariantやoperationも<ruby>含<rt>ふく</rt></ruby>むすべてのNamed<ruby>参照<rt>さんしょう</rt></ruby>を<ruby>解決<rt>かいけつ</rt></ruby>する。<ruby>相互参照<rt>そうごさんしょう</rt></ruby>する<ruby>型<rt>かた</rt></ruby>・packageの<ruby>登録<rt>とうろく</rt></ruby>は<ruby>許<rt>ゆる</rt></ruby>すが、finalize<ruby>前<rt>まえ</rt></ruby>の<ruby>値検査<rt>あたいけんさ</rt></ruby>・<ruby>実行<rt>じっこう</rt></ruby>は<ruby>拒否<rt>きょひ</rt></ruby>する。

`interfaces/foundation.json` はcontractsから `cargo run --locked -p nepl3-tools -- foundation --write` で<ruby>生成<rt>せいせい</rt></ruby>する<ruby>実際<rt>じっさい</rt></ruby>の `nepl3.foundation` descriptorである。build\.rsから<ruby>生成<rt>せいせい</rt></ruby>せず、<ruby>通常<rt>つうじょう</rt></ruby>の<ruby>検査<rt>けんさ</rt></ruby>で<ruby>正本<rt>せいほん</rt></ruby>との<ruby>一致<rt>いっち</rt></ruby>とproduction core registryによる<ruby>登録<rt>とうろく</rt></ruby>・<ruby>参照閉包<rt>さんしょうへいほう</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。<ruby>未完成<rt>みかんせい</rt></ruby>のoperation<ruby>説明表<rt>せつめいひょう</rt></ruby>を<ruby>実行可能<rt>じっこうかのう</rt></ruby>なoperationsへコピーしない。このpackageは<ruby>共通<rt>きょうつう</rt></ruby>の<ruby>値<rt>あたい</rt></ruby>・transport recordのschemaを<ruby>提供<rt>ていきょう</rt></ruby>し、<ruby>言語操作<rt>げんごそうさ</rt></ruby>の<ruby>実装<rt>じっそう</rt></ruby>を<ruby>広告<rt>こうこく</rt></ruby>しない。

<ruby>同<rt>おな</rt></ruby>じ<ruby>明示生成<rt>めいじせいせい</rt></ruby>で `crates/foundation/core/src/schema/foundation.rs` を<ruby>作<rt>つく</rt></ruby>り、productionの `foundation::descriptor` が<ruby>型付<rt>かたつ</rt></ruby>きdescriptorを<ruby>構築<rt>こうちく</rt></ruby>する。<ruby>構築前<rt>こうちくまえ</rt></ruby>に<ruby>割当<rt>わりあて</rt></ruby>・work<ruby>予算<rt>よさん</rt></ruby>を<ruby>計上<rt>けいじょう</rt></ruby>し、toolsのJSON parserをproductionへ<ruby>依存<rt>いぞん</rt></ruby>させない。<ruby>通常<rt>つうじょう</rt></ruby>の<ruby>検査<rt>けんさ</rt></ruby>はJSONとRust<ruby>投影<rt>とうえい</rt></ruby>の<ruby>両方<rt>りょうほう</rt></ruby>を<ruby>正本<rt>せいほん</rt></ruby>と<ruby>比較<rt>ひかく</rt></ruby>する。wireのsource\/Span adapterはこのdescriptorを<ruby>登録<rt>とうろく</rt></ruby>したregistryで<ruby>構造<rt>こうぞう</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>し、さらにsource digest・<ruby>宣言順<rt>せんげんじゅん</rt></ruby>・identity・locator・snapshot・UTF\-8<ruby>境界<rt>きょうかい</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>してnative<ruby>型<rt>がた</rt></ruby>へ<ruby>戻<rt>もど</rt></ruby>す。

raw encode\/decodeはNDF intrinsicのcanonical<ruby>性<rt>せい</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。<ruby>公開操作<rt>こうかいそうさ</rt></ruby>の<ruby>境界<rt>きょうかい</rt></ruby>ではexpected TypeDescriptorとfinalize<ruby>済<rt>ず</rt></ruby>みregistryを<ruby>渡<rt>わた</rt></ruby>すchecked encode\/decodeを<ruby>使<rt>つか</rt></ruby>い、<ruby>受信<rt>じゅしん</rt></ruby>したschema\/kind\/variant\/field<ruby>型<rt>がた</rt></ruby>を<ruby>照合<rt>しょうごう</rt></ruby>する。<ruby>得<rt>え</rt></ruby>られるStructuralValueは<ruby>構造検査<rt>こうぞうけんさ</rt></ruby>の<ruby>証明<rt>しょうめい</rt></ruby>であり、constraintsに<ruby>列挙<rt>れっきょ</rt></ruby>したsourceの<ruby>対応<rt>たいおう</rt></ruby>・<ruby>回路<rt>かいろ</rt></ruby>の<ruby>幅等<rt>はばなど</rt></ruby>の<ruby>意味検査<rt>いみけんさ</rt></ruby>を<ruby>代替<rt>だいたい</rt></ruby>しない。coreの<ruby>対応<rt>たいおう</rt></ruby>constructorまたはdomain<ruby>操作<rt>そうさ</rt></ruby>で<ruby>必要<rt>ひつよう</rt></ruby>な<ruby>不変条件<rt>ふへんじょうけん</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>してから<ruby>使用<rt>しよう</rt></ruby>する。

<a name="n-696e766f636174696f6e"></a>

<a name="3-操作呼出し"></a>

## 3\. <ruby>操作呼出<rt>そうさよびだ</rt></ruby>し

CallはrequestId、OperationRef、input TypedRecord、sources\/resources\/environment、Limitsを<ruby>持<rt>も</rt></ruby>つ。OperationRefはpackage\/revision\/digestとoperation<ruby>名<rt>めい</rt></ruby>。provider manifestにinput\/output schema、<ruby>必要<rt>ひつよう</rt></ruby>capability、<ruby>純粋性契約<rt>じゅんすいせいけいやく</rt></ruby>を<ruby>宣言<rt>せんげん</rt></ruby>する。

ReplyはComplete \/ Invalid \/ Stopped \/ Awaitのvariant。DiagnosticとEventは<ruby>成功<rt>せいこう</rt></ruby>の<ruby>値<rt>あたい</rt></ruby>と<ruby>別<rt>べつ</rt></ruby>field。<ruby>未対応<rt>みたいおう</rt></ruby>operationを<ruby>空<rt>から</rt></ruby>の<ruby>値<rt>あたい</rt></ruby>や<ruby>成功<rt>せいこう</rt></ruby>Unitで<ruby>返<rt>かえ</rt></ruby>さない。

Awaitは<ruby>外部<rt>がいぶ</rt></ruby>service<ruby>要求<rt>ようきゅう</rt></ruby>とcontinuationを<ruby>返<rt>かえ</rt></ruby>す。hostはallowlistで<ruby>照合<rt>しょうごう</rt></ruby>し、<ruby>同一親予算<rt>どういつおやよさん</rt></ruby>で<ruby>要求<rt>ようきゅう</rt></ruby>を<ruby>実行<rt>じっこう</rt></ruby>してresumeする。continuationはprovider revision、<ruby>親<rt>おや</rt></ruby>request、snapshot digestに<ruby>束縛<rt>そくばく</rt></ruby>し、<ruby>別<rt>べつ</rt></ruby>providerへ<ruby>渡<rt>わた</rt></ruby>せない。<ruby>中断中<rt>ちゅうだんちゅう</rt></ruby>の<ruby>要求<rt>ようきゅう</rt></ruby>を<ruby>実装差替<rt>じっそうさしか</rt></ruby>え<ruby>対象<rt>たいしょう</rt></ruby>にしない。

call graphはhostが<ruby>追跡<rt>ついせき</rt></ruby>する。<ruby>同<rt>おな</rt></ruby>じoperation\/input\/contextの<ruby>循環依存<rt>じゅんかんいぞん</rt></ruby>はCyclicOperation。<ruby>有限<rt>ゆうげん</rt></ruby>だが<ruby>大<rt>おお</rt></ruby>きい<ruby>再帰<rt>さいき</rt></ruby>も<ruby>共通<rt>きょうつう</rt></ruby>Limitsで<ruby>停止<rt>ていし</rt></ruby>できる。provider<ruby>内部<rt>ないぶ</rt></ruby>のアルゴリズム<ruby>固有<rt>こゆう</rt></ruby>costは<ruby>性能情報<rt>せいのうじょうほう</rt></ruby>であり、<ruby>二実装<rt>にじっそう</rt></ruby>でusageの<ruby>数値一致<rt>すうちいっち</rt></ruby>を<ruby>互換要件<rt>ごかんようけん</rt></ruby>にしない。

<a name="n-7472616e73666f726d"></a>

<a name="31-reader-transformの操作返信"></a>

### 3\.1 Reader Transformの<ruby>操作返信<rt>そうさへんしん</rt></ruby>

Transformのdomain<ruby>結果<rt>けっか</rt></ruby>と<ruby>外側<rt>そとがわ</rt></ruby>OperationReplyは<ruby>次<rt>つぎ</rt></ruby>のように<ruby>対応<rt>たいおう</rt></ruby>する。<ruby>内側<rt>うちがわ</rt></ruby>のtyped TransformReplyはsources\/sourceMapsを<ruby>全結果<rt>ぜんけっか</rt></ruby>で<ruby>所有<rt>しょゆう</rt></ruby>する。

| TransformOutcome | OperationReply | typed payload |
| --- | --- | --- |
| Complete | Complete | TransformReply |
| Failed | Invalid | partial\=Some\(TransformReply\) |
| Stopped\(reason\) | Stopped\(reason\) | partial\=Some\(TransformReply\) |

<ruby>内外<rt>ないがい</rt></ruby>のdiagnostics\/events\/usage\/traceOverflowは<ruby>同<rt>おな</rt></ruby>じReportの<ruby>正確<rt>せいかく</rt></ruby>な<ruby>再掲<rt>さいけい</rt></ruby>であり、<ruby>不一致<rt>ふいっち</rt></ruby>を<ruby>拒否<rt>きょひ</rt></ruby>する。<ruby>再掲<rt>さいけい</rt></ruby>によるstorageと<ruby>符号化<rt>ふごうか</rt></ruby>の<ruby>費用<rt>ひよう</rt></ruby>は<ruby>計上<rt>けいじょう</rt></ruby>するが、<ruby>診断<rt>しんだん</rt></ruby>・eventを<ruby>再発行<rt>さいはっこう</rt></ruby>したことにはしない。Stoppedの<ruby>理由<rt>りゆう</rt></ruby>も<ruby>一致<rt>いっち</rt></ruby>を<ruby>要求<rt>ようきゅう</rt></ruby>する。CompleteにFailedを<ruby>隠<rt>かく</rt></ruby>す、Invalidを<ruby>成功<rt>せいこう</rt></ruby>Unitに<ruby>変<rt>か</rt></ruby>える、Transformに<ruby>未定義<rt>みていぎ</rt></ruby>のAwaitを<ruby>返<rt>かえ</rt></ruby>す、といった<ruby>返信<rt>へんしん</rt></ruby>は<ruby>拒否<rt>きょひ</rt></ruby>する。

partial\=NoneのInvalid\/Stoppedは、domain Transform<ruby>結果<rt>けっか</rt></ruby>が<ruby>得<rt>え</rt></ruby>られる<ruby>前<rt>まえ</rt></ruby>のdispatch<ruby>失敗<rt>しっぱい</rt></ruby>である。adapterは<ruby>型付<rt>かたつ</rt></ruby>きの<ruby>拒否結果<rt>きょひけっか</rt></ruby>と<ruby>正式<rt>せいしき</rt></ruby>Reportをhostへ<ruby>返<rt>かえ</rt></ruby>し、Readerの<ruby>待機<rt>たいき</rt></ruby>slotへ<ruby>成功<rt>せいこう</rt></ruby>やdomain<ruby>失敗<rt>しっぱい</rt></ruby>として<ruby>適用<rt>てきよう</rt></ruby>しない。この<ruby>場合<rt>ばあい</rt></ruby>の<ruby>位置参照<rt>いちさんしょう</rt></ruby>は<ruby>元<rt>もと</rt></ruby>の<ruby>保存要求<rt>ほぞんようきゅう</rt></ruby>と<ruby>正式<rt>せいしき</rt></ruby>checkpointのsource<ruby>閉包<rt>へいほう</rt></ruby>だけに<ruby>限<rt>かぎ</rt></ruby>り、そのsource\/mapsを<ruby>拒否結果<rt>きょひけっか</rt></ruby>が<ruby>所有<rt>しょゆう</rt></ruby>する。<ruby>新<rt>あたら</rt></ruby>しい<ruby>生成<rt>せいせい</rt></ruby>source<ruby>上<rt>じょう</rt></ruby>の<ruby>診断<rt>しんだん</rt></ruby>を<ruby>返<rt>かえ</rt></ruby>す<ruby>場合<rt>ばあい</rt></ruby>は、<ruby>明示<rt>めいじ</rt></ruby>source tableを<ruby>持<rt>も</rt></ruby>つtyped TransformReplyを<ruby>使<rt>つか</rt></ruby>う。Reportが<ruby>不正<rt>ふせい</rt></ruby>な<ruby>返信<rt>へんしん</rt></ruby>は<ruby>待機<rt>たいき</rt></ruby>を<ruby>消費<rt>しょうひ</rt></ruby>せず、<ruby>訂正返信<rt>ていせいへんしん</rt></ruby>を<ruby>再検査<rt>さいけんさ</rt></ruby>できる。

この<ruby>対応<rt>たいおう</rt></ruby>の<ruby>実<rt>じつ</rt></ruby>codecは<ruby>保存要求<rt>ほぞんようきゅう</rt></ruby>proofを<ruby>使<rt>つか</rt></ruby>うTransform<ruby>返信<rt>へんしん</rt></ruby>のnative\/NDF<ruby>比較<rt>ひかく</rt></ruby>を<ruby>提供<rt>ていきょう</rt></ruby>する。ReadのMatched\/NoMatch\/NeedMoreは<ruby>正常<rt>せいじょう</rt></ruby>なreader<ruby>結果<rt>けっか</rt></ruby>としてCompleteへ<ruby>運<rt>はこ</rt></ruby>ぶ<ruby>対象<rt>たいしょう</rt></ruby>だが、その<ruby>操作返信<rt>そうさへんしん</rt></ruby>adapter、<ruby>初回<rt>しょかい</rt></ruby>provider<ruby>要求<rt>ようきゅう</rt></ruby>、<ruby>全<rt>ぜん</rt></ruby>Await<ruby>継続<rt>けいぞく</rt></ruby>、process transportの<ruby>実装完了<rt>じっそうかんりょう</rt></ruby>をこのTransform<ruby>比較<rt>ひかく</rt></ruby>から<ruby>推定<rt>すいてい</rt></ruby>しない。

<a name="n-6e6174697665"></a>

<a name="4-native-provider"></a>

## 4\. native provider

Rustの<ruby>各<rt>かく</rt></ruby>coreは<ruby>型付<rt>かたつ</rt></ruby>き<ruby>関数<rt>かんすう</rt></ruby>を<ruby>公開<rt>こうかい</rt></ruby>する。suiteの<ruby>登録時<rt>とうろくじ</rt></ruby>に、その<ruby>関数<rt>かんすう</rt></ruby>とOperationRefの<ruby>対応<rt>たいおう</rt></ruby>を<ruby>固定<rt>こてい</rt></ruby>する。wire boundaryを<ruby>通<rt>とお</rt></ruby>る<ruby>場合<rt>ばあい</rt></ruby>にだけTypedRecordとencode\/decodeを<ruby>行<rt>おこな</rt></ruby>う。<ruby>全<rt>ぜん</rt></ruby>tokenを<ruby>常<rt>つね</rt></ruby>にCBOR<ruby>化<rt>か</rt></ruby>する<ruby>設計<rt>せっけい</rt></ruby>にしない。

<ruby>呼出<rt>よびだ</rt></ruby>し<ruby>元<rt>もと</rt></ruby>へ<ruby>渡<rt>わた</rt></ruby>すparse treeやsourceは<ruby>借用<rt>しゃくよう</rt></ruby>してよいが、そのborrow\/lifetimeをoperation schemaへ<ruby>露出<rt>ろしゅつ</rt></ruby>しない。wire<ruby>要求<rt>ようきゅう</rt></ruby>では<ruby>必要<rt>ひつよう</rt></ruby>なsnapshot bundleを<ruby>明示的<rt>めいじてき</rt></ruby>な<ruby>値<rt>あたい</rt></ruby>として<ruby>渡<rt>わた</rt></ruby>す。

<a name="n-70726f63657373"></a>

<a name="5-process-provider"></a>

## 5\. process provider

apps\/providerはstdin\/stdoutの8byte unsigned big\-endian length \+ NDF frameを<ruby>処理<rt>しょり</rt></ruby>する。<ruby>最大<rt>さいだい</rt></ruby>frame<ruby>長<rt>ちょう</rt></ruby>をLimitsで<ruby>検査<rt>けんさ</rt></ruby>する。stdoutへlogを<ruby>書<rt>か</rt></ruby>かない。<ruby>失敗診断<rt>しっぱいしんだん</rt></ruby>はReply、hostの<ruby>運用<rt>うんよう</rt></ruby>logはstderr。

frame kindはInvoke \/ Resume \/ Reply \/ Cancel \/ Close。NDF payloadはProviderFrame variantとし、Invokeはcall、Resumeはresume、ReplyはrequestIdとreply、CancelはrequestId、Closeは<ruby>空<rt>から</rt></ruby>のfield<ruby>列<rt>れつ</rt></ruby>を<ruby>持<rt>も</rt></ruby>つ。InvokeとResumeのrequestIdは<ruby>内包<rt>ないほう</rt></ruby>するrecordに<ruby>保持<rt>ほじ</rt></ruby>する。Cancelは<ruby>指定要求<rt>していようきゅう</rt></ruby>の<ruby>停止<rt>ていし</rt></ruby>を<ruby>要求<rt>ようきゅう</rt></ruby>し、Closeは<ruby>接続全体<rt>せつぞくぜんたい</rt></ruby>の<ruby>終了<rt>しゅうりょう</rt></ruby>を<ruby>要求<rt>ようきゅう</rt></ruby>する。hostはCloseの<ruby>受信後<rt>じゅしんご</rt></ruby>に<ruby>新規要求<rt>しんきようきゅう</rt></ruby>の<ruby>受付<rt>うけつけ</rt></ruby>を<ruby>終了<rt>しゅうりょう</rt></ruby>し、<ruby>残存要求<rt>ざんそんようきゅう</rt></ruby>を<ruby>停止<rt>ていし</rt></ruby>して<ruby>接続<rt>せつぞく</rt></ruby>を<ruby>閉<rt>と</rt></ruby>じる。<ruby>複数要求<rt>ふくすうようきゅう</rt></ruby>をrequestIdで<ruby>識別<rt>しきべつ</rt></ruby>する。requestIdの<ruby>重複<rt>じゅうふく</rt></ruby>、<ruby>知<rt>し</rt></ruby>らないcontinuation、schema mismatchをprotocol errorにする。<ruby>再試行<rt>さいしこう</rt></ruby>は<ruby>純粋<rt>じゅんすい</rt></ruby>なoperationに<ruby>限<rt>かぎ</rt></ruby>り、<ruby>要求全体<rt>ようきゅうぜんたい</rt></ruby>が<ruby>同<rt>おな</rt></ruby>じ<ruby>場合<rt>ばあい</rt></ruby>に<ruby>行<rt>おこな</rt></ruby>う。

runtime I\/Oはこのadapter<ruby>内<rt>ない</rt></ruby>に<ruby>限定<rt>げんてい</rt></ruby>し、<ruby>各<rt>かく</rt></ruby>coreの<ruby>計算<rt>けいさん</rt></ruby>は<ruby>純粋入力<rt>じゅんすいにゅうりょく</rt></ruby>\/<ruby>出力<rt>しゅつりょく</rt></ruby>に<ruby>保<rt>たも</rt></ruby>つ。processによる<ruby>別実装<rt>べつじっそう</rt></ruby>はnative APIの<ruby>代替<rt>だいたい</rt></ruby>として<ruby>使<rt>つか</rt></ruby>える。Wasm<ruby>用<rt>よう</rt></ruby>adapterも<ruby>同<rt>おな</rt></ruby>じCall\/Replyを<ruby>利用<rt>りよう</rt></ruby>でき、wasm32\-wasip2<ruby>自体<rt>じたい</rt></ruby>をこのwire ABIの<ruby>別名<rt>べつめい</rt></ruby>としない。

<a name="n-7265706c6163656d656e74"></a>

<a name="6-置換手順"></a>

## 6\. <ruby>置換手順<rt>ちかんてじゅん</rt></ruby>

<ruby>一<rt>ひと</rt></ruby>つのoperationについて<ruby>新実装<rt>しんじっそう</rt></ruby>を<ruby>登録<rt>とうろく</rt></ruby>し、<ruby>同<rt>おな</rt></ruby>じconformance<ruby>入力<rt>にゅうりょく</rt></ruby>をnative Rust<ruby>経路<rt>けいろ</rt></ruby>とNDF<ruby>経路<rt>けいろ</rt></ruby>へ<ruby>渡<rt>わた</rt></ruby>す。<ruby>比較対象<rt>ひかくたいしょう</rt></ruby>は<ruby>意味正規形<rt>いみせいきけい</rt></ruby>、<ruby>定義済<rt>ていぎず</rt></ruby>みdiagnostic code\/<ruby>位置<rt>いち</rt></ruby>、<ruby>参照先<rt>さんしょうさき</rt></ruby>、source<ruby>対応<rt>たいおう</rt></ruby>、<ruby>出力<rt>しゅつりょく</rt></ruby>artifactのcanonical<ruby>内容<rt>ないよう</rt></ruby>。traceの<ruby>内部手順<rt>ないぶてじゅん</rt></ruby>や<ruby>実行時間<rt>じっこうじかん</rt></ruby>は<ruby>比較対象外<rt>ひかくたいしょうがい</rt></ruby>。

<ruby>新実装<rt>しんじっそう</rt></ruby>がpassingになったoperationだけdispatchを<ruby>切<rt>き</rt></ruby>り<ruby>替<rt>か</rt></ruby>える。<ruby>残<rt>のこ</rt></ruby>りをRustで<ruby>実行<rt>じっこう</rt></ruby>する<ruby>混在<rt>こんざい</rt></ruby>を<ruby>許<rt>ゆる</rt></ruby>す。<ruby>各<rt>かく</rt></ruby>coreが<ruby>別言語<rt>べつげんご</rt></ruby>coreへ<ruby>直接依存<rt>ちょくせついぞん</rt></ruby>していないため、<ruby>単一言語<rt>たんいつげんご</rt></ruby>・<ruby>単一操作<rt>たんいつそうさ</rt></ruby>から<ruby>交換<rt>こうかん</rt></ruby>できる。

<ruby>最初<rt>さいしょ</rt></ruby>のRust<ruby>実装段階<rt>じっそうだんかい</rt></ruby>からencode\/decode loopbackと<ruby>別<rt>べつ</rt></ruby>process providerを<ruby>受入対象<rt>うけいれたいしょう</rt></ruby>にする。「<ruby>後<rt>あと</rt></ruby>でABIを<ruby>決<rt>き</rt></ruby>める」<ruby>作業<rt>さぎょう</rt></ruby>を<ruby>残<rt>のこ</rt></ruby>さない。ただし<ruby>将来言語<rt>しょうらいげんご</rt></ruby>のコンパイラ・runtimeそのものが<ruby>既<rt>すで</rt></ruby>に<ruby>存在<rt>そんざい</rt></ruby>すると<ruby>仮定<rt>かてい</rt></ruby>しない。
