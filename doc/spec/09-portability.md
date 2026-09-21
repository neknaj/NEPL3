<!-- Generated from doc/spec/09&#45;portability.nepld; renderer nepl3-tools.markdown-annotated-pages/1; page portability; source SHA-256 5d455bc42fe39b5b3bf30f414f8086118ece1d450a5977ab34d499afb426def1; alias input SHA-256 f6e5ea8b221e730a8d50ed28afc74cb0afd9831704b5f19b3729abf52cd8cd9d; document digest 1d7d3f40e6be027eda6cc1c2ccd25cf7e36ad8e80e93d1d414f253e4dcf03506; input PageSet digest 54e863d523a3a12f0d21fb4991e4ea02d5e7a006a308d5bfb7313c688b93251c; input context SHA-256 bc1cada6a66c44f3080f3a62fb3ed4828532f2fc8d127c587e7bf6677763815e. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="09-rust以外へ置換するための契約"></a>

# 09\. Rust以外\[いがい\]へ置換\[ちかん\]するための契約\[けいやく\]

この章\[しょう\]は公開境界\[こうかいきょうかい\]と置換\[ちかん\]の目標契約\[もくひょうけいやく\]を定\[さだ\]める。全\[ぜん\]provider・process transport・全\[ぜん\]targetの実装\[じっそう\]と受入完了\[うけいれかんりょう\]を宣言\[せんげん\]するものではない。現在\[げんざい\]の実装\[じっそう\]と受入状態\[うけいれじょうたい\]はimplementation\-status\.jsonで管理\[かんり\]する。

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## 方針\[ほうしん\]

意味\[いみ\]モデル・操作\[そうさ\]・診断\[しんだん\]・source対応\[たいおう\]のschemaを公開境界\[こうかいきょうかい\]とする。Rustのメモリ表現\[ひょうげん\]やtrait objectをABIにしない。native fast pathとportable pathを同時\[どうじ\]に提供\[ていきょう\]し、その意味\[いみ\]を比較\[ひかく\]する。

<a name="n-6c6179657273"></a>

<a name="1-三つの層"></a>

## 1\. 三\[みっ\]つの層\[そう\]

1. 言語中立\[げんごちゅうりつ\]のschema\: field順\[じゅん\]、variant、必須制約\[ひっすせいやく\]、整数\[せいすう\]\/有理数\[ゆうりすう\]、範囲\[はんい\]と参照\[さんしょう\]、operation signature。
1. Rustの型付\[かたつ\]きAPI\: schemaに対応\[たいおう\]するstruct\/enumと関数\[かんすう\]。Rust内\[ない\]の通常利用\[つうじょうりよう\]ではserialize不要\[ふよう\]。
1. transport adapter\: schema値\[あたい\]をNDF（下記\[かき\]）でencode\/decodeし、別\[べつ\]process\/別実装\[べつじっそう\]へ渡\[わた\]す。

この分離\[ぶんり\]により、将来\[しょうらい\]のNEPL3プログラミング言語\[げんご\]は同\[おな\]じschemaを生成\[せいせい\]\/消費\[しょうひ\]する実装\[じっそう\]を持\[も\]てばよい。現在\[げんざい\]のRustのallocator、enum discriminant、pointer、Arc\/Rc、usize、trait vtableは境界\[きょうかい\]を越\[こ\]えない。

<a name="n-6e6466"></a>

<a name="2-ndf1"></a>

## 2\. NDF\/1

