# 検証記録の履歴

過去のsource、レビューscript、workspace、生成物、実行logはGit revisionから取得する。
現在の段階記録への参照は `implementation-status.json` と `conformance/results/stages/` が所有する。
各参照は元記録のrevision・path・SHA-256を保持し、記録時点の検証範囲を示す。

| revision | 対象path | 確認できる記録 |
| --- | --- | --- |
| `d87ca8c4ba18e74bc71efb0e2958af87ac971e1b` | `conformance/results/` | 2026年9月13日までの検証archive。mainの祖先として到達可能。 |
| `fd8057199f2f32fb36455210df36e78015b77fab` | `conformance/results/` | 今回の整理前の2,046ファイル。後続の文書移行・外部consumer・runtime段階記録を含む。 |
| `fd8057199f2f32fb36455210df36e78015b77fab` | `conformance/results/reader-builtins/allocation-baseline.log`、`allocation-working.log` | R020の同一allocation probeによる修正前後の観測。 |
| `fd8057199f2f32fb36455210df36e78015b77fab` | `conformance/results/markup-html/independent.json` | R049〜R051の独立レビュー記録。 |
| `011d4162126a059da39553c9cdee53b62967c2f0` | runtime source | T11/T12の段階試験対象。mainから到達不能な4 commitを持つ `archive/runtime-ownership-2026-09-23` で保全する。 |

例：

```sh
git show fd8057199f2f32fb36455210df36e78015b77fab:conformance/results/reader-builtins/allocation-baseline.log
git worktree add --detach ../NEPL3-history fd8057199f2f32fb36455210df36e78015b77fab
```

現行試験が使用する最小fixtureは、その試験のdirectoryで管理する。
`tools/site/fixtures/provenance.json` はtar fixtureの元revision・path・digestと保持理由を記録する。
過去の実行結果は対象revisionとscopeに属する。現在のHEADと正式受入の状態は、現在の試験と状態索引で確認する。
