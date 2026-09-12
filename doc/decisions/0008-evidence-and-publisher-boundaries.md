# 証拠保存と文書publisherの責務を戻す

調査基準: `d87ca8c4ba18e74bc71efb0e2958af87ac971e1b`。2026-09-13。
本変更は新機能開発ではなく、実装・証拠・開発優先度の是正である。
先に調査とこの判断を記録し、以下の移行を実施・検証する。

## 1. 現在起きていること

Git管理対象の `conformance/results/` は10,472ファイル、221,122,398 bytes。
Python本体と `.py.fixture` は1,061ファイル、3,236,358 bytesである。
同じbyte列のPythonが91組あり、各組の最初の1個を除くコピーは205個。
`tools/site/` はPython 52ファイル、4,146行（空行・試験を含む）。
証拠全体の容量をpublisherだけの問題として扱わない。

全Python一覧はGit blobから採取し、実行せず内容・定義・複合責務・同一hashを
調査した。詳細一覧は `python -m tools.evidence.inventory <基準commit> <出力path>`
で必要時に生成し、repositoryへ再複製しない。初回の内容分類では
95件が要確認となった。追加の独立内容監査で、この95件は次の責務へ分類した。
これは歴史的な全反例の再実行や現行testへの移植完了を意味しない。

| 主責務 | 件数 | 今後の扱い |
| --- | ---: | --- |
| 既存toolのコピー・旧版 | 17 | Git revisionと既存toolを参照 |
| ブラウザ表示・ナビゲーション観測 | 23 | 既存browser/site auditへ必要な回帰だけ追加 |
| 文書内容・構造・生成結果の比較 | 12 | 通常の意味・projection試験へ |
| hash一覧作成 | 3 | 共通収集器へ |
| 固定review文章の書出し | 3 | review文章を直接保存 |
| 境界・失敗の直接再現 | 7 | 必要な反例を通常testへ |
| workerの偽応答・sleep | 4 | 管理されたtest fixtureへ |
| 試験用コピー・Rust試験注入・修正 | 24 | source注入の共通frameworkは作らず通常Rust testへ |
| 独立期待値生成 | 2 | production実装と統合せず独立性を維持 |