canonical値\[あたい\]digestは `SHA-256(domain || canonical NDF/1 CBOR(value))` とする。digest専用経路\[せんようけいろ\]は通常\[つうじょう\]のencoderと同\[おな\]じ符号化規則\[ふごうかきそく\]でbyte片\[へん\]を順次\[じゅんじ\]hashへ渡\[わた\]してよく、完全\[かんぜん\]なCBOR bufferの作成\[さくせい\]を必須\[ひっす\]にしない。この場合\[ばあい\]も値\[あたい\]の走査\[そうさ\]・整数\[せいすう\]の一時表現\[いちじひょうげん\]・stackの割当\[わりあて\]と、符号化\[ふごうか\]およびhashの全\[ぜん\]byte処理\[しょり\]を同\[おな\]じBudgetへ先行課金\[せんこうかきん\]し、停止時\[ていしじ\]はdigestを返\[かえ\]さない。出力\[しゅつりょく\]は32byteのdigestとして課金\[かきん\]する。CBOR bufferを実際\[じっさい\]に作\[つく\]る経路\[けいろ\]ではその出力\[しゅつりょく\]・割当\[わりあて\]も課金\[かきん\]し、会計\[かいけい\]だけを省略\[しょうりゃく\]してはならない。通常\[つうじょう\]のNDF encodeは引\[ひ\]き続\[つづ\]き生成\[せいせい\]する全\[ぜん\]CBOR byteを出力\[しゅつりょく\]として課金\[かきん\]する。文書\[ぶんしょ\]digestを得\[え\]るための一時\[いちじ\]CBORを除去\[じょきょ\]することと、HTMLの出力上限\[しゅつりょくじょうげん\]を変更\[へんこう\]することは別\[べつ\]である。

NDFは本仕様\[ほんしよう\]の型付\[かたつ\]き値\[あたい\]をCBORで運\[はこ\]ぶ符号化\[ふごうか\]profile。RFC 8949のdefinite lengthと最短\[さいたん\]の整数\[せいすう\]\/長\[なが\]さ表現\[ひょうげん\]を要求\[ようきゅう\]する。map、float、NaN、CBOR tag、null、indefinite lengthはNDFでは使用\[しよう\]しない。CBORそのものの汎用機能\[はんようきのう\]を全部許可\[ぜんぶきょか\]するわけではない。

値\[あたい\]は次\[つぎ\]のtagged arrayで表\[あらわ\]す。

| tag | 配列\[はいれつ\] | 型\[かた\] |
| ---: | --- | --- |
| 0 | `[0]` | Unit |
| 1 | `[1, bool]` | Bool |
| 2 | `[2, uint64]` | Offset\/Index等\[など\]の有限\[ゆうげん\]unsigned |
| 3 | `[3, negative:bool, magnitude:bytes]` | 任意精度\[にんいせいど\]Integer |
| 4 | `[4, numerator:Value(tag3), denominator:bytes]` | Rational |
| 5 | `[5, text]` | Text（valid UTF\-8） |
| 6 | `[6, bytes]` | Bytes |
| 7 | `[7, [Value...]]` | List |
| 8 | `[8]` | None |
| 9 | `[9, Value]` | Some |
| 10 | `[10, SchemaRef, KindName, [Value...]]` | Record、field順\[じゅん\]はschema |
| 11 | `[11, SchemaRef, TypeName, VariantName, [Value...]]` | Variant |

SchemaRefは `[packageName:text, revision:uint64, digest:bytes32]`。型名\[かためい\]\/kind名\[めい\]は登録済\[とうろくず\]みschemaに照合\[しょうごう\]する。naturalはtag3のnonnegativeを要求\[ようきゅう\]するfield制約\[せいやく\]。integer magnitudeはbig\-endian、先頭\[せんとう\]zeroなし、zeroはempty bytesかつnegative\=false。rational denominatorは正\[せい\]で先頭\[せんとう\]zeroなし、gcd\=1、zero numeratorはdenominator\=1。

NodeId\/EntityId等\[など\]は公開\[こうかい\]record内\[ない\]のindexとしてencodeし、同\[おな\]じbundleに対応\[たいおう\]するtableとschemaを含\[ふく\]める。未定義参照\[みていぎさんしょう\]、循環禁止構造\[じゅんかんきんしこうぞう\]の循環\[じゅんかん\]、重複\[じゅうふく\]ID、範囲外\[はんいがい\]spanはdecode直後\[ちょくご\]に拒否\[きょひ\]する。文字列型\[もじれつがた\]の違反\[いはん\]を遅\[おく\]れてRustのdowncastで検出\[けんしゅつ\]する方式\[ほうしき\]にしない。

