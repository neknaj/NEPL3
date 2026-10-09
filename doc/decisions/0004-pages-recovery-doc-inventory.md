<!-- Generated from doc/decisions/0004&#45;pages&#45;recovery&#45;doc&#45;inventory.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page pages&#45;recovery&#45;inventory&#45;decision; source SHA-256 197435331edd5013d54f6259bcb162b20eb477d5345b08b253816738da522153; alias input SHA-256 bb6e2fa765704c26aa2c2da6428dff56915901cd434431d299dd90d172109c70; page input SHA-256 ca710ef836d5d54f824999e09068695f2dcc444c75e4d5546b14149510a4f6cb. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="0004-pages復旧とdoc設計前のinventory"></a>

# 0004\: Pages<ruby>復旧<rt>ふっきゅう</rt></ruby>とDoc<ruby>設計前<rt>せっけいまえ</rt></ruby>のinventory

[正本（NEPL3d）](<0004-pages-recovery-doc-inventory.nepld>)

- <ruby>日付<rt>ひづけ</rt></ruby>\: 2026\-09\-06
- <ruby>状態<rt>じょうたい</rt></ruby>\: <ruby>採用<rt>さいよう</rt></ruby>
- <ruby>対象設計<rt>たいしょうせっけい</rt></ruby>\: `nepl3-design-2026-09-06-r3`
- <ruby>対象<rt>たいしょう</rt></ruby>レビュー\: commit `25a096b` の<ruby>公開後失敗処理<rt>こうかいごしっぱいしょり</rt></ruby>とDoc<ruby>移行順序<rt>いこうじゅんじょ</rt></ruby>

<a name="n-7265636f76657279"></a>

<a name="pages公開後の失敗"></a>

## Pages<ruby>公開後<rt>こうかいご</rt></ruby>の<ruby>失敗<rt>しっぱい</rt></ruby>

<ruby>従来仕様<rt>じゅうらいしよう</rt></ruby>はpublic smokeの<ruby>失敗<rt>しっぱい</rt></ruby>を<ruby>記録<rt>きろく</rt></ruby>するだけで、<ruby>切替済<rt>きりかえず</rt></ruby>みの<ruby>公開物<rt>こうかいぶつ</rt></ruby>を<ruby>復元<rt>ふくげん</rt></ruby>する<ruby>処理<rt>しょり</rt></ruby>がなかった。T20\/S06へLKGの<ruby>永続保存<rt>えいぞくほぞん</rt></ruby>、<ruby>公開<rt>こうかい</rt></ruby>writerの<ruby>全区間排他<rt>ぜんくかんはいた</rt></ruby>、candidate identityの<ruby>照合<rt>しょうごう</rt></ruby>、<ruby>有限<rt>ゆうげん</rt></ruby>の<ruby>旧<rt>きゅう</rt></ruby>payload<ruby>再<rt>さい</rt></ruby>deployと<ruby>再<rt>さい</rt></ruby>smokeを<ruby>追加<rt>ついか</rt></ruby>する。<ruby>復旧成功後<rt>ふっきゅうせいこうご</rt></ruby>も<ruby>元<rt>もと</rt></ruby>runはfailedを<ruby>維持<rt>いじ</rt></ruby>する。

