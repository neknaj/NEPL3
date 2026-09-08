# 15. 静的サイト・例・GitHub Pages

## 1. 公開構成

標準公開先を `https://neknaj.github.io/NEPL3/` とするGitHub Pages project siteを最終成果物に含める。独立HTMLのトップ・docsと、ブラウザWasmで動くPlaygroundを配布する。言語処理serverは設けない。現在のsource artifact配布は維持し、サイト実装前に空のPages siteや架空のWasmを公開しない。

予定する出力はトップのindex.html、playground/index.html、docs/index.htmlとtutorials/reference/design、api/rust、examples/manifest.json、assets内のJS/CSS/Wasm/Worker、build.json。`site/` は配置・template・固定assetを所有し、`tools/src/site/` が生成と検査を行う。

T19はUI/runtimeのタスクへ依存せず、既存の固定版Markdown生成器を使うhost側の静的生成経路を先に完成できる。独自Markdown parserを追加しない。能力状態をdocs-only/interactiveで明示し、docs-onlyにはWasmや動くPlaygroundを要求しない。本文はJSを無効にしても読め、静的文書全pageに巨大Wasmを強制loadしない。未実装の操作入口は未提供と説明する。T19のscope付き完了はS01等のinteractive部分のpassedを意味しない。

T20でT15/T18の実Wasm/Worker/UIを統合して全Web受入とPages公開を完成させる。T21はDoc rendererを使う文書移行を担当する。初期docs-only公開と全体完成を分け、language compilerが未実装なことだけで静的文書生成を止めない。

SiteConfigをbase pathの唯一の設定元にする。既定 `/NEPL3/` と受入用 `/acceptance/project/` の両方で、全HTML、CSS、JS、Worker、Wasm、rustdoc、例manifest、画像、検索、リンクを検査する。root絶対の `/assets/` を混ぜない。baseは先頭・末尾slash付きの正規化済みpathで、query、fragment、`..`、origin混入を拒否する。

トップ/docsは静的なdirectory/index.htmlのroute、Playground内部はhash routeを使う。例は `/NEPL3/playground/#/doc?example=doc.sentence&revision=<commit>`。直接アクセス・再読み込みで復元でき、存在しないpage/例/版を別のものへ黙ってfallbackしない。未知routeは404または型付きのMissingRouteとして示す。

## 2. 正本とmanifest

正式文書はdoc/、文法とreferenceはlanguages/とschema/form表、実行例はexamples/、Rust APIは同じcommitのrustdocを入力とする。サイト専用にコピーした別仕様や別サンプルを手で保守しない。MarkdownはDoc移行完了まで正本として利用できるが、最終Doc移行は必須のT21で管理する。

page registryは安定page ID、source正本の形式とpath、公開URL、見出し/anchor ID、旧URL aliasを保持する。表示見出しやファイル移設でIDを暗黙に再生成しない。例manifestは安定例ID、language/category、source path、byte digest、必要profile、revisionを持つ。「この例を試す」は同じmanifestから取得した同じbyte列を開く。存在しない例、digest不一致、異なるruntime/profile版を拒否する。

例manifestには実行前提と期待する操作/結果も記録する。4言語それぞれの入門、完全なreference、典型例、診断の読み方、成果物取得、CLI/Web比較を用意する。rustdocだけを生成して利用者文書が完成した扱いにはしない。

build.jsonにはsource commit、設計revision、schema/package/providerの解決済みdigest、例manifestと各assetのdigest、renderer/toolchain識別を記録する。`design/profile.json` はsource manifestであり、そのままruntime Profileと呼ばない。R009では解決済みProfileの閉じた型・検査を先に定め、T05/T11で実際のpackage/providerから生成して差分検査する。架空digestや未解決aliasでdispatchしない。R006のschema閉包をUI専用の文字列signatureで迂回しない。

docs-onlyではruntimeの解決済みidentityを未存在として明示し、架空値で埋めない。interactive起動時にはHTML/JS/Worker/Wasmのidentity一致を検査する。配信更新や古いcacheで混在を検出したら、新schemaを旧Wasmへ送らず、再取得またはreloadを案内する。公開済みの全commitについてWeb版を永久保存する義務は設けない。

## 3. 文書buildと利用品質

固定toolchain・依存・asset・入力snapshotで決定的にbuildする。buildで意図しないnetwork取得、時刻埋込、例の評価を行わない。必要なdownloadは明示した取得段階でdigest固定し、生成段階は取得済みresourceだけを使う。欠けたresource、未解決参照、重複ID、unsupported content、失敗したrendererをエラーとする。