`interfaces/contracts.json` と各言語\[かくげんご\]constructor表\[ひょう\]からcodecの網羅性検査\[もうらせいけんさ\]を生成\[せいせい\]する。serdeのderive既定表現\[きていひょうげん\]を契約\[けいやく\]にせず、custom codecでNDFへ写\[うつ\]す。内部\[ないぶ\]でserdeを使\[つか\]う場合\[ばあい\]も明示\[めいじ\]したarray\/schemaに固定\[こてい\]する。

<a name="n-696e7472696e736963"></a>

<a name="21-intrinsic値の閉じた記述"></a>

### 2\.1 intrinsic値\[あたい\]の閉\[と\]じた記述\[きじゅつ\]

`interfaces/contracts.json` の `intrinsic_types` は、NDF\/1をdecodeした論理値\[ろんりち\]の型\[かた\]を定義\[ていぎ\]する。`NdfValue` の12caseは上表\[じょうひょう\]のtag 0–11に一対一\[いちたいいち\]で対応\[たいおう\]する。`NdfScalar` はUnit、Bool、U64、Integer、Rational、Text、Bytesの部分型\[ぶぶんがた\]、`TypedValue` はRecord、Variantだけの部分型\[ぶぶんがた\]である。subsetのcase列\[れつ\]は集合\[しゅうごう\]であり、順序\[じゅんじょ\]をwire IDにしない。Unitは追加\[ついか\]payloadを持\[も\]たず、Noneとは異\[こと\]なる。NaturalはIntegerの非負制約\[ひふせいやく\]、Bytes32はBytesの長\[なが\]さ制約\[せいやく\]であり、追加\[ついか\]のwire tagを持\[も\]たない。

このintrinsic記述\[きじゅつ\]は通常\[つうじょう\]のdomain sumではない。たとえば `NdfValue.Integer(value: Integer)` の論理\[ろんり\]fieldを一般\[いっぱん\]のVariantとしてtag11で包\[つつ\]まず、上表\[じょうひょう\]のtag3へ直接写\[ちょくせつうつ\]す。Integerのnegative\/magnitude、Rationalの分子\[ぶんし\]tag3と分母\[ぶんぼ\]bytesは上表\[じょうひょう\]の専用表現\[せんようひょうげん\]を使\[つか\]う。tag10\/11のfieldsは裸\[はだか\]のCBOR arrayであり、Listのtag7を付\[つ\]けない。そのheader内\[ない\]のSchemaRefも裸\[はだか\]の `[text,uint64,bytes32]` であり、通常\[つうじょう\]のRecordのtag10を付\[つ\]けない。論理\[ろんり\]fieldのTextやU64も、上表\[じょうひょう\]で裸\[はだか\]のCBOR text\/uint64を指定\[してい\]する位置\[いち\]へ余分\[よぶん\]なNDF tagを付\[つ\]けない。codecの物理表現\[ぶつりひょうげん\]は上表\[じょうひょう\]を正本\[せいほん\]とする。

intrinsicの識別子\[しきべつし\] `nepl3.ndf/1` は本符号化\[ほんふごうか\]profileに組\[く\]み込\[こ\]まれた固定識別子\[こていしきべつし\]である。intrinsic自体\[じたい\]へdomain SchemaRefや自己\[じこ\]hashを要求\[ようきゅう\]しない。Record\/Variant headerのSchemaRefは運\[はこ\]ばれるdomain値\[あたい\]のdescriptorを識別\[しきべつ\]し、intrinsicの識別子\[しきべつし\]とは別物\[べつもの\]である。domain descriptorの登録\[とうろく\]・digest検証\[けんしょう\]は必要\[ひつよう\]であり、TypedValueという型名\[かためい\]やtag10\/11であることだけでは検査済\[けんさず\]みにならない。通常\[つうじょう\]のfieldとしてのSchemaRefは `nepl3.foundation` revision 1のSchemaRef recordをtag10で運\[はこ\]ぶ。そのheaderのSchemaRefだけが裸\[はだか\]のtupleとなるため、無限\[むげん\]のwrapper再帰\[さいき\]は生\[しょう\]じない。

