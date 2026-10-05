<!-- Generated from doc/spec/references.nepld; renderer nepl3-tools.markdown-annotated/4; source SHA-256 c1dfe540bd87470fd3b301f9a864fc9496e4eb03f2954a148590c727234f06d1; alias input SHA-256 ca48ebda933f38af0d21841dc5cc9d221771c9b2e7bc3db46ed528fdebc1f14d. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="根拠と参照資料"></a>

# <ruby>根拠<rt>こんきょ</rt></ruby>と<ruby>参照資料<rt>さんしょうしりょう</rt></ruby>

[正本（NEPL3d）](<references.nepld>)

<ruby>取得日<rt>しゅとくび</rt></ruby>\: 2026\-09\-06（Asia\/Tokyo）。<ruby>外部仕様<rt>がいぶしよう</rt></ruby>の<ruby>説明<rt>せつめい</rt></ruby>とNEPL3<ruby>自身<rt>じしん</rt></ruby>の<ruby>新規設計<rt>しんきせっけい</rt></ruby>を<ruby>区別<rt>くべつ</rt></ruby>する。

<a name="n-726571756972656d656e7473"></a>

<a name="本人の要件"></a>

## <ruby>本人<rt>ほんにん</rt></ruby>の<ruby>要件<rt>ようけん</rt></ruby>

<ruby>本会話<rt>ほんかいわ</rt></ruby>の<ruby>最新指示<rt>さいしんしじ</rt></ruby>\: 4<ruby>言語<rt>げんご</rt></ruby>と<ruby>共通基盤<rt>きょうつうきばん</rt></ruby>をRustで<ruby>実装<rt>じっそう</rt></ruby>、<ruby>後<rt>のち</rt></ruby>のNEPL3<ruby>言語<rt>げんご</rt></ruby>への<ruby>置換<rt>ちかん</rt></ruby>、<ruby>依存関係<rt>いぞんかんけい</rt></ruby>、sentence literal\/prefix<ruby>両経路<rt>りょうけいろ</rt></ruby>、sentence<ruby>単位<rt>たんい</rt></ruby>parallel、<ruby>汎用<rt>はんよう</rt></ruby>kind、editor<ruby>支援<rt>しえん</rt></ruby>、<ruby>暫定設計<rt>ざんていせっけい</rt></ruby>を<ruby>認<rt>みと</rt></ruby>めない。

<ruby>元<rt>もと</rt></ruby>の<ruby>設計資料<rt>せっけいしりょう</rt></ruby>は、ChatGPTのLibraryに<ruby>保存<rt>ほぞん</rt></ruby>された2026\-09\-05の<ruby>方針<rt>ほうしん</rt></ruby>・<ruby>構文基盤<rt>こうぶんきばん</rt></ruby>・<ruby>意味論<rt>いみろん</rt></ruby>・API・<ruby>検証計画<rt>けんしょうけいかく</rt></ruby>・apply<ruby>追補<rt>ついほ</rt></ruby>を<ruby>参照<rt>さんしょう</rt></ruby>したと<ruby>記録<rt>きろく</rt></ruby>している。それらのLibrary<ruby>本文<rt>ほんぶん</rt></ruby>はこのリポジトリへ<ruby>提供<rt>ていきょう</rt></ruby>されていないため、<ruby>本整備<rt>ほんせいび</rt></ruby>では<ruby>直接確認<rt>ちょくせつかくにん</rt></ruby>していない。<ruby>本<rt>ほん</rt></ruby>リポジトリの<ruby>契約<rt>けいやく</rt></ruby>は<ruby>提供<rt>ていきょう</rt></ruby>された<ruby>会話<rt>かいわ</rt></ruby>と74ファイルの<ruby>設計資料<rt>せっけいしりょう</rt></ruby>に<ruby>基<rt>もと</rt></ruby>づく。<ruby>来歴<rt>らいれき</rt></ruby>は `doc/history/README.md` を<ruby>参照<rt>さんしょう</rt></ruby>。

<a name="n-736f7572636573"></a>

<a name="公開資料"></a>

## <ruby>公開資料<rt>こうかいしりょう</rt></ruby>