検索index、navigation、見出し階層、tableのheader、図の代替説明、keyboard操作、focus、contrast、拡大時の読み順、診断からの移動を検証する。自動アクセシビリティ検査に加え、指定browserでkeyboardと日本語IMEを手動確認する。未実行のdevice/screen reader確認を自動試験から推定しない。

## 4. 配布

T20でWasm/JS/Worker/docs/rustdoc/例manifest/build.jsonを一つのartifactにまとめる。実際の非root baseでbuild・browser・リンク検査を通し、quality成功後に**同じartifact**を配布する。検査後の別rebuildを配信しない。deploy jobだけに `pages: write` と `id-token: write`、`github-pages` environmentを与え、PR jobは公開権限を持たない。

Pagesのworkflow公開sourceを実状態で確認し、fork PRをprivileged workflowで再実行しない。配信は共通concurrency groupで直列化し、公開直前にmainとartifactのfreshnessを確認して古いrunの後勝ちを防ぐ。gateは実行したcode/schema/site checksとartifactに基づき、T16 completeの文字列をdeploy前提にしない。公開smoke→T20証拠→T16完了の順序を成立させる。

公開後はHTTPSでdeep link・Worker/Wasm・例実行・asset digestとbuild identityを照合し、対象URL、commit、artifact ID、検査結果を保存する。事前検査に失敗したcandidateは公開しない。公開後に初めて問題が判明する場合には、検証済み復旧が終わるまで不良candidateが配信される可能性がある。無停止・全cacheの原子的切替を保証せず、公開previewが使えることも前提にしない。docs-only更新は動作確認済みruntimeをdigest固定して利用できるが、異なる版を混ぜて同一runtime版と称さず、build.jsonで各版と互換性を示す。runtime更新には当該runtimeとbrowserの受入条件を要求する。

公開後smokeが失敗した場合はworkflowを失敗として維持し、配信成功の記録を付けない。実Pages deployなしのローカルHTTP確認は公開確認の代わりにならない。失敗を記録するだけで緑の成功statusにする実装は禁止する。

公開済みcandidateのsmoke失敗には、以下の旧成果物への復旧契約を適用する。失敗statusだけでは、すでに切り替わった公開物は元へ戻らない。

## 5. 公開失敗からの復旧

### 保持する成果物とidentity

復旧基準（LastKnownGood、以下LKG）は、事前CIに通っただけのartifactではなく、**実Pages公開後の必須smokeにも合格し、その証拠と復旧用payloadを永続保存した成果物**とする。元source commit、build identity、base/origin、能力状態、schema/profile/asset digest、元のPages payload archiveのSHA-256、公開deployment ID、smoke証拠digest、保存先release/asset IDを結び付ける。デプロイ後にbuild.jsonへdeployment IDを書き足して検査済みpayloadを変更しない。deployment IDとの対応は外側のreceiptで管理する。

復旧用の保存先はrepositoryのimmutableなrecovery releaseとし、予約した `site-recovery/` tagで通常のruntime releaseと区別する。Pagesのpublic smoke合格後、recovery releaseをpublishする前のdraftへ元payload・identity・smoke証拠をすべて添付し、immutableとして公開できたこと、download可能なbytesとdigestを確認してからLKGへ昇格する。現在この機能やPagesを有効化済みとは扱わず、T20で設定・権限・公開後のimmutabilityを実確認する。利用できなければ新candidateの自動公開を開始しない。

保存単位は `upload-pages-artifact` が運ぶ**元のtar payloadのbyte列**とそのdigestである。Actions側のupload wrapper、圧縮やartifact IDは再uploadで変わり得る。復旧時は保存済みtarを検証して現在の復旧runへ再uploadし、Pagesが受けるpayloadの同一性を検査してdeployする。sourceからのrebuild、tar内容の編集、異なるHTMLへの差替えを復旧扱いにしない。

Actions artifactの短いretentionや元runの存在を復旧保存の条件にしない。LKGとその直前の公開検証済みsnapshotを保持し、後継の公開smoke・永続保存・journal昇格がすべて済むまで削除しない。最後の更新から日数が経ってもこの2世代を期限切れにしない。3世代目以降の削除は参照・復旧中でないことを同じ公開lock内で確認し、管理者の明示した整理操作で行う。全過去版の永久保存は要求しない。release全体の削除や外部障害まで不可逆に防げるとは主張せず、次の公開前に復旧payloadの取得とdigestを再検査する。壊れていれば公開を止める。

