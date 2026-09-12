# Doc移行の休止・再開地点

ユーザーの2026年9月9日の休止指示により、以下の区切りで停止する。
処理系全体、T21/T16、Pages公開の完成を意味しない。

## 第08章までの完了分

- PR97（第12章）はmainの `ddd5bee10abac05702ea09490d421cc0e1188672` に取り込み済み。
- [PR98](https://github.com/neknaj/NEPL3/pull/98) は第08章の正本切替。
  最終headは `9985a3906ed455c6466ee5fd6daac4f13eca0ce1`。
  production生成、独立レビュー、browser、compiler不要の復元、archive監査まで完了。
  追加の休止整理で同headの必須CI16件すべての成功を確認し、
  mainの `f2cbf4fb95ec1d52c76df299d64bb376b98cb06e` へmerge済み。
- 第08章を含む正本9章はmain取り込み済み。
  PR98の証拠は `conformance/results/doc-canonical-editor/`。

## 第20章の途中状態

作業branchは `feat/doc-canonical-doc-html`、worktreeは
`C:/projects/NEPL3-doc-canonical-doc-html`。
準備commitは `10255b819296ba3c7eb786784779a67ac15740d7`。
その後の `426bad439028f5dc99054acf0da92e304a5200d7` はPR98のarchiveを統合したもの。

第20章の稿を `doc/spec/20-doc-html.nepld` へ移し、旧URLのalias3件と
10章目のregistry登録を準備した。冒頭は、pure local fragmentの制約と、
既に実装された開発hostのshell/CSS書き出しを区別する3文へ訂正した。
元稿全体はKepler、rootによる冒頭変更は元稿著者と区別してCarsonがレビューした。
16段落・InlineCode13値・External link4件を保持する。

**このbranchは未完了であり、mainへmergeしてはならない。**
第20章のMarkdownはまだ旧正本のまま。registry更新後の第01/12章の
context headerも未再生成である。canonical --checkの成功を主張しない。

専用worktreeでbuildしたproduction toolによる正しい10章生成は、
MarkdownがAllocationLimit、HTMLがWorkLimitで停止し、どちらも出力未作成。
HTMLはSource242,864 / Work299,999,987 / Nodes12,051,403 /
Allocation639,833,274 / Output352 / Depth14、resolve/render段階の停止。
この段階で上限は変更していない。

休止checkpoint `0e6401097c26d1797d70b106b1d5c03d9637c0d7` の
CI run `34287360823` も確認した。native3OSの既存canonical corpus testが
AllocationLimitで失敗し、qualityも失敗している。検査を解除せず、
第20章branchは未完了のまま保全する。この失敗をPR98/mainのCIと混同しない。

先行する別worktreeのbinary流用は検証helperの誤りだった。
toolsのrootはcwdでなくcompile-time CARGO_MANIFEST_DIRに基づくため、
その2回の成功出力は第08章までの9章を読み、第20章の検証ではない。
誤った帰属と実際のmanifestを保存し、成功証拠から除外する。

## 再開する順序

1. AGENTS/CODEX/authoringと現HEAD・全working treeを再確認する。
2. PR98のmerge後のmainを取得し、この途中branchへ取り込む。未完了の第20章を
   先にmainへmergeしない。再開時にGitHubの最新状態も確認する。
3. 第20章の正しい停止記録を基に、有限の集合出力予算を検討する。
   stopped Budgetのreset、core/parse/lower既定値の無根拠な変更はしない。
4. 同じworktreeでbuildしたtoolにより、新規出力へMarkdown/HTMLを2回生成し、
   10章すべてのsource/manifest・byte一致・以前の出力保持を確認する。
5. 第20章Markdownと第01/12章のcontext headerを採用し、production検査、
   独立code/表示レビュー、旧3＋semantic2の実GitHub移動、10文書・CSS・manifestの
   計12ファイルからなるHTML集合のcompiler不要復元、必須CIを経て正本切替を完了する。

休止証拠は `conformance/results/doc-canonical-doc-html-checkpoint/`。
これは途中状態を保全する記録であり、受入passedの証拠ではない。
`.tmp/` 内のbuild/browser環境と作業資料は再開用に保持する。
以前から保全している別worktreeの未保存変更は今回の休止整理では変更しない。