<a name="n-7265666572656e636573"></a>

<a name="22-descriptorの型参照検査と残る契約"></a>

### 2\.2 descriptorの型参照検査\[かたさんしょうけんさ\]と残\[のこ\]る契約\[けいやく\]

fieldとunion参照\[さんしょう\]の型式\[かたしき\]は `Name | List<Type> | Option<Type>` とする。NameはASCII英字\[えいじ\]で始\[はじ\]まる英数字\[えいすうじ\]\/underscoreのsegmentを `:` または `/` で接続\[せつぞく\]する。空\[くう\]segment、空白\[くうはく\]、余分\[よぶん\]なtoken、未宣言\[みせんげん\]のgeneric、引数\[ひきすう\]の過不足\[かふそく\]は拒否\[きょひ\]する。List\/Optionは予約\[よやく\]された1引数\[ひきすう\]constructorであり、名義型\[めいぎがた\]として宣言\[せんげん\]できない。開発\[かいはつ\]checkerは128階層\[かいそう\]を超\[こ\]える型式\[かたしき\]を拒否\[きょひ\]する。この型名文法\[かためいぶんぽう\]と上限\[じょうげん\]は設計\[せっけい\]descriptorの記述\[きじゅつ\]に限\[かぎ\]り、利用者\[りようしゃ\]の言語中\[げんごちゅう\]の名前\[なまえ\]・kind文字列\[もじれつ\]・source・NDF Textの文字集合\[もじしゅうごう\]を制限\[せいげん\]しない。runtime入力\[にゅうりょく\]のLimitsも代替\[だいたい\]しない。

builtinはUnit、Bool、U64、Integer、Natural、Rational、Text、Bytes、Bytes32である。所有者\[しょゆうしゃ\]は本\[ほん\]profileであり、modelのscalar\_typesとcontractsのscalar\_aliasesはそれへの参照\[さんしょう\]・説明\[せつめい\]である。model\.types、contracts\.records\/enums\/intrinsic\_typesの名義型定義\[めいぎがたていぎ\]は重複\[じゅうふく\]を許\[ゆる\]さない。modelの外部参照\[がいぶさんしょう\]はexternal\_typesへ明示\[めいじ\]し、存在\[そんざい\]する外部所有型\[がいぶしょゆうがた\]へ解決\[かいけつ\]する。modelの可視名\[かしめい\]は自身\[じしん\]の定義\[ていぎ\]と明示\[めいじ\]したscalar\/external importに限\[かぎ\]る。既存\[きそん\]の再帰的\[さいきてき\]な型\[かた\]graphは許\[ゆる\]すが、値\[あたい\]の循環可否\[じゅんかんかひ\]・source\/Origin tableの整合\[せいごう\]は操作\[そうさ\]ごとの値検査\[あたいけんさ\]で別\[べつ\]に判定\[はんてい\]する。

型名\[かためい\]がすべて解決\[かいけつ\]することと公開操作契約\[こうかいそうさけいやく\]が完成\[かんせい\]することを区別\[くべつ\]する。27操作\[そうさ\]のinput\/outputには説明用\[せつめいよう\]の式\[しき\]が残\[のこ\]り、型付\[かたつ\]きの具体化\[ぐたいか\]・provider frame・Profile・Doc移行\[いこう\]のDG01–DG06は対応\[たいおう\]する実装\[じっそう\]とともに完成\[かんせい\]させる。進捗\[しんちょく\]と実行証拠\[じっこうしょうこ\]は [実装記録\[じっそうきろく\]](<\.\.\/progress\/foundation\-runtime\.md>) を参照\[さんしょう\]する。

