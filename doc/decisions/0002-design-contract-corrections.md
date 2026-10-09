<!-- Generated from doc/decisions/0002&#45;design&#45;contract&#45;corrections.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page design&#45;contract&#45;corrections&#45;decision; source SHA-256 b397f6dd4867ef5c91b2686a20b2408d4788943739f3a54ed4cc479d45a3616f; alias input SHA-256 a80472d9fe59d0a8111152d4ae2bf4551094ec519930b98c3158b407c2c64918; page input SHA-256 47d6246c76f5f9f4d7ee64969b286e564062a96a480a4941aaafbc7ada9d40a3. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="0002-独立レビューによる契約の訂正"></a>

# 0002\: <ruby>独立<rt>どくりつ</rt></ruby>レビューによる<ruby>契約<rt>けいやく</rt></ruby>の<ruby>訂正<rt>ていせい</rt></ruby>

[正本（NEPL3d）](<0002-design-contract-corrections.nepld>)

- <ruby>日付<rt>ひづけ</rt></ruby>\: 2026\-09\-06
- <ruby>状態<rt>じょうたい</rt></ruby>\: <ruby>採用<rt>さいよう</rt></ruby>
- <ruby>取<rt>と</rt></ruby>り<ruby>込<rt>こ</rt></ruby>み<ruby>元<rt>もと</rt></ruby>\: `nepl3-design-2026-09-06-r1`
- <ruby>現行設計<rt>げんこうせっけい</rt></ruby>\: `nepl3-design-2026-09-06-r2`

[<ruby>独立<rt>どくりつ</rt></ruby>レビュー](<\.\.\/review\.md>) で、<ruby>交換形式<rt>こうかんけいしき</rt></ruby>の<ruby>順序<rt>じゅんじょ</rt></ruby>、Circuitの<ruby>操作前提<rt>そうさぜんてい</rt></ruby>、Codeの<ruby>意味値保持<rt>いみちほじ</rt></ruby>、Numberのprint roundtrip、MathMLの<ruby>長<rt>なが</rt></ruby>さ、<ruby>停止理由<rt>ていしりゆう</rt></ruby>、XML<ruby>文字列出力<rt>もじれつしゅつりょく</rt></ruby>の<ruby>矛盾<rt>むじゅん</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>しました。

