<!-- Generated from doc/decisions/0001&#45;repository&#45;foundation.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page repository&#45;foundation&#45;decision; source SHA-256 626bff12deac9e6b6eeccd980ceb79fd34a4414ad2be9634ffd8539886c89ef4; alias input SHA-256 65de32d5a17ecfe75f9f79b8316c1a09c7487ea4aebdaa2285966109126f1d11; page input SHA-256 0bccfe775f2ba0c752d745c62b0233358ab5012b7b1e106fba13147b073cdbc7. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="0001-設計の取り込みとリポジトリ基盤"></a>

# 0001\: <ruby>設計<rt>せっけい</rt></ruby>の<ruby>取<rt>と</rt></ruby>り<ruby>込<rt>こ</rt></ruby>みとリポジトリ<ruby>基盤<rt>きばん</rt></ruby>

[正本（NEPL3d）](<0001-repository-foundation.nepld>)

- <ruby>日付<rt>ひづけ</rt></ruby>\: 2026\-09\-06
- <ruby>状態<rt>じょうたい</rt></ruby>\: <ruby>採用<rt>さいよう</rt></ruby>
- <ruby>元設計<rt>もとせっけい</rt></ruby>\: `nepl3-design-2026-09-06-r1`

<a name="n-70726f626c656d"></a>

<a name="問題"></a>

## <ruby>問題<rt>もんだい</rt></ruby>

<ruby>元<rt>もと</rt></ruby>の<ruby>資料<rt>しりょう</rt></ruby>は<ruby>設計<rt>せっけい</rt></ruby>・<ruby>実装依頼<rt>じっそういらい</rt></ruby>・<ruby>設計時<rt>せっけいじ</rt></ruby>の<ruby>検査報告<rt>けんさほうこく</rt></ruby>をまとめたものです。<ruby>設計<rt>せっけい</rt></ruby>の<ruby>完成<rt>かんせい</rt></ruby>、Rust<ruby>実装<rt>じっそう</rt></ruby>の<ruby>完成<rt>かんせい</rt></ruby>、<ruby>現在実行<rt>げんざいじっこう</rt></ruby>した<ruby>検査<rt>けんさ</rt></ruby>が<ruby>混同<rt>こんどう</rt></ruby>されると、<ruby>後続<rt>こうぞく</rt></ruby>の<ruby>開発<rt>かいはつ</rt></ruby>で<ruby>未実装<rt>みじっそう</rt></ruby>の<ruby>機能<rt>きのう</rt></ruby>や<ruby>未実行<rt>みじっこう</rt></ruby>の<ruby>受入試験<rt>うけいれしけん</rt></ruby>を<ruby>成功扱<rt>せいこうあつか</rt></ruby>いする<ruby>危険<rt>きけん</rt></ruby>があります。また、ユーザーが<ruby>指定<rt>してい</rt></ruby>した<ruby>正式<rt>せいしき</rt></ruby>な<ruby>文書配置<rt>ぶんしょはいち</rt></ruby>は `doc/` です。

<a name="n-6465636973696f6e"></a>

<a name="判断"></a>

## <ruby>判断<rt>はんだん</rt></ruby>