<a name="n-64657363726970746f72"></a>

<a name="23-実行可能なschema-descriptor"></a>

### 2\.3 実行可能\[じっこうかのう\]なschema descriptor

`interfaces/contracts.json` のTypeDescriptorはUnit\/Bool\/U64\/Integer\/Natural\/Rational\/Text\/Bytes\/Bytes32、intrinsicのNdfValue\/NdfScalar\/TypedValue、List、Option、Namedを区別\[くべつ\]する。Namedはpackage\/revision\/nameの記号参照\[きごうさんしょう\]でありdigestを入\[い\]れない。Recordは順序付\[じゅんじょつ\]きfield、Variantは名前付\[なまえつ\]きのvariantと順序付\[じゅんじょつ\]きpayloadを持\[も\]つ。constraintsはschema所有\[しょゆう\]の意味制約\[いみせいやく\]IDの集合\[しゅうごう\]である。

SchemaDescriptorはpackage、revision、types、operationsを持\[も\]つ。canonical JSONではtypesとoperationsを名前\[なまえ\]keyのobject、型定義\[かたていぎ\]を `{constraints,record}` または `{constraints,variant}` とし、fieldを `[name,TypeJSON]` の順序付\[じゅんじょつ\]きarrayにする。TypeJSONはbuiltin\/intrinsic名\[めい\]の文字列\[もじれつ\]、`{list:TypeJSON}`、`{option:TypeJSON}`、`{named:{name,package,revision}}` のいずれか。operationは `{input,output,pure}`。constraintsは重複\[じゅうふく\]を拒否\[きょひ\]して名前順\[なまえじゅん\]に並\[なら\]べる。13章\[しょう\]のkey順\[じゅん\]・escape・domain separatorでSHA\-256を求\[もと\]める。

登録\[とうろく\]は期待\[きたい\]するSchemaRefとdescriptorの計算\[けいさん\]digestを照合\[しょうごう\]する。一\[ひと\]つのregistryで同\[おな\]じpackage\/revisionに異\[こと\]なるdigestを同時選択\[どうじせんたく\]しない。全\[ぜん\]packageを登録後\[とうろくご\]にfinalizeし、使用\[しよう\]されていないvariantやoperationも含\[ふく\]むすべてのNamed参照\[さんしょう\]を解決\[かいけつ\]する。相互参照\[そうごさんしょう\]する型\[かた\]・packageの登録\[とうろく\]は許\[ゆる\]すが、finalize前\[まえ\]の値検査\[あたいけんさ\]・実行\[じっこう\]は拒否\[きょひ\]する。

`interfaces/foundation.json` はcontractsから `cargo run --locked -p nepl3-tools -- foundation --write` で生成\[せいせい\]する実際\[じっさい\]の `nepl3.foundation` descriptorである。build\.rsから生成\[せいせい\]せず、通常\[つうじょう\]の検査\[けんさ\]で正本\[せいほん\]との一致\[いっち\]とproduction core registryによる登録\[とうろく\]・参照閉包\[さんしょうへいほう\]を検査\[けんさ\]する。未完成\[みかんせい\]のoperation説明表\[せつめいひょう\]を実行可能\[じっこうかのう\]なoperationsへコピーしない。このpackageは共通\[きょうつう\]の値\[あたい\]・transport recordのschemaを提供\[ていきょう\]し、言語操作\[げんごそうさ\]の実装\[じっそう\]を広告\[こうこく\]しない。

