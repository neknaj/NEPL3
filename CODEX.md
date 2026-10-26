<!-- Generated from doc/migration/authored/project/CODEX.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page codex; source SHA-256 0af810b59cd8492b69b1a9463d41d86740cd71862e49d0b8d62667787ca272b0; alias input SHA-256 bec0c5cfc9c12d328291487f63678764c420ce35d2ba417cc49eb215b73f060a; page input SHA-256 4d29a27e7a72fd34aced2b616f2f9aecaaefdb0a26494d5637e29baa5ff90f2e. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="実装作業の入口"></a>

# <ruby>実装作業<rt>じっそうさぎょう</rt></ruby>の<ruby>入口<rt>いりぐち</rt></ruby>

[正本（NEPL3d）](<doc/migration/authored/project/CODEX.nepld>)

このファイルは<ruby>案内<rt>あんない</rt></ruby>です。<ruby>独立<rt>どくりつ</rt></ruby>した<ruby>開発規約<rt>かいはつきやく</rt></ruby>は<ruby>置<rt>お</rt></ruby>かず、[AGENTS\.md](<AGENTS\.md>)を<ruby>規約<rt>きやく</rt></ruby>の<ruby>入口<rt>いりぐち</rt></ruby>とします。

- <ruby>仕様<rt>しよう</rt></ruby>・<ruby>執筆<rt>しっぴつ</rt></ruby>・<ruby>実装資料<rt>じっそうしりょう</rt></ruby>は[<ruby>文書索引<rt>ぶんしょさくいん</rt></ruby>](<doc\/README\.md>)、<ruby>検査<rt>けんさ</rt></ruby>とPR<ruby>統合手順<rt>とうごうてじゅん</rt></ruby>は[<ruby>開発手順<rt>かいはつてじゅん</rt></ruby>](<doc\/development\.md>)を<ruby>参照<rt>さんしょう</rt></ruby>します。
- <ruby>着手範囲<rt>ちゃくしゅはんい</rt></ruby>は<ruby>今回<rt>こんかい</rt></ruby>のユーザー<ruby>指示<rt>しじ</rt></ruby>、[現在の実装状態](<implementation\-status\.json>)、[タスク定義と依存関係](<design\/tasks\.json>)を<ruby>照合<rt>しょうごう</rt></ruby>し、<ruby>利用<rt>りよう</rt></ruby>する<ruby>前段成果物<rt>ぜんだんせいかぶつ</rt></ruby>が<ruby>成立<rt>せいりつ</rt></ruby>した<ruby>範囲<rt>はんい</rt></ruby>から<ruby>選<rt>えら</rt></ruby>びます。
- <ruby>文書<rt>ぶんしょ</rt></ruby>の<ruby>編集対象<rt>へんしゅうたいしょう</rt></ruby>は[正本登録](<doc\/canonical\.json>)で<ruby>確認<rt>かくにん</rt></ruby>します。<ruby>登録済<rt>とうろくず</rt></ruby>みページは<ruby>登録<rt>とうろく</rt></ruby>されたNEPL3d sourceを<ruby>編集<rt>へんしゅう</rt></ruby>し、Markdown projectionを<ruby>直接編集<rt>ちょくせつへんしゅう</rt></ruby>しません。<ruby>未移行<rt>みいこう</rt></ruby>ページはMarkdown<ruby>正本<rt>せいほん</rt></ruby>を<ruby>編集<rt>へんしゅう</rt></ruby>します。ページ<ruby>単位<rt>たんい</rt></ruby>の<ruby>正本切替条件<rt>せいほんきりかえじょうけん</rt></ruby>はAGENTS\.mdに<ruby>従<rt>したが</rt></ruby>います。
- <ruby>完成判定<rt>かんせいはんてい</rt></ruby>は[登録された受入条件](<design\/acceptance\.json>)と<ruby>各<rt>かく</rt></ruby>タスクの<ruby>条件<rt>じょうけん</rt></ruby>に<ruby>従<rt>したが</rt></ruby>います。<ruby>個別試験<rt>こべつしけん</rt></ruby>・CI<ruby>成功<rt>せいこう</rt></ruby>を<ruby>処理系全体<rt>しょりけいぜんたい</rt></ruby>の<ruby>完成<rt>かんせい</rt></ruby>へ<ruby>読<rt>よ</rt></ruby>み<ruby>替<rt>か</rt></ruby>えません。