### 全writerを一つのtransactionにする

通常公開・手動再実行・watchdog・復旧・LKG昇格・保管整理は、同じliteral concurrency group `nepl3-pages-production` を使うpublisher workflowだけで行う。`cancel-in-progress: false` とし、公開直前確認からdeploy、public smoke、昇格または復旧、再smoke、最終記録までlockを保持する。deployだけを一つのjobでlockし、smokeや復旧をlock外へ出さない。別のCI workflowのcancel設定をpublisherに引き継がない。

Pagesへの直接書込み権限を持つ他workflow・tokenや旧branch公開を残さない。手動の復旧も同じwriter経路と `github-pages` environmentを通す。必要なPages権限はpublisher、release/journal書込みのcontents権限は当該保存jobに限定する。concurrencyは同一repositoryで同じgroupを使う実行間の排他であり、外部管理者操作を原子的に封鎖するAPIとは扱わない。待機の順序から新旧を推定せず、lock取得後にcandidateのsource freshnessを再検査する。

`pages-state` 保護branchを公開制御journalに使い、Pagesのsource branchとはしない。journalは世代番号、transaction ID、元run/attempt、candidate artifact/deployment ID、前LKG、最新の公開観測、smoke/復旧結果をappend-only eventで記録する。force pushを使わずfast-forwardだけで更新し、読んだjournal世代と親commitが変わっていれば書込みをやめて再確認する。Git refの競合検出とPagesの切替は一つのatomic transactionではない。

Pagesへの変更要求前にtransaction IDとpayload identityを持つ `DeployIntent` または `RecoveryIntent` をjournalへ永続記録する。APIが要求を受理した後、deployment IDを記録する前にrunが消失する場合がある。未解決intentを次のwriterが見たら、lockを取得できたことだけで前deployが終わったと仮定しない。control-plane上の受理・進行・配信対象を管理者も含め同じ経路でreconcileできるまで、新candidateと復旧の両方を停止する。

LKG昇格順は「public smoke合格→immutable復旧releaseの確定とdownload検証→journalのLKG pointer更新」。途中で失敗したら `HealthyUncommitted` とし、旧LKGを残したまま次のcandidate公開を禁止する。再開時は同じpayload・証拠に加えてjournal/API/公開identityを再照合し、新たなpublic smokeに合格してから昇格を完了する。再smoke不合格はcandidate失敗として以下の照合付き復旧へ移し、対象不明ならRecoveryBlockedとする。過去のsmokeだけを信頼しない。journal更新後に保存物を作る順序は禁止する。

### 失敗candidateだけを復旧する条件

smokeが失敗したら、lock内でjournalが自分のtransaction/deploymentを現在の未解決candidateとして指していること、保存されたAPI receiptと当該deployment IDのstatus、公開build.jsonとasset identityを照合する。HTTPは有限の再取得・cache再検証を行うが、一回のcache hitを現在のdeploymentの証明にしない。

公開identityが失敗candidateと一致し、API/journalに別の新しい公開がなく、LKGの保存payloadと証拠が検証できる場合だけ復旧を許す。候補Aの処理中に確認された後続BをAの復旧で上書きしない。Bが健康なら `Superseded` としてAの復旧を中止する。別candidate、status応答欠落、timeout、journal欠落、cache混在、APIと公開identityの不一致、外部writerの疑いは `RecoveryBlocked` とし、自動で公開物を書き換えない。

公開APIはPages deploymentの作成・特定IDのstatus・cancelを提供するが、expected-current-deploymentを指定したcompare-and-swap切替は公開契約にない。特定IDの `succeed` は、そのIDが現在も配信中である証明ではない。単一writerとjournal、公開identityの照合を前提にし、それでも現在対象を確定できなければ止める。cache混在時にrollbackで安定するだろうと推測して書き込まない。

### 有限の復旧と再確認

適格なLKGを現在のrunへ再uploadし、新しい復旧deployment IDと元candidateへの関係をjournalへ記録してdeployする。復旧はcandidateあたり自動で1回まで。復旧後に元LKGの能力状態に対応するHTTPS smokeとpayload/asset identity検査をもう一度行う。合格した場合だけ `Recovered` とする。journalの `current_publication` は新しい復旧deployment IDと再smoke結果へ更新し、`last_known_good` は元payload/source/元smoke証拠の保存先を維持する。古いdeployment IDを現行配信のIDとして再利用しない。元candidateとそのrunの結果はfailedのまま維持し、復旧成功を元candidateの成功へ書き換えない。