同\[おな\]じ明示生成\[めいじせいせい\]で `crates/foundation/core/src/schema/foundation.rs` を作\[つく\]り、productionの `foundation::descriptor` が型付\[かたつ\]きdescriptorを構築\[こうちく\]する。構築前\[こうちくまえ\]に割当\[わりあて\]・work予算\[よさん\]を計上\[けいじょう\]し、toolsのJSON parserをproductionへ依存\[いぞん\]させない。通常\[つうじょう\]の検査\[けんさ\]はJSONとRust投影\[とうえい\]の両方\[りょうほう\]を正本\[せいほん\]と比較\[ひかく\]する。wireのsource\/Span adapterはこのdescriptorを登録\[とうろく\]したregistryで構造\[こうぞう\]を検査\[けんさ\]し、さらにsource digest・宣言順\[せんげんじゅん\]・identity・locator・snapshot・UTF\-8境界\[きょうかい\]を検査\[けんさ\]してnative型\[がた\]へ戻\[もど\]す。

raw encode\/decodeはNDF intrinsicのcanonical性\[せい\]を検査\[けんさ\]する。公開操作\[こうかいそうさ\]の境界\[きょうかい\]ではexpected TypeDescriptorとfinalize済\[ず\]みregistryを渡\[わた\]すchecked encode\/decodeを使\[つか\]い、受信\[じゅしん\]したschema\/kind\/variant\/field型\[がた\]を照合\[しょうごう\]する。得\[え\]られるStructuralValueは構造検査\[こうぞうけんさ\]の証明\[しょうめい\]であり、constraintsに列挙\[れっきょ\]したsourceの対応\[たいおう\]・回路\[かいろ\]の幅等\[はばなど\]の意味検査\[いみけんさ\]を代替\[だいたい\]しない。coreの対応\[たいおう\]constructorまたはdomain操作\[そうさ\]で必要\[ひつよう\]な不変条件\[ふへんじょうけん\]を検査\[けんさ\]してから使用\[しよう\]する。

<a name="n-696e766f636174696f6e"></a>

<a name="3-操作呼出し"></a>

## 3\. 操作呼出\[そうさよびだ\]し

CallはrequestId、OperationRef、input TypedRecord、sources\/resources\/environment、Limitsを持\[も\]つ。OperationRefはpackage\/revision\/digestとoperation名\[めい\]。provider manifestにinput\/output schema、必要\[ひつよう\]capability、純粋性契約\[じゅんすいせいけいやく\]を宣言\[せんげん\]する。

ReplyはComplete \/ Invalid \/ Stopped \/ Awaitのvariant。DiagnosticとEventは成功\[せいこう\]の値\[あたい\]と別\[べつ\]field。未対応\[みたいおう\]operationを空\[から\]の値\[あたい\]や成功\[せいこう\]Unitで返\[かえ\]さない。

Awaitは外部\[がいぶ\]service要求\[ようきゅう\]とcontinuationを返\[かえ\]す。hostはallowlistで照合\[しょうごう\]し、同一親予算\[どういつおやよさん\]で要求\[ようきゅう\]を実行\[じっこう\]してresumeする。continuationはprovider revision、親\[おや\]request、snapshot digestに束縛\[そくばく\]し、別\[べつ\]providerへ渡\[わた\]せない。中断中\[ちゅうだんちゅう\]の要求\[ようきゅう\]を実装差替\[じっそうさしか\]え対象\[たいしょう\]にしない。

call graphはhostが追跡\[ついせき\]する。同\[おな\]じoperation\/input\/contextの循環依存\[じゅんかんいぞん\]はCyclicOperation。有限\[ゆうげん\]だが大\[おお\]きい再帰\[さいき\]も共通\[きょうつう\]Limitsで停止\[ていし\]できる。provider内部\[ないぶ\]のアルゴリズム固有\[こゆう\]costは性能情報\[せいのうじょうほう\]であり、二実装\[にじっそう\]でusageの数値一致\[すうちいっち\]を互換要件\[ごかんようけん\]にしない。

<a name="n-7472616e73666f726d"></a>

<a name="31-reader-transformの操作返信"></a>

### 3\.1 Reader Transformの操作返信\[そうさへんしん\]

