<!-- Generated from conformance/history.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page conformance&#45;history; source SHA-256 2b915850ee4e9c6028864bbb34c60307861bfe62dee9d9dc85f659cbd3697d35; alias input SHA-256 fca5b68c9ebc6b8d899248b6aa5fd058e08622d6212776d8bc831b9bc43edd30; page input SHA-256 852e8ed0a203c16de03be3471e601e093c08929d9228d42468e8407d549b75a5. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="検証記録の履歴"></a>

# <ruby>検証記録<rt>けんしょうきろく</rt></ruby>の<ruby>履歴<rt>りれき</rt></ruby>

[正本（NEPL3d）](<history.nepld>)

<ruby>過去<rt>かこ</rt></ruby>のsource、レビューscript、workspace、<ruby>生成物<rt>せいせいぶつ</rt></ruby>、<ruby>実行<rt>じっこう</rt></ruby>logはGit revisionから<ruby>取得<rt>しゅとく</rt></ruby>する。<ruby>現在<rt>げんざい</rt></ruby>の<ruby>段階記録<rt>だんかいきろく</rt></ruby>への<ruby>参照<rt>さんしょう</rt></ruby>は `implementation-status.json` と `conformance/results/stages/` が<ruby>所有<rt>しょゆう</rt></ruby>する。<ruby>各参照<rt>かくさんしょう</rt></ruby>は<ruby>元記録<rt>もときろく</rt></ruby>のrevision・path・SHA\-256を<ruby>保持<rt>ほじ</rt></ruby>し、<ruby>記録時点<rt>きろくじてん</rt></ruby>の<ruby>検証範囲<rt>けんしょうはんい</rt></ruby>を<ruby>示<rt>しめ</rt></ruby>す。

| revision | <ruby>対象<rt>たいしょう</rt></ruby>path | <ruby>確認<rt>かくにん</rt></ruby>できる<ruby>記録<rt>きろく</rt></ruby> |
| --- | --- | --- |
| `44bb19235cbf8039af2fceca59c0c9d7c261db00` | `conformance/results/acceptance/e01.json` | E01の7<ruby>環境<rt>かんきょう</rt></ruby>・<ruby>各<rt>かく</rt></ruby>228<ruby>観測<rt>かんそく</rt></ruby>と<ruby>認証付<rt>にんしょうつ</rt></ruby>きlog<ruby>復元<rt>ふくげん</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>した<ruby>固定入力<rt>こていにゅうりょく</rt></ruby>の<ruby>正式記録<rt>せいしききろく</rt></ruby>。OriginGraphのsource<ruby>変更後<rt>へんこうご</rt></ruby>は<ruby>現在入力<rt>げんざいにゅうりょく</rt></ruby>の<ruby>証拠<rt>しょうこ</rt></ruby>として<ruby>再利用<rt>さいりよう</rt></ruby>せず、<ruby>元記録<rt>もときろく</rt></ruby>はこのGit revisionから<ruby>取得<rt>しゅとく</rt></ruby>する。 |
| `d87ca8c4ba18e74bc71efb0e2958af87ac971e1b` | `conformance/results/` | 2026<ruby>年<rt>ねん</rt></ruby>9<ruby>月<rt>がつ</rt></ruby>13<ruby>日<rt>にち</rt></ruby>までの<ruby>検証<rt>けんしょう</rt></ruby>archive。mainの<ruby>祖先<rt>そせん</rt></ruby>として<ruby>到達可能<rt>とうたつかのう</rt></ruby>。 |
| `fd8057199f2f32fb36455210df36e78015b77fab` | `conformance/results/` | <ruby>今回<rt>こんかい</rt></ruby>の<ruby>整理前<rt>せいりまえ</rt></ruby>の2\,046ファイル。<ruby>後続<rt>こうぞく</rt></ruby>の<ruby>文書移行<rt>ぶんしょいこう</rt></ruby>・<ruby>外部<rt>がいぶ</rt></ruby>consumer・runtime<ruby>段階記録<rt>だんかいきろく</rt></ruby>を<ruby>含<rt>ふく</rt></ruby>む。 |
| `fd8057199f2f32fb36455210df36e78015b77fab` | `conformance/results/reader-builtins/allocation-baseline.log`、`allocation-working.log` | R020の<ruby>同一<rt>どういつ</rt></ruby>allocation probeによる<ruby>修正前後<rt>しゅうせいぜんご</rt></ruby>の<ruby>観測<rt>かんそく</rt></ruby>。 |
| `fd8057199f2f32fb36455210df36e78015b77fab` | `conformance/results/markup-html/independent.json` | R049〜R051の<ruby>独立<rt>どくりつ</rt></ruby>レビュー<ruby>記録<rt>きろく</rt></ruby>。 |
| `011d4162126a059da39553c9cdee53b62967c2f0` | runtime source | T11\/T12の<ruby>段階試験対象<rt>だんかいしけんたいしょう</rt></ruby>。mainから<ruby>到達不能<rt>とうたつふのう</rt></ruby>な4 commitを<ruby>持<rt>も</rt></ruby>つ `archive/runtime-ownership-2026-09-23` で<ruby>保全<rt>ほぜん</rt></ruby>する。 |

<ruby>例<rt>れい</rt></ruby>：

```sh
git show fd8057199f2f32fb36455210df36e78015b77fab:conformance/results/reader-builtins/allocation-baseline.log
git worktree add --detach ../NEPL3-history fd8057199f2f32fb36455210df36e78015b77fab
```

<ruby>現行試験<rt>げんこうしけん</rt></ruby>が<ruby>使用<rt>しよう</rt></ruby>する<ruby>最小<rt>さいしょう</rt></ruby>fixtureは、その<ruby>試験<rt>しけん</rt></ruby>のdirectoryで<ruby>管理<rt>かんり</rt></ruby>する。`tools/site/fixtures/provenance.json` はtar fixtureの<ruby>元<rt>もと</rt></ruby>revision・path・digestと<ruby>保持理由<rt>ほじりゆう</rt></ruby>を<ruby>記録<rt>きろく</rt></ruby>する。<ruby>過去<rt>かこ</rt></ruby>の<ruby>実行結果<rt>じっこうけっか</rt></ruby>は<ruby>対象<rt>たいしょう</rt></ruby>revisionとscopeに<ruby>属<rt>ぞく</rt></ruby>する。<ruby>現在<rt>げんざい</rt></ruby>のHEADと<ruby>正式受入<rt>せいしきうけいれ</rt></ruby>の<ruby>状態<rt>じょうたい</rt></ruby>は、<ruby>現在<rt>げんざい</rt></ruby>の<ruby>試験<rt>しけん</rt></ruby>と<ruby>状態索引<rt>じょうたいさくいん</rt></ruby>で<ruby>確認<rt>かくにん</rt></ruby>する。
