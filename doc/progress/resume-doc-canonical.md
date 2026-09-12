# Doc移行の休止・再開地点

## 2026年9月12日の再開（現在）

第17章を先に移行し、第11章から参照できる正本を用意する。
第17章を加えた11ページ集合の初回HTML生成は、serialize中に
`AllocationLimit`（使用量749,999,977、上限750,000,000）で停止した。
出力ディレクトリは作成されず、Markdownの初回生成だけが成功した。
これはHTML成功・正本切替の証拠ではない。
拡大した文書集合のHTML生成用allocation上限を1,000,000,000へ変更し、
別の生成操作として再検証する。Work400M・Nodes20M・その他のHTML資源、
Markdown予算、core・各ページの既定値は維持する。
この値は累積する論理allocationの許容量であり、実メモリ量や最小予算を示さない。
停止の記録は残し、再生成・意味同等性・リンク・表示の検証後にprojectionを採用する。

新しい選択でMarkdown12ファイル・HTML13ファイルを各2回生成し、全byte一致を確認した。
HTMLの成功使用量はWork362,735,172・Allocation762,304,126・Nodes13,880,291である。
第17章と第01/12章のcontext headerを採用し、既存10文書のHTMLとCSSはbyte一致した。
第17章の21 code値・2表の3行/8行・参照先は独立に原文と照合した。
生成Markdownの再検査・独立した最終レビュー・必須CIを継続する。
全体の移行・KaTeX実装・Pages公開が完了したという記録ではない。

ユーザーの休止終了・自律続行指示により開発を再開した。
PR99は必須CI16件の成功後、mainの `994526e04c5d711aca06d6eefe6b11f3f175bcba`
へmerge済み。以下の休止記録は当時の状態を保存したものである。
次は第11章等の移行を妨げるMarkdown projectionの隣接List境界を実装し、
Relative linkの解決と合わせて文書移行を進める。未保存の別worktreeは保全する。

## 2026年9月12日の休止準備（履歴）

ユーザー指示により、第20章の正本切替を区切りに休止する。
PR99の対象実装は `f459596e87dfe28b24954cfad82472626033ea91`。
第20章を含む10章の生成物を採用し、ローカルの14 canonical試験、
canonical --check、format、Clippy、repository検査は成功した。
独立した全文・生成物レビュー、JS無効の3ブラウザ表示とGitHubの5 anchor、
compiler不要の12ファイル復元も確認済み。証拠は
`conformance/results/doc-canonical-doc-html/` にまとめる。
exact-headの必須CIと最終監査に合格してからmergeする。

以下の9月9日の「未完了・merge禁止」は当時の記録であり、現在の状態への指示ではない。
当時の停止や誤ったbinary流用を成功記録へ変更しない。
T21/T16・Playground・Pages公開・処理系全体の完成は宣言しない。

再開時は最新mainと `doc/canonical.json` を確認し、未移行稿を
`doc/authoring.md` に従って独立レビュー・実生成・リンク検証後にページ単位で切り替える。
第11章はRelative linkだけでなく隣接Listの表現も確認する必要がある。
巨大なreview文書のWorkLimitとsource検査費用は未解決であり、単に上限を増やして
全体成功としない。Pages公開も未完了である。

別worktree `C:/projects/NEPL3-doc-current-reader-work` の
`crates/foundation/core/src/budget.rs` と `tools/src/doc/source.rs` の未保存変更は
本PRへ含めず保全する。休止時のclean確認はmainと本作業branchを対象とする。

## 2026年9月12日の再開

下記は9月9日の休止記録である。再開後にmainを統合し、第20章を含む10章の
集合生成用予算をMarkdown Work800M/Allocation2B、HTML Work400Mへ調整した。
Nodes・その他の資源・core/各ページのparse/lower既定値は変更していない。
同じ入力からMarkdown11ファイル、HTML12ファイルをそれぞれ2回生成し、
全byte一致を確認した。第20章のprojectionと第01/12章のcontext headerを採用した。
従来の9文書のHTMLとCSSはbyte一致している。停止の過去記録は成功へ変更しない。

Markdownの実測はWork607,160,055、Allocation1,633,873,615で旧上限をともに超える。
これはreceipt serialization前の論理使用量であり、実メモリ使用量や最小予算ではない。
独立した全文・生成内容レビューに加え、実ブラウザ・GitHub anchor・再生成検査・
必須CIの最終確認を継続する。以下の「未再生成」は休止時点の状態を示す。

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