Transformのdomain結果\[けっか\]と外側\[そとがわ\]OperationReplyは次\[つぎ\]のように対応\[たいおう\]する。内側\[うちがわ\]のtyped TransformReplyはsources\/sourceMapsを全結果\[ぜんけっか\]で所有\[しょゆう\]する。

| TransformOutcome | OperationReply | typed payload |
| --- | --- | --- |
| Complete | Complete | TransformReply |
| Failed | Invalid | partial\=Some\(TransformReply\) |
| Stopped\(reason\) | Stopped\(reason\) | partial\=Some\(TransformReply\) |

内外\[ないがい\]のdiagnostics\/events\/usage\/traceOverflowは同\[おな\]じReportの正確\[せいかく\]な再掲\[さいけい\]であり、不一致\[ふいっち\]を拒否\[きょひ\]する。再掲\[さいけい\]によるstorageと符号化\[ふごうか\]の費用\[ひよう\]は計上\[けいじょう\]するが、診断\[しんだん\]・eventを再発行\[さいはっこう\]したことにはしない。Stoppedの理由\[りゆう\]も一致\[いっち\]を要求\[ようきゅう\]する。CompleteにFailedを隠\[かく\]す、Invalidを成功\[せいこう\]Unitに変\[か\]える、Transformに未定義\[みていぎ\]のAwaitを返\[かえ\]す、といった返信\[へんしん\]は拒否\[きょひ\]する。

partial\=NoneのInvalid\/Stoppedは、domain Transform結果\[けっか\]が得\[え\]られる前\[まえ\]のdispatch失敗\[しっぱい\]である。adapterは型付\[かたつ\]きの拒否結果\[きょひけっか\]と正式\[せいしき\]Reportをhostへ返\[かえ\]し、Readerの待機\[たいき\]slotへ成功\[せいこう\]やdomain失敗\[しっぱい\]として適用\[てきよう\]しない。この場合\[ばあい\]の位置参照\[いちさんしょう\]は元\[もと\]の保存要求\[ほぞんようきゅう\]と正式\[せいしき\]checkpointのsource閉包\[へいほう\]だけに限\[かぎ\]り、そのsource\/mapsを拒否結果\[きょひけっか\]が所有\[しょゆう\]する。新\[あたら\]しい生成\[せいせい\]source上\[じょう\]の診断\[しんだん\]を返\[かえ\]す場合\[ばあい\]は、明示\[めいじ\]source tableを持\[も\]つtyped TransformReplyを使\[つか\]う。Reportが不正\[ふせい\]な返信\[へんしん\]は待機\[たいき\]を消費\[しょうひ\]せず、訂正返信\[ていせいへんしん\]を再検査\[さいけんさ\]できる。

この対応\[たいおう\]の実\[じつ\]codecは保存要求\[ほぞんようきゅう\]proofを使\[つか\]うTransform返信\[へんしん\]のnative\/NDF比較\[ひかく\]を提供\[ていきょう\]する。ReadのMatched\/NoMatch\/NeedMoreは正常\[せいじょう\]なreader結果\[けっか\]としてCompleteへ運\[はこ\]ぶ対象\[たいしょう\]だが、その操作返信\[そうさへんしん\]adapter、初回\[しょかい\]provider要求\[ようきゅう\]、全\[ぜん\]Await継続\[けいぞく\]、process transportの実装完了\[じっそうかんりょう\]をこのTransform比較\[ひかく\]から推定\[すいてい\]しない。

<a name="n-6e6174697665"></a>

<a name="4-native-provider"></a>

## 4\. native provider

Rustの各\[かく\]coreは型付\[かたつ\]き関数\[かんすう\]を公開\[こうかい\]する。suiteの登録時\[とうろくじ\]に、その関数\[かんすう\]とOperationRefの対応\[たいおう\]を固定\[こてい\]する。wire boundaryを通\[とお\]る場合\[ばあい\]にだけTypedRecordとencode\/decodeを行\[おこな\]う。全\[ぜん\]tokenを常\[つね\]にCBOR化\[か\]する設計\[せっけい\]にしない。