復旧payloadのsource commitがmainより古いことは意図した動作である。復旧のfreshnessは現在の失敗transaction・deployment・公開identityに対して検査し、新candidateをmainへ照合する規則と混同しない。復旧の実行codeは信頼済みpublisherを使い、古いsourceのworkflowを無検査で実行しない。

初回公開にLKGがなくsmokeが失敗した場合は `BootstrapFailed` とし、復旧済みと主張しない。公開が不完全な可能性とLKG不在を明示し、後続の通常自動公開を停止する。既存のpayloadを削除したり、未検証の保守ページへ置換したりせず、管理者が同じwriter経路で状態を確認し、検証済みの修正candidateを明示的に選んで初回公開を再開する。

復旧payloadの取得失敗・digest違反・復旧deploy失敗・再smoke失敗は `RecoveryFailed`、lock所有runの強制cancel・runner消失・journal途中状態は `RecoveryUnknown` とする。自動復旧の再帰や無限retryをしない。後続publisherは未解決journalを見たら通常公開を停止し、同じlockでreconcileする。状態が確定しない間は書込みを拒否する。管理者操作であっても新しい健康な公開を無条件に上書きしない。

上限は各deploymentのstatus待機600秒、各public smoke300秒、identity再照合5回かつ120秒以内、復旧payload取得300秒、transaction全体3600秒とする。個々のcheckにも終了条件を持たせる。GitHub側の停止や全体timeoutでcleanupが必ず動くとは仮定せず、journalを次回のreconcileへ残す。

| 状態 | 条件と次の動作 |
| --- | --- |
| DeployIntent / RecoveryIntent | 書込み前に保存。応答とdeployment IDが未確定なら新規書込みを停止 |
| HealthyUncommitted | public smoke合格、永続保存またはLKG昇格が未確定。同じ内容の確定処理だけを再開 |
| Healthy | public smoke・永続保存・journal昇格がすべて合格。次candidateを許す |
| Recovered | 旧payloadの再deployと再smokeに合格。元candidate/runはfailed |
| Superseded | 後続の健康な公開を確認。旧candidateの復旧を行わない |
| RecoveryBlocked / RecoveryUnknown | 対象不一致・不明・未解決intent。reconcileまで書込み禁止 |
| BootstrapFailed | LKGなしで初回smoke失敗。自動削除を行わず、明示した修正candidateで再開 |
| RecoveryFailed | payload取得・deploy・再smokeが失敗。自動再帰をせずincidentに残す |

元runの失敗、復旧の最終状態、対象/復旧deployment ID、payload digest、観測identity、API結果、試行数、時間上限、残る公開影響と次に必要な操作を一つのincident記録へ保存する。失敗ログと再smoke証拠を保存し、workflowをfailedで終了する。復旧不能・対象不明はrun summaryとincidentで管理者へ明示し、別途メール等へ送信する機能を暗黙に追加しない。

これらはT20/S06の実装契約であり、現在のsource artifact CIへ未実装の復旧jobを追加した状態ではない。

Doc移行に使う既知のrendererと同revision runtimeの試験を分ける規則は次章に従う。公開URLの準備だけでT20、T21、T16をcompleteにしない。

GitHub Pagesの静的公開とproject siteの仕様は [GitHub公式資料](https://docs.github.com/en/pages/getting-started-with-github-pages/what-is-github-pages)、workflowの権限・artifact・environmentは [custom workflows公式資料](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages) に従う。

復旧契約の制約は [Pages REST API](https://docs.github.com/en/rest/pages/pages)、[deploy-pages](https://github.com/actions/deploy-pages)、[Actions concurrency](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#concurrency)、[artifact削除とretention](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/remove-workflow-artifacts)、[immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases) に照合した。GitHubの機能を組み合わせた上記の状態機械はNEPL3の設計判断であり、GitHubが自動rollbackを保証するという意味ではない。

## 数式生成assetと書出し文書

[17章](17-math-html.md)のKaTeX実装・CSS/fontは同版の同一artifactに束縛し、path・digest・license・CSS相対font参照を検査する。PlaygroundのWorkerはその資源からHTMLを生成し、previewと書出しで同じ生成artifactを使う。S01/S02/S03/S04/S05では非root path、資源欠落時の診断とMathML設定、外部CDNなしの閲覧、JavaScript無効の書出済み文書を検査する。対話生成がJavaScriptを必要とすることと、生成済み文書の閲覧条件を分けて表示する。