直近の [acquireのseal](https://github.com/neknaj/NEPL3/blob/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-acquire/review/seal.py.fixture) はレビュー本文の作成、
sourceのコピー、全ファイルのhash列挙を兼ねる。同名のdownload reviewにも
同じ保存算法がある。`verify.py.fixture` は通常・`-O`試験の実行とlog保存に
加え、個別の期限probeを含む。保存算法と検証対象の論理が混ざっている。
さらに4つの現行site試験が過去reviewのtarを直接fixtureとして参照している。

PR #120/#121は未統合。CLIの未commit追加2ファイルは元worktreeに保全し、
この是正へ含めない。公開issueは0件。26タスクの多数が未完で、T11のsuiteも
未実装。publisherの細分化されたhelper完了を本体の進捗に代用していた。

最終差分はmain `b8bae3fd65440f5e48fbec67cbcbfac083b5f291` から独立して統合する。
未統合PR #120/#121のcandidate/download/observations/stage追加は元branchと
保全refに残し、この是正の最終treeへ含めない。既存mainのpublisher本体を維持し、
そのfixture参照・試験配置・責務文書だけを整理する。未導入機能を同時導入して
「整理が完了した」としない。移行途中の1f8b7d6試験と最終treeの試験は分けて記録する。

## 2. 合理的な部分

対象revision、原stdout/stderr、実行環境、独立レビュー、hashは必要である。
既存 `tools/src/evidence/` は受入群、target、source/spec identity、結果、logを
検査しており、これを置換する二つ目の受入判定器は作らない。
生成・検証・upload・公開・LKGは異なる状態として維持する。
GitHubの成功だけからHTMLの正しさや全体受入合格を推定しない。

## 3. 過剰・重複・一回限りの部分

- reviewごとの `seal/freeze/finalize/report` とsource丸ごとの再コピー。
- 通常試験実行・終了コード・log保存を毎回別scriptに書くこと。
- 回帰価値のあるprobeが管理されたtestへ移らず、結果配下でしか読めないこと。
- 同じ処理を別processへ運ぶ小さなworkerとtransportの反復。
- 各APIが安全であっても、完成したpublisherがないまま入口を増やすこと。

ファイル名だけで全scriptの内容を断定しない。全Python一覧は用途・実行主体・
保存理由・同一hashの関係を記録し、sourceコピー、保存算法、通常test runner、
固有reproducer、要確認を分ける。複合用途は複合として扱う。

## 4. 維持する保証とpublisherの分担

| 機構 | 防ぐ失敗・既存保証 | 今後の所有者 |
| --- | --- | --- |
| CIの成功・run identity | Actionsのneeds/outputsは同runを接続する。任意の別run artifactは自動で信頼されない | 同runの接続はworkflow。別run採用時だけ明示的な検証 |
| tar・asset identity | artifact IDとZIP digestだけでは内部HTMLやsourceを検査できない | repositoryのpayload/内容validatorを維持 |
| upload/downloadと資格情報 | Actions公式の固定版actionがHTTP/認証/転送を提供する | 公式actionで実現できる入口では独自HTTP/IPCを重ねない |
| 公開の排他 | Actions concurrencyは同一repository/groupのjob/workflowを直列化するが、Pages側の原子的CASではない | 単一publisher workflow。異なるwriterを禁止し、不明状態は停止 |
| 不明なPOST・再試行防止 | timeout後のremote結果をActions成功/失敗だけでは確定できない | mutation境界のintent/receiptと次回照合を維持 |
| public smoke | deploy API成功は配信byte・全assetの成功ではない | repositoryのHTTP/内容検査を維持 |
| LKGと復旧 | artifact retentionは永続保存でもLKGでもない | T20/S06の保存・対象照合・有限復旧は維持。未実装を合格にしない |
| review evidence | action logは独立レビューやscopeを定義しない | 小さな共通収集/照合ツールと宣言データ |

独自transportを即削除するのではなく、必要な失敗試験を公式actionとの接続に
引き継げた経路から廃止する。代替未検証の安全性を「GitHubが保証」としない。
現行publisherは未完成であり、公開済み・復旧可能とはいえない。ただし補助系の
hardening拡張は停止し、本是正後に新しいpublisher専用milestoneを追加しない。
immutable releasesの設定承認待ちも本体開発の停止条件ではない。

公式artifact転送にはdigest比較があるが、不一致の表示はwarningであるため、
repository側の採用拒否まで自動保証されるとは扱わない。
根拠: [artifactの共有と検査](https://docs.github.com/en/actions/tutorials/store-and-share-data)、
[concurrencyの範囲](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency)。

## 5. 統合・停止する機構

通常command実行、原log保存、manifest作成とhash検査を `tools/evidence/`
へ集約する。対象command・scopeは `tools/evidence/specs/` の小さなJSONで指定する。
独自の式、plugin、callback、実行言語、複雑なテンプレートは導入しない。
初期APIはargv/cwd/期限、原stdout/stderr、終了状態、revision/environmentとhashの
収集・照合に限定する。受入判定、probeの生成、レビュー文章の生成は担当しない。
review本文は人/独立agentが書くdataとし、scriptに埋め込んで再生成しない。
sourceはGit commitとpath/hashで指す。未保存変更を検証する場合はcommitするか、
理由付き差分を残す。無条件にsource全文を証拠へ複写しない。

## 6. 新しい責務境界

- `crates/`, `apps/`: NEPL3本体・言語backend・host入口。
- `tools/generate/`: code generator。
- `tools/src/site/`, Doc/markup crates: 文書生成。
- `tools/site/`: 配布payloadとpublisher。通常testはtest領域へ分離する。
- `tools/evidence/`: 共通の実行/証拠収集/照合。受入判定は既存Rust tools。
- `tools/evidence/specs/`: 収集するcommandとscope。仕様適合ケースでもレビュー本文でもない。
- `conformance/`: 共通仕様への適合を検査する入力・期待値・不変条件・target adapter。
  native、portable、別実装の意味比較を維持する。
- `conformance/results/`: 既存の参照が必要な歴史記録と小さな結果参照。
  今後の汎用レビュー、CI log、deployment journalの保管先にはしない。
- 独立レビューは対象commitとscopeを明記してPRへ残す。実行logはCI artifact、
  公開attempt・journal・LKGはpublisherの永続状態として管理する。
  必要な長期保全は外部archiveへ行い、Gitの中へsource履歴を複製しない。
- 通常回帰testへ移す固有probeはtest sourceへ。一時的実験は `.tmp/`。
  特殊reproducerを残すときだけ保存理由と対象revisionをmanifestへ記す。

## 7. 移行手順と優先度

1. 全script一覧と重複を記録し、基準commit以前をhistorical evidenceとして凍結。
   過去の結果やmanifestを機械的に書換えない。新規増殖を通常checkで防ぐ。
   historical sourceを実行せず、現行testから直接依存しない境界を検査する。
   正規test領域への回帰試験追加まで禁止する検査にしない。
2. 確認済みの共通保存算法だけを抽出し、今回の是正の検証自体に使用する。
   保存ツールを保存するための新しいseal scriptは作らない。
3. 現行testの実行領域とhistorical inputを分離。必要なfixtureはhashを保って
   test fixtureへ移し、由来を記載する。元証拠は履歴として保持する。
4. publisherの入口と責務表を整理し、仕様・CI・タスク・開発手順を対応させる。
   部分実装の安全性を落とさず、これ以上の独自protocol追加を止める。
5. 本体へ戻る次工程はT08の名前scope/free_symbols・CheckedExpression。
   Mathの表示/評価の前提をproduction APIとportable試験で実装する。
   T20の未完了・本来の最終範囲は維持し、T08をpublisher完成待ちにしない。
   最初はletのinit、sum/integralの上下限を外scope、indexをbodyのみへ束縛し、
   shadowing/free_symbols・Origin・予算を検証する。free_symbolsだけで
   CheckedExpression全体やM01/M02/M03の完成とはしない。

## 8. 完了条件

通常のrepository source検査は保全archive refに依存しない。残存する旧source
342pathだけを `tools/src/repository/legacy-evidence-sources.txt` へ列挙する。
これは旧pathの移行例外であり、内容の正当性や現在の受入合格を表さない。
新規review sourceの許可には使用しない。歴史資料の調査だけで旧revisionを取得する。
Doc inventoryの過去baseline監査は別の操作・契約として残る。

通常main pushの `deliver-source` はGit管理sourceを再archiveするだけで、検査や
runtime配布を所有しないため廃止する。CIのcommit参照と全検査gateは維持する。
[GitHubのsource archive契約](https://docs.github.com/en/repositories/working-with-files/using-files/downloading-source-code-archives)
に従い、commitのファイル内容と圧縮archiveのbyte同一性を区別する。
LKGの元payload保全・実行log・正式release成果物をこの削減の対象にしない。

新しい通常reviewで専用Pythonを作らず、同じ共通コマンドで結果を保存・照合
できる。個別scriptの分類と例外理由が追える。historical evidenceは破壊しない。
現行testのsourceとdataの場所が明確で、必要な失敗・境界試験が維持される。
通常repository check/CIが通り、新規script増殖が検出される。
仕様・開発手順・taskが新境界を指し、publisherの未保証範囲と本体の次作業を
明示する。変更行数や新frameworkの大きさを成果と数えない。

## 追加調査: 状態・正本・CI

`task::completion()` は `depends_on` をcomplete時の条件として検査する。
T21の早期監査とspec22のT26独立consumer先行は既に仕様が認めている。
従ってT24/T12が未完という事実だけで着手違反とはしない。前段の必要な成果物が
成立した範囲の段階着手と、依存タスクすべての完了を要求する完成判定を区別する。
依存edgeを削除したり、preparatory等の新状態を増やしたりして説明を合わせない。

進行中14タスクの証拠参照が全て空である点は是正する。既存の
`implementation-status.json` を状態・参照の正本として再利用し、別の手書きledgerは
追加しない。段階記録への参照は過去の対象scopeの索引であり、現HEADの合格ではない。
生成task本文へ参照を投影する。既存の正式受入validatorは引き続き現在identityと
全required targetを照合する。57群のnot-runを過去の部分試験から変更しない。

意味仕様、機械契約、計画、観測、生成projectionを開発手順で区分する。同じ値を
複数の正本で編集せず、不整合は同時訂正する。Doc正本はcanonical.jsonの対応に従う。
doc-inventoryは固定commitを再検査する歴史的監査入力であり、現在の仕様や文書の
正本ではない。削除で再現検査を壊さず、その大きさを理由に追加の管理機構も作らない。
今回のscript inventoryも派生した監査資料であり、言語の規範ではない。
詳細JSONの恒久保存案は撤回した。既存の`.json`/`.json.fixture`は2,623ファイル、
1,176,707行。そのうち過去のresultsが1,047,270行を占める。minifyで行数だけを
減らすのではなく、再取得できるsource一覧・snapshotと歴史的archiveの参照を
Gitへ戻す。現行fixtureと正式schemaは意味と利用者を確認して別に扱う。

CIはnativeのproduction/API・OS境界試験を残し、一度で足りる生成物一致・索引・
publication protocol試験を独立host jobへ移す。必要なWindows/macOSのpath/process
境界試験は選択して残す。qualityは新jobも要求し、必要な検査を解除しない。

後続のworkflow監査では、repository checkがnative 3 OSと専用jobで重複し、
fmt・rustdoc・allocation probe・全文書生成も3回実行されていた。これらは代表の
Linux一回へ集約する。siteのgeneric試験は専用Linux jobを正本とし、他OSには
junction、stdin引数の構築、実processの強制終了、実HTTP/期限、macOSのtemporary-root回帰を残す。
Rustの全workspace試験とClippyは現時点では3 OSを維持し、未分類のhost試験を
誤って省略しない。この変更は同じ検査の重複解消であり、browser/emulatorの
実行頻度変更やrequired gateのskip許容は含まない。

feature branchではpushとpull_requestが別concurrency groupで同時に全jobを
起動していた。branchの自動実行はPRとmain pushへ一本化し、tag pushと手動実行を維持する。
PRの対象branchは制限せず、依存branchをbaseとする段階PRにも同じ検査を適用する。
PRなしのcheckpointを検証済み扱いにしない。main保護・qualityの条件は変更しない。

独立した参照監査後、無参照91単位とルートgitattributes指定のみの29単位を
作業treeから取り除き、[Git履歴索引](../../conformance/history.md)へ移した。
計8,406ファイル、154,303,564 bytes、JSON/JSON fixture 876,848行を参照化する。
baselineを `archive/evidence-2026-09-13` refとしてremoteにも保全した。
原manifestや原ログの内容は変更していない。過去の相互参照とbyte属性を検査する際は
baseline全体を別checkoutへ復元し、現在treeへの部分復元と混ぜない。
既存task/review台帳から参照される記録と現行fixtureは今回の削除対象ではない。

この削減は作業treeの削減であり、Git objectやfresh full cloneの容量削減ではない。
保全refを同じrepositoryに置く限り旧blobは到達可能なままである。次工程では
外部archive・復元検査・除去path/ref・commit参照の移行を準備してから履歴rewriteを
別途判断する。共有historyのforce-pushとremote ref削除は明示承認を要する。
今後のraw実行log・大きな証拠はCI artifact/承認済み外部archiveへ置き、Gitには
小さな参照・scope・manifestを残す。今回の途中区切りで追加したraw logも最終tree
から外し、local distの原manifest/logは外部退避の準備用に保全する。