<ruby>復旧<rt>ふっきゅう</rt></ruby>archiveはpublic smokeに<ruby>合格<rt>ごうかく</rt></ruby>した<ruby>元<rt>もと</rt></ruby>tarをimmutable recovery releaseへ<ruby>保存<rt>ほぞん</rt></ruby>し、<ruby>現行<rt>げんこう</rt></ruby>LKGと<ruby>直前世代<rt>ちょくぜんせだい</rt></ruby>を<ruby>期限<rt>きげん</rt></ruby>で<ruby>消<rt>け</rt></ruby>さない。<ruby>通常<rt>つうじょう</rt></ruby>のActions artifactは<ruby>短<rt>みじか</rt></ruby>いretentionやrun<ruby>削除<rt>さくじょ</rt></ruby>で<ruby>失<rt>うしな</rt></ruby>われるため、<ruby>復旧保存<rt>ふっきゅうほぞん</rt></ruby>の<ruby>正本<rt>せいほん</rt></ruby>にしない。payload<ruby>保存<rt>ほぞん</rt></ruby>→download<ruby>検証<rt>けんしょう</rt></ruby>→journal<ruby>昇格<rt>しょうかく</rt></ruby>の<ruby>順序<rt>じゅんじょ</rt></ruby>を<ruby>守<rt>まも</rt></ruby>る。<ruby>保存未確定<rt>ほぞんみかくてい</rt></ruby>、<ruby>復旧失敗<rt>ふっきゅうしっぱい</rt></ruby>、<ruby>初回<rt>しょかい</rt></ruby>LKG<ruby>不在<rt>ふざい</rt></ruby>、run<ruby>消失<rt>しょうしつ</rt></ruby>は<ruby>独立<rt>どくりつ</rt></ruby>した<ruby>状態<rt>じょうたい</rt></ruby>にし、<ruby>推測<rt>すいそく</rt></ruby>による<ruby>新<rt>あら</rt></ruby>たな<ruby>公開<rt>こうかい</rt></ruby>を<ruby>止<rt>と</rt></ruby>める。

Pages APIにはexpected\-currentによるatomic<ruby>切替<rt>きりかえ</rt></ruby>の<ruby>公開契約<rt>こうかいけいやく</rt></ruby>がない。<ruby>全<rt>ぜん</rt></ruby>writerを<ruby>同<rt>おな</rt></ruby>じconcurrency groupに<ruby>限定<rt>げんてい</rt></ruby>し、<ruby>保護<rt>ほご</rt></ruby>journalとAPI receiptと<ruby>公開<rt>こうかい</rt></ruby>identityを<ruby>合<rt>あ</rt></ruby>わせて<ruby>対象<rt>たいしょう</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する。<ruby>新<rt>あたら</rt></ruby>しい<ruby>健康<rt>けんこう</rt></ruby>な<ruby>公開<rt>こうかい</rt></ruby>や<ruby>対象不明<rt>たいしょうふめい</rt></ruby>の<ruby>公開<rt>こうかい</rt></ruby>を、<ruby>古<rt>ふる</rt></ruby>いcandidateの<ruby>失敗処理<rt>しっぱいしょり</rt></ruby>で<ruby>上書<rt>うわが</rt></ruby>きしない。<ruby>詳細<rt>しょうさい</rt></ruby>と<ruby>公式資料<rt>こうしきしりょう</rt></ruby>の<ruby>照合<rt>しょうごう</rt></ruby>は [15<ruby>章<rt>しょう</rt></ruby>](<\.\.\/spec\/15\-site\.md>) に<ruby>記録<rt>きろく</rt></ruby>する。

<a name="n-696e76656e746f7279"></a>

<a name="doc-schemaの設計前にinventoryを使う"></a>

## Doc schemaの<ruby>設計前<rt>せっけいまえ</rt></ruby>にinventoryを<ruby>使<rt>つか</rt></ruby>う

<ruby>現在<rt>げんざい</rt></ruby>のDoc schemaには<ruby>表<rt>ひょう</rt></ruby>\/list\/<ruby>一般<rt>いっぱん</rt></ruby>link\/<ruby>汎用<rt>はんよう</rt></ruby>code\/<ruby>図等<rt>ずとう</rt></ruby>の<ruby>不足<rt>ふそく</rt></ruby>がある。T21の<ruby>変換直前<rt>へんかんちょくぜん</rt></ruby>に<ruby>初<rt>はじ</rt></ruby>めて<ruby>本文<rt>ほんぶん</rt></ruby>を<ruby>調<rt>しら</rt></ruby>べると、T01\/T02\/T07で<ruby>確定<rt>かくてい</rt></ruby>した<ruby>公開型<rt>こうかいがた</rt></ruby>・wireを<ruby>再<rt>ふたた</rt></ruby>び<ruby>広<rt>ひろ</rt></ruby>く<ruby>変<rt>か</rt></ruby>える<ruby>必要<rt>ひつよう</rt></ruby>がある。そこで<ruby>現存文書<rt>げんそんぶんしょ</rt></ruby>のbaseline inventoryとgap reportを<ruby>早期<rt>そうき</rt></ruby>に<ruby>作成<rt>さくせい</rt></ruby>し、Docに<ruby>関係<rt>かんけい</rt></ruby>する<ruby>共通型<rt>きょうつうがた</rt></ruby>・wire・<ruby>意味<rt>いみ</rt></ruby>モデルの<ruby>設計前<rt>せっけいまえ</rt></ruby>に<ruby>消費<rt>しょうひ</rt></ruby>する。