呼出\[よびだ\]し元\[もと\]へ渡\[わた\]すparse treeやsourceは借用\[しゃくよう\]してよいが、そのborrow\/lifetimeをoperation schemaへ露出\[ろしゅつ\]しない。wire要求\[ようきゅう\]では必要\[ひつよう\]なsnapshot bundleを明示的\[めいじてき\]な値\[あたい\]として渡\[わた\]す。

<a name="n-70726f63657373"></a>

<a name="5-process-provider"></a>

## 5\. process provider

apps\/providerはstdin\/stdoutの8byte unsigned big\-endian length \+ NDF frameを処理\[しょり\]する。最大\[さいだい\]frame長\[ちょう\]をLimitsで検査\[けんさ\]する。stdoutへlogを書\[か\]かない。失敗診断\[しっぱいしんだん\]はReply、hostの運用\[うんよう\]logはstderr。

frame kindはInvoke \/ Resume \/ Reply \/ Cancel \/ Close。複数要求\[ふくすうようきゅう\]をrequestIdで識別\[しきべつ\]する。requestIdの重複\[じゅうふく\]、知\[し\]らないcontinuation、schema mismatchをprotocol errorにする。再試行\[さいしこう\]は純粋\[じゅんすい\]なoperationに限\[かぎ\]り、要求全体\[ようきゅうぜんたい\]が同\[おな\]じ場合\[ばあい\]に行\[おこな\]う。

runtime I\/Oはこのadapter内\[ない\]に限定\[げんてい\]し、各\[かく\]coreの計算\[けいさん\]は純粋入力\[じゅんすいにゅうりょく\]\/出力\[しゅつりょく\]に保\[たも\]つ。processによる別実装\[べつじっそう\]はnative APIの代替\[だいたい\]として使\[つか\]える。Wasm用\[よう\]adapterも同\[おな\]じCall\/Replyを利用\[りよう\]でき、wasm32\-wasip2自体\[じたい\]をこのwire ABIの別名\[べつめい\]としない。

<a name="n-7265706c6163656d656e74"></a>

<a name="6-置換手順"></a>

## 6\. 置換手順\[ちかんてじゅん\]

一\[ひと\]つのoperationについて新実装\[しんじっそう\]を登録\[とうろく\]し、同\[おな\]じconformance入力\[にゅうりょく\]をnative Rust経路\[けいろ\]とNDF経路\[けいろ\]へ渡\[わた\]す。比較対象\[ひかくたいしょう\]は意味正規形\[いみせいきけい\]、定義済\[ていぎず\]みdiagnostic code\/位置\[いち\]、参照先\[さんしょうさき\]、source対応\[たいおう\]、出力\[しゅつりょく\]artifactのcanonical内容\[ないよう\]。traceの内部手順\[ないぶてじゅん\]や実行時間\[じっこうじかん\]は比較対象外\[ひかくたいしょうがい\]。

新実装\[しんじっそう\]がpassingになったoperationだけdispatchを切\[き\]り替\[か\]える。残\[のこ\]りをRustで実行\[じっこう\]する混在\[こんざい\]を許\[ゆる\]す。各\[かく\]coreが別言語\[べつげんご\]coreへ直接依存\[ちょくせついぞん\]していないため、単一言語\[たんいつげんご\]・単一操作\[たんいつそうさ\]から交換\[こうかん\]できる。

最初\[さいしょ\]のRust実装段階\[じっそうだんかい\]からencode\/decode loopbackと別\[べつ\]process providerを受入対象\[うけいれたいしょう\]にする。「後\[あと\]でABIを決\[き\]める」作業\[さぎょう\]を残\[のこ\]さない。ただし将来言語\[しょうらいげんご\]のコンパイラ・runtimeそのものが既\[すで\]に存在\[そんざい\]すると仮定\[かてい\]しない。