| <ruby>指摘<rt>してき</rt></ruby> | <ruby>訂正<rt>ていせい</rt></ruby> |
| --- | --- |
| R001 | recordとvariant payloadのfieldを<ruby>順序付<rt>じゅんじょつ</rt></ruby>きarrayに<ruby>変更<rt>へんこう</rt></ruby>。variantはNDFの<ruby>名前<rt>なまえ</rt></ruby>で<ruby>識別<rt>しきべつ</rt></ruby>し、map<ruby>順<rt>じゅん</rt></ruby>に<ruby>意味<rt>いみ</rt></ruby>を<ruby>持<rt>も</rt></ruby>たせない。 |
| R002 | initialの<ruby>入力<rt>にゅうりょく</rt></ruby>をPreparedNetlistに<ruby>統一<rt>とういつ</rt></ruby>。NORは4<ruby>種<rt>しゅ</rt></ruby>のnodeと2<ruby>種<rt>しゅ</rt></ruby>のsink<ruby>配列<rt>はいれつ</rt></ruby>で<ruby>表<rt>あらわ</rt></ruby>す。 |
| R003 | Doc<ruby>自身<rt>じしん</rt></ruby>のCodeもForeignSyntaxを<ruby>保持<rt>ほじ</rt></ruby>。<ruby>表示<rt>ひょうじ</rt></ruby>のためにguestのlowerや<ruby>意味検査<rt>いみけんさ</rt></ruby>を<ruby>要求<rt>ようきゅう</rt></ruby>しない。 |
| R004 | Numberは<ruby>有限十進<rt>ゆうげんじっしん</rt></ruby>の<ruby>有理数<rt>ゆうりすう</rt></ruby>に<ruby>限定<rt>げんてい</rt></ruby>。<ruby>任意有理数<rt>にんいゆうりすう</rt></ruby>からの<ruby>式構築<rt>しきこうちく</rt></ruby>helperが<ruby>必要<rt>ひつよう</rt></ruby>に<ruby>応<rt>おう</rt></ruby>じFracを<ruby>作<rt>つく</rt></ruby>り、<ruby>著者<rt>ちょしゃ</rt></ruby>のFracは<ruby>保存<rt>ほぞん</rt></ruby>する。 |
| R005 | mspaceの<ruby>長<rt>なが</rt></ruby>さを<ruby>非負<rt>ひふ</rt></ruby>のcanonical<ruby>十進<rt>じっしん</rt></ruby> \+ emに<ruby>限定<rt>げんてい</rt></ruby>。SVG<ruby>座標<rt>ざひょう</rt></ruby>のDecimalと<ruby>区別<rt>くべつ</rt></ruby>する。 |
| R007 | sourceBytes<ruby>超過<rt>ちょうか</rt></ruby>の<ruby>型付<rt>かたつ</rt></ruby>き<ruby>停止理由<rt>ていしりゆう</rt></ruby>SourceLimitを<ruby>追加<rt>ついか</rt></ruby>する。 |
| R008 | Markupに<ruby>出力可能<rt>しゅつりょくかのう</rt></ruby>な<ruby>文字集合<rt>もじしゅうごう</rt></ruby>を<ruby>設<rt>もう</rt></ruby>け、XMLで<ruby>表<rt>あらわ</rt></ruby>せない<ruby>文字<rt>もじ</rt></ruby>を<ruby>拒否<rt>きょひ</rt></ruby>。\>とCR、<ruby>属性内<rt>ぞくせいない</rt></ruby>のTAB\/LF\/CRのescapeを<ruby>固定<rt>こてい</rt></ruby>する。 |
| R011 | task\.acceptanceをcoverage<ruby>参照<rt>さんしょう</rt></ruby>と<ruby>定義<rt>ていぎ</rt></ruby>。<ruby>前段<rt>ぜんだん</rt></ruby>タスクは<ruby>自身<rt>じしん</rt></ruby>のscope<ruby>付<rt>つ</rt></ruby>き<ruby>証拠<rt>しょうこ</rt></ruby>で<ruby>判定<rt>はんてい</rt></ruby>し、<ruby>群全体<rt>ぐんぜんたい</rt></ruby>のpassedとT16の<ruby>全<rt>ぜん</rt></ruby>37<ruby>群合格<rt>ぐんごうかく</rt></ruby>を<ruby>分離<rt>ぶんり</rt></ruby>する。 |

<ruby>対応<rt>たいおう</rt></ruby>する<ruby>文章仕様<rt>ぶんしょうしよう</rt></ruby>・<ruby>意味<rt>いみ</rt></ruby>モデル・<ruby>属性<rt>ぞくせい</rt></ruby>schema・conformance<ruby>入力<rt>にゅうりょく</rt></ruby>・<ruby>受入条件<rt>うけいれじょうけん</rt></ruby>を<ruby>合<rt>あ</rt></ruby>わせて<ruby>修正<rt>しゅうせい</rt></ruby>しました。source<ruby>文法<rt>ぶんぽう</rt></ruby>はNumberの<ruby>有限十進<rt>ゆうげんじっしん</rt></ruby>とForeignSyntaxの<ruby>構文<rt>こうぶん</rt></ruby>をすでに<ruby>表現<rt>ひょうげん</rt></ruby>しており、formのarity<ruby>変更<rt>へんこう</rt></ruby>はありません。source r1の<ruby>履歴<rt>りれき</rt></ruby>は<ruby>書<rt>か</rt></ruby>き<ruby>換<rt>か</rt></ruby>えません。<ruby>現行<rt>げんこう</rt></ruby>metadataのdesign\_revisionと<ruby>実装<rt>じっそう</rt></ruby>タスクのdesignはr2を<ruby>示<rt>しめ</rt></ruby>します。