<ruby>早期監査<rt>そうきかんさ</rt></ruby>の<ruby>完了<rt>かんりょう</rt></ruby>とR014<ruby>全体<rt>ぜんたい</rt></ruby>の<ruby>解消<rt>かいしょう</rt></ruby>を<ruby>分<rt>わ</rt></ruby>ける。T01\/T02\/T07は<ruby>監査<rt>かんさ</rt></ruby>から<ruby>必要<rt>ひつよう</rt></ruby>な<ruby>表現<rt>ひょうげん</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>して<ruby>設計<rt>せっけい</rt></ruby>するが、T21や<ruby>未実装<rt>みじっそう</rt></ruby>backendの<ruby>完成<rt>かんせい</rt></ruby>を<ruby>前提<rt>ぜんてい</rt></ruby>にしない。T21は<ruby>実装<rt>じっそう</rt></ruby>・wire・backend・conformanceが<ruby>整<rt>ととの</rt></ruby>った<ruby>後<rt>あと</rt></ruby>にページ<ruby>単位<rt>たんい</rt></ruby>の<ruby>移行<rt>いこう</rt></ruby>を<ruby>完成<rt>かんせい</rt></ruby>させる。この<ruby>順序<rt>じゅんじょ</rt></ruby>により<ruby>循環依存<rt>じゅんかんいぞん</rt></ruby>と<ruby>二重手書<rt>にじゅうてが</rt></ruby>き<ruby>保守<rt>ほしゅ</rt></ruby>を<ruby>避<rt>さ</rt></ruby>ける。

<ruby>今回<rt>こんかい</rt></ruby>の<ruby>変更<rt>へんこう</rt></ruby>はr3の<ruby>公開失敗条件<rt>こうかいしっぱいじょうけん</rt></ruby>と<ruby>移行順序<rt>いこうじゅんじょ</rt></ruby>の<ruby>具体化<rt>ぐたいか</rt></ruby>であり、<ruby>言語<rt>げんご</rt></ruby>のform・<ruby>意味<rt>いみ</rt></ruby>・NDF<ruby>表現<rt>ひょうげん</rt></ruby>をまだ<ruby>変更<rt>へんこう</rt></ruby>しないためdesign revisionはr3を<ruby>維持<rt>いじ</rt></ruby>する。source\/spec identityは<ruby>差分<rt>さぶん</rt></ruby>により<ruby>変<rt>か</rt></ruby>わり、<ruby>古<rt>ふる</rt></ruby>い<ruby>実行証拠<rt>じっこうしょうこ</rt></ruby>を<ruby>現在<rt>げんざい</rt></ruby>の<ruby>証拠<rt>しょうこ</rt></ruby>へ<ruby>転用<rt>てんよう</rt></ruby>しない。<ruby>新<rt>あたら</rt></ruby>しいDoc constructor<ruby>等<rt>とう</rt></ruby>を<ruby>採用<rt>さいよう</rt></ruby>する<ruby>段階<rt>だんかい</rt></ruby>では、<ruby>仕様識別<rt>しようしきべつ</rt></ruby>と<ruby>全影響箇所<rt>ぜんえいきょうかしょ</rt></ruby>を<ruby>改<rt>あらた</rt></ruby>めて<ruby>更新<rt>こうしん</rt></ruby>する。