- <ruby>本人<rt>ほんにん</rt></ruby>の<ruby>設計指針<rt>せっけいししん</rt></ruby>\: [https\:\/\/zenn\.dev\/bem130\/articles\/1b352797de94e7](<https\:\/\/zenn\.dev\/bem130\/articles\/1b352797de94e7>)
- Cargo feature<ruby>設計<rt>せっけい</rt></ruby>\: [https\:\/\/doc\.rust\-lang\.org\/cargo\/reference\/features\.html](<https\:\/\/doc\.rust\-lang\.org\/cargo\/reference\/features\.html>)
- Rust edition 2024 \/ resolver 3\: [https\:\/\/doc\.rust\-lang\.org\/edition\-guide\/rust\-2024\/cargo\-resolver\.html](<https\:\/\/doc\.rust\-lang\.org\/edition\-guide\/rust\-2024\/cargo\-resolver\.html>)
- wasm32\-wasip2 target\: [https\:\/\/doc\.rust\-lang\.org\/rustc\/platform\-support\/wasm32\-wasip2\.html](<https\:\/\/doc\.rust\-lang\.org\/rustc\/platform\-support\/wasm32\-wasip2\.html>)
- Langium grammar\: [https\:\/\/langium\.org\/docs\/reference\/grammar\-language\/](<https\:\/\/langium\.org\/docs\/reference\/grammar\-language\/>)
- Langium features\: [https\:\/\/langium\.org\/docs\/features\/](<https\:\/\/langium\.org\/docs\/features\/>)
- <ruby>開発者<rt>かいはつしゃ</rt></ruby>による<ruby>事例<rt>じれい</rt></ruby>\: [https\:\/\/www\.typefox\.io\/blog\/langium\-1\.0\-a\-mature\-language\-toolkit\/](<https\:\/\/www\.typefox\.io\/blog\/langium\-1\.0\-a\-mature\-language\-toolkit\/>)
- Racket syntax\-spec\: [https\:\/\/docs\.racket\-lang\.org\/syntax\-spec\-v3\/Specifying\_languages\.html](<https\:\/\/docs\.racket\-lang\.org\/syntax\-spec\-v3\/Specifying\_languages\.html>)
- LSP 3\.17の<ruby>参照契約<rt>さんしょうけいやく</rt></ruby>\: [https\:\/\/microsoft\.github\.io\/language\-server\-protocol\/specifications\/lsp\/3\.17\/specification\/](<https\:\/\/microsoft\.github\.io\/language\-server\-protocol\/specifications\/lsp\/3\.17\/specification\/>)
- MathML Core\: [https\:\/\/www\.w3\.org\/TR\/mathml\-core\/](<https\:\/\/www\.w3\.org\/TR\/mathml\-core\/>)
- OpenType MATH（<ruby>独自<rt>どくじ</rt></ruby>layout backendを<ruby>追加<rt>ついか</rt></ruby>する<ruby>際<rt>さい</rt></ruby>の<ruby>境界<rt>きょうかい</rt></ruby>の<ruby>参考<rt>さんこう</rt></ruby>）\: [https\:\/\/learn\.microsoft\.com\/en\-us\/typography\/opentype\/doc\/spec\/math](<https\:\/\/learn\.microsoft\.com\/en\-us\/typography\/opentype\/doc\/spec\/math>)
- HTML ruby\: [https\:\/\/html\.spec\.whatwg\.org\/multipage\/text\-level\-semantics\.html](<https\:\/\/html\.spec\.whatwg\.org\/multipage\/text\-level\-semantics\.html>)
- CBOR\: [https\:\/\/www\.rfc\-editor\.org\/rfc\/rfc8949\.html](<https\:\/\/www\.rfc\-editor\.org\/rfc\/rfc8949\.html>)
- WITの<ruby>言語中立<rt>げんごちゅうりつ</rt></ruby>interface（<ruby>本仕様<rt>ほんしよう</rt></ruby>のNDFと<ruby>同一<rt>どういつ</rt></ruby>のABIではない）\: [https\:\/\/component\-model\.bytecodealliance\.org\/design\/wit\.html](<https\:\/\/component\-model\.bytecodealliance\.org\/design\/wit\.html>)
- num\-bigint no\_std\: [https\:\/\/docs\.rs\/num\-bigint](<https\:\/\/docs\.rs\/num\-bigint>)
- num\-rational\: [https\:\/\/crates\.io\/crates\/num\-rational](<https\:\/\/crates\.io\/crates\/num\-rational>)
- Language tags RFC 5646\: [https\:\/\/www\.rfc\-editor\.org\/rfc\/rfc5646\.html](<https\:\/\/www\.rfc\-editor\.org\/rfc\/rfc5646\.html>)
- Unicode 16\.0\.0\: [https\:\/\/www\.unicode\.org\/versions\/Unicode16\.0\.0\/](<https\:\/\/www\.unicode\.org\/versions\/Unicode16\.0\.0\/>)

<ruby>設計<rt>せっけい</rt></ruby>のGrammar\/Doc\/Math\/Circuitの<ruby>具体的<rt>ぐたいてき</rt></ruby>な<ruby>表層文法<rt>ひょうそうぶんぽう</rt></ruby>、NDF profile、crate<ruby>構成<rt>こうせい</rt></ruby>は、この<ruby>成果物<rt>せいかぶつ</rt></ruby>で<ruby>定<rt>さだ</rt></ruby>めた<ruby>設計<rt>せっけい</rt></ruby>であり、これらの<ruby>外部資料<rt>がいぶしりょう</rt></ruby>が<ruby>採用<rt>さいよう</rt></ruby>を<ruby>保証<rt>ほしょう</rt></ruby>するものではない。