1. <ruby>文章仕様<rt>ぶんしょうしよう</rt></ruby>を `doc/spec/` へ<ruby>移<rt>うつ</rt></ruby>し、<ruby>正式<rt>せいしき</rt></ruby>な<ruby>入口<rt>いりぐち</rt></ruby>を `doc/README.md` とします。<ruby>機械可読<rt>きかいかどく</rt></ruby>の `design/`、`interfaces/`、<ruby>文法<rt>ぶんぽう</rt></ruby>source、<ruby>例<rt>れい</rt></ruby>、<ruby>受入入力<rt>うけいれにゅうりょく</rt></ruby>は<ruby>責務<rt>せきむ</rt></ruby>ごとに<ruby>維持<rt>いじ</rt></ruby>します。ローカルの `.tmp/` は<ruby>除外<rt>じょがい</rt></ruby>します。
1. <ruby>元<rt>もと</rt></ruby>manifestと<ruby>設計検査報告<rt>せっけいけんさほうこく</rt></ruby>は `doc/history/` に<ruby>元<rt>もと</rt></ruby>byte<ruby>列<rt>れつ</rt></ruby>で<ruby>保存<rt>ほぞん</rt></ruby>します。<ruby>移設後<rt>いせつご</rt></ruby>の<ruby>検査結果<rt>けんさけっか</rt></ruby>へ<ruby>読<rt>よ</rt></ruby>み<ruby>替<rt>か</rt></ruby>えません。
1. `design/tasks.json` はタスク<ruby>定義<rt>ていぎ</rt></ruby>に<ruby>専念<rt>せんねん</rt></ruby>し、<ruby>実行状態<rt>じっこうじょうたい</rt></ruby>を `implementation-status.json` へ<ruby>分離<rt>ぶんり</rt></ruby>します。タスク<ruby>本文<rt>ほんぶん</rt></ruby>と<ruby>索引<rt>さくいん</rt></ruby>は<ruby>開発<rt>かいはつ</rt></ruby>toolsで<ruby>生成<rt>せいせい</rt></ruby>・<ruby>差分検査<rt>さぶんけんさ</rt></ruby>します。
1. 18 crateの<ruby>依存表<rt>いぞんひょう</rt></ruby>は<ruby>目標構成<rt>もくひょうこうせい</rt></ruby>とし、Cargo workspaceには<ruby>実装済<rt>じっそうず</rt></ruby>みのmemberだけを<ruby>登録<rt>とうろく</rt></ruby>します。<ruby>基盤整備段階<rt>きばんせいびだんかい</rt></ruby>では<ruby>開発<rt>かいはつ</rt></ruby>toolsを<ruby>実装<rt>じっそう</rt></ruby>し、<ruby>言語<rt>げんご</rt></ruby>crateの<ruby>空実装<rt>からじっそう</rt></ruby>や<ruby>成功<rt>せいこう</rt></ruby>stubを<ruby>作<rt>つく</rt></ruby>りません。
1. CIは<ruby>現存<rt>げんそん</rt></ruby>する<ruby>実装<rt>じっそう</rt></ruby>を3つのnative OSで<ruby>検査<rt>けんさ</rt></ruby>します。mainの<ruby>同<rt>おな</rt></ruby>じ<ruby>検査済<rt>けんさず</rt></ruby>みSHAからsource archiveを<ruby>配布<rt>はいふ</rt></ruby>します。WASI・ブラウザ・<ruby>別<rt>べつ</rt></ruby>process providerの<ruby>受入条件<rt>うけいれじょうけん</rt></ruby>は<ruby>実装<rt>じっそう</rt></ruby>とrunnerがそろうまで<ruby>未実行<rt>みじっこう</rt></ruby>です。
1. メインagentが<ruby>統括<rt>とうかつ</rt></ruby>し、<ruby>実装<rt>じっそう</rt></ruby>subagentと<ruby>独立<rt>どくりつ</rt></ruby>レビューsubagentを<ruby>分<rt>わ</rt></ruby>けます。<ruby>設計<rt>せっけい</rt></ruby>に<ruby>矛盾<rt>むじゅん</rt></ruby>が<ruby>見<rt>み</rt></ruby>つかった<ruby>場合<rt>ばあい</rt></ruby>は、<ruby>根拠<rt>こんきょ</rt></ruby>と<ruby>影響範囲<rt>えいきょうはんい</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>して<ruby>仕様<rt>しよう</rt></ruby>・schema・<ruby>例<rt>れい</rt></ruby>・<ruby>受入条件<rt>うけいれじょうけん</rt></ruby>を<ruby>同時<rt>どうじ</rt></ruby>に<ruby>訂正<rt>ていせい</rt></ruby>します。

<a name="n-65666665637473"></a>

<a name="影響と確認"></a>

## <ruby>影響<rt>えいきょう</rt></ruby>と<ruby>確認<rt>かくにん</rt></ruby>

<ruby>仕様<rt>しよう</rt></ruby>の<ruby>意味<rt>いみ</rt></ruby>を<ruby>縮小<rt>しゅくしょう</rt></ruby>せず、<ruby>現在<rt>げんざい</rt></ruby>の<ruby>実装<rt>じっそう</rt></ruby>と<ruby>将来<rt>しょうらい</rt></ruby>の<ruby>目標<rt>もくひょう</rt></ruby>を<ruby>区別<rt>くべつ</rt></ruby>します。metadataからのファイル<ruby>参照<rt>さんしょう</rt></ruby>、タスクID・<ruby>依存<rt>いぞん</rt></ruby>・<ruby>受入条件参照<rt>うけいれじょうけんさんしょう</rt></ruby>、workspace<ruby>依存<rt>いぞん</rt></ruby>とDAG、<ruby>生成<rt>せいせい</rt></ruby>タスクの<ruby>整合<rt>せいごう</rt></ruby>を<ruby>開発<rt>かいはつ</rt></ruby>toolsで<ruby>検査<rt>けんさ</rt></ruby>します。<ruby>一般<rt>いっぱん</rt></ruby>のMarkdownリンクは<ruby>別途<rt>べっと</rt></ruby>レビューします。<ruby>取<rt>と</rt></ruby>り<ruby>込<rt>こ</rt></ruby>み<ruby>元<rt>もと</rt></ruby>で<ruby>報告<rt>ほうこく</rt></ruby>された43<ruby>項目<rt>こうもく</rt></ruby>の<ruby>検査<rt>けんさ</rt></ruby>と37<ruby>群<rt>ぐん</rt></ruby>のruntime<ruby>受入条件<rt>うけいれじょうけん</rt></ruby>は<ruby>別物<rt>べつもの</rt></ruby>として<ruby>保持<rt>ほじ</rt></ruby>します。

この<ruby>判断<rt>はんだん</rt></ruby>は<ruby>元設計全体<rt>もとせっけいぜんたい</rt></ruby>の<ruby>正<rt>ただ</rt></ruby>しさを<ruby>承認<rt>しょうにん</rt></ruby>するものではありません。<ruby>個々<rt>ここ</rt></ruby>の<ruby>実装<rt>じっそう</rt></ruby>に<ruby>入<rt>はい</rt></ruby>る<ruby>際<rt>さい</rt></ruby>にも、<ruby>関連<rt>かんれん</rt></ruby>する<ruby>契約<rt>けいやく</rt></ruby>・<ruby>文法<rt>ぶんぽう</rt></ruby>・<ruby>不変条件<rt>ふへんじょうけん</rt></ruby>を<ruby>独立<rt>どくりつ</rt></ruby>レビューし、<ruby>矛盾<rt>むじゅん</rt></ruby>や<ruby>不足<rt>ふそく</rt></ruby>を<ruby>記録<rt>きろく</rt></ruby>します。