R006（<ruby>操作<rt>そうさ</rt></ruby>・protocol schemaの<ruby>閉包不足<rt>へいほうぶそく</rt></ruby>）とR009（<ruby>解決済<rt>かいけつず</rt></ruby>みProfileの<ruby>生成<rt>せいせい</rt></ruby>）は、<ruby>関連<rt>かんれん</rt></ruby>タスクの<ruby>完成<rt>かんせい</rt></ruby>を<ruby>阻<rt>はば</rt></ruby>む<ruby>設計課題<rt>せっけいかだい</rt></ruby>として<ruby>記録<rt>きろく</rt></ruby>しています。<ruby>現時点<rt>げんじてん</rt></ruby>のinterfaceを<ruby>外部実装<rt>がいぶじっそう</rt></ruby>がそのまま<ruby>実行<rt>じっこう</rt></ruby>できる<ruby>完全<rt>かんぜん</rt></ruby>なschemaとして<ruby>扱<rt>あつか</rt></ruby>いません。<ruby>各課題<rt>かくかだい</rt></ruby>の<ruby>対象<rt>たいしょう</rt></ruby>タスクと<ruby>状態<rt>じょうたい</rt></ruby>は [design\/review\.json](<\.\.\/\.\.\/design\/review\.json>) を<ruby>参照<rt>さんしょう</rt></ruby>してください。

conformanceへ<ruby>追加<rt>ついか</rt></ruby>した<ruby>期待値<rt>きたいち</rt></ruby>は<ruby>仕様入力<rt>しようにゅうりょく</rt></ruby>で、runtimeによる<ruby>検証済<rt>けんしょうず</rt></ruby>み<ruby>結果<rt>けっか</rt></ruby>ではありません。<ruby>実装後<rt>じっそうご</rt></ruby>にfield<ruby>順<rt>じゅん</rt></ruby>\/digest、print roundtrip、Code、<ruby>回路<rt>かいろ</rt></ruby>、<ruby>停止<rt>ていし</rt></ruby>、Markupの<ruby>成功系<rt>せいこうけい</rt></ruby>・<ruby>失敗系<rt>しっぱいけい</rt></ruby>を<ruby>実行<rt>じっこう</rt></ruby>します。<ruby>今回<rt>こんかい</rt></ruby>の<ruby>訂正<rt>ていせい</rt></ruby>はレビュー<ruby>結果<rt>けっか</rt></ruby>と<ruby>公開規格<rt>こうかいきかく</rt></ruby>に<ruby>基<rt>もと</rt></ruby>づきます。<ruby>根拠<rt>こんきょ</rt></ruby>は [RFC 8259](<https\:\/\/www\.rfc\-editor\.org\/rfc\/rfc8259\.html\#section\-4>)、[MathML Core](<https\:\/\/www\.w3\.org\/TR\/mathml\-core\/\#space\-mspace>)、[XML 1\.0](<https\:\/\/www\.w3\.org\/TR\/xml\/\#charsets>) です。

<ruby>段階完了<rt>だんかいかんりょう</rt></ruby>の<ruby>訂正<rt>ていせい</rt></ruby>は、T01が<ruby>参照<rt>さんしょう</rt></ruby>するE03\/E04\/A04に<ruby>後続<rt>こうぞく</rt></ruby>エディタ<ruby>等<rt>とう</rt></ruby>の<ruby>責務<rt>せきむ</rt></ruby>も<ruby>含<rt>ふく</rt></ruby>まれることが<ruby>根拠<rt>こんきょ</rt></ruby>です。<ruby>参照群全体<rt>さんしょうぐんぜんたい</rt></ruby>の<ruby>合格<rt>ごうかく</rt></ruby>を<ruby>前段完了<rt>ぜんだんかんりょう</rt></ruby>の<ruby>条件<rt>じょうけん</rt></ruby>にすると<ruby>依存順<rt>いぞんじゅん</rt></ruby>で<ruby>進<rt>すす</rt></ruby>められません。<ruby>各<rt>かく</rt></ruby>タスクの<ruby>実装責務<rt>じっそうせきむ</rt></ruby>と<ruby>全体<rt>ぜんたい</rt></ruby>の<ruby>受入条件<rt>うけいれじょうけん</rt></ruby>を<ruby>分<rt>わ</rt></ruby>け、<ruby>前段<rt>ぜんだん</rt></ruby>をcompleteにしても<ruby>未実行<rt>みじっこう</rt></ruby>の<ruby>群<rt>ぐん</rt></ruby>はnot\-runを<ruby>維持<rt>いじ</rt></ruby>します。
