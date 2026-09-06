# 14. Web UI・TEA・Playground

## 1. 責務と実装範囲

4言語の操作をブラウザWasmから利用するPlaygroundを最終成果物に含める。専用の言語処理server、localhost LSP、WebSocketを利用条件にしない。T15はWorker/Wasm実行境界を所有し、T17は純粋UI core、T18はWeb editorと操作画面を所有する。現在は計画であり、動く画面・Wasm・完成したUI schemaを提供した状態ではない。

`nepl3-ui-core` は `no_std + alloc` とし、Model、Msg、Cmd、SubscriptionSet、ViewModelと純粋な状態遷移を所有する。依存は共通coreの値・source・操作データ契約に限定する。suite、各言語実装、DOM、Web API、LSP transportを呼ばない。共通操作型を閉じるR006、解決済みProfileを型として定めるR009を先に解消し、UI型の穴を万能辞書やRust pointerで埋めない。

概念上の操作は `init(configuration) -> (Model, Commands)`、`update(Model, Msg) -> (Model, Commands)`、`view(Model) -> ViewModel`、`subscriptions(Model) -> SubscriptionSet`。これらは責務のsignatureであり、未確定のpayloadを実行可能schemaとして広告しない。`interfaces/ui/` に言語中立のfield・variant・不変条件・失敗を定義し、Rust型とwire経路、別実装replayを同時に実装する。

update/viewはclock、乱数、DOM、I/O、Worker、GPUを利用しない。解析・評価・回路stepもCmdでhostへ委譲する。hostは実行結果をMsgへ変換する。保存要求の発行だけで保存済みにはせず、SaveFinished(Result)の対象snapshotを照合して反映する。timer等のsubscriptionも論理的な要求であり、登録・解除・実時刻の取得はhostが行う。同一Modelと同一Msg列からModel/Cmd/View/Subscriptionの意味正規形が一致することを検証する。

Cmdに任意closureを格納して作用を隠さない。局所mutationは外部状態を変えなければ許容する。子Model/Msgへ分割でき、巨大な一枚のupdateや汎用widget framework自作を要求しない。subscriptionはIDで差分適用し、listener/Promise/timerからModelを直接書き換えない。入力・選択の反映まで解析debounceに巻き込まない。

## 2. 配置とhost

`crates/ui/core/src/` はmodel/message/update/command/subscription/viewへ責務を分ける。`apps/web/` はRust/Wasm facade、`web/src/` はshell/editor/worker/preview/storage/bindingsの薄いTypeScript adapterを置く。画面の状態遷移、parser、束縛・名前解決をTypeScriptへ再実装しない。将来のnative UIも同じcoreを使えるが、専用native GUI製品を新しい必須成果物にはしない。

TypeScript境界はnull/undefined・未知payloadを検査し、Option/Resultへ変換する。未検査の型assertionで公開schemaを通過させない。editor/bundler等の依存は通常のpackage manifestとlockfileで固定し、生成bindingsとschemaの一致を検査する。

## 3. 非同期の同一性と停止

各要求・応答は `sessionEpoch`、`workerEpoch`、`requestId`、`sourceSetIdentity`、`profileIdentity`、`operationIdentity`、`optionsAndResourcesIdentity` を照合する。source集合にはdocument identityとsnapshot revision/digestを含める。profileには実際のschema/package/provider revision・digestを含める。各結果slotは現在待っている要求との完全一致だけを受理する。

応答逆転、cancel後の成功応答、close/reopen、Worker再作成、profile/provider/options/resources変更で旧結果を採用しない。単一のsource revisionだけで比較しない。履歴として表示する場合は対象snapshotと旧結果であることを明示し、現在のrename・diagnostic・previewへ適用しない。request IDを再利用するときもepochで隔離する。

協調cancelはbudget/pollで処理するが、長い同期Wasm実行中に通常のpostMessageが直ちに処理されるとは仮定しない。hostは定義済みの停止期限でWorkerをterminateし、epochを進めて再作成する。終了したWorker内の状態・continuationを再利用せず、必要なsnapshot/profileを新しいWorkerへ送り直す。破棄と再起動の途中もUIの処理状態を失敗・停止・再準備として表し、成功へ置き換えない。

Workerだけで完全なsecurity sandboxが成立するとは扱わず、allowlist・入力予算・resource権限を維持する。標準利用にSharedArrayBufferや追加server headerを必須としない。終了により部分結果も失った場合はその事実を表示する。

## 4. Editorのtransaction

editor widgetはcaret、IME composition、undo/redo、layout cacheという局所状態を持ってよい。文書内容の正本はsnapshot契約で一つにする。adapterはtransaction ID、base snapshot、変更byte range、置換Text、新snapshotを照合し、programmatic editの再通知を同じtransactionとして処理する。

IME中にpreview更新でeditorを再生成したり、無条件の全文置換をしない。composition中の一時入力と確定transactionを区別し、確定まで安全に保留する操作を定義する。renameは古いsnapshotを拒否し、複数編集を一つのundo transactionとして適用する。選択範囲のUTF-16と内部UTF-8 byte offsetの変換はadapterで行い、日本語・補助平面文字・CRLF・undo/redo・外部読込と競合する編集を検証する。

毎eventで巨大sourceやASTを複製しない。不変snapshot handle、差分、共有データを使用し、公開wireではpointerを運ばない。WidgetとModelに独立したsource正本を持たせない。

## 5. 四言語の操作

| 言語 | 必須操作と表示 |
| --- | --- |
| Grammar | 文法の編集・検査・package生成、対象DSL sourceの別editor、生成した文法でparse、highlight、定義ジャンプ |
| Doc | sentence literal/prefix、Ruby/Anno、sentence単位parallel、HTML preview、artifact取得 |
| Math | 元の式、bindings入力、明示評価、Exact/Symbolic/Invalid/Stopped、MathML artifact |
| Circuit | 接続・幅検査、入力変更、initial/step/reset、現在state/output、test、SVG、NOR IR |
| 共通 | 例選択、source入出力、診断一覧と位置移動、処理状態/cancel、成果物入出力 |

共通editorはhighlight、definition、references、rename、completion、対応文・生成元への移動も提供する。全文sourceをdebug logへ既定出力しない。ブラウザに存在しないproviderは能力不足として示し、workspaceからnative pluginや任意JSを自動生成・実行しない。

追加DSLのeditor支援はGrammarの構文・束縛・表示定義と共通engineから得る。追加ごとのTypeScript keyword表や名前解決器を要求しない。言語固有の操作panelは許すが、未知DSLのsimulatorやrendererを自動生成した扱いにはしない。機能のない操作は選択肢として成功stubを表示しない。

## 6. Previewとsourceの保護

source表示からguest評価を開始しない。Doc/Math/Circuitの表示は仕様で選んだ操作だけを実行し、Math評価・Circuit step・Grammar compileは明示した操作経路に置く。例の読み込みやURLの変更だけで危険なproviderを許可しない。

自動更新はparse/analyzeと明示的に許可されたbounded previewに限る。高負荷評価、回路step/test、外部作用は明示Msgから開始し、例の表示を自動評価の同意として扱わない。

previewは検査済みMarkup artifactだけを、scriptを許さない隔離されたiframeへ渡す。top navigation、form送信、任意network、同origin権限の付与を避け、hostとの連携は検証したmessageと明示したIDだけを受ける。例外的な機能を必要とするならtrust契約を先に変更する。外部providerの許可とpreviewの隔離は別に検査する。

sourceへの移動はhostの診断・構造paneから提供でき、iframe内のscriptやpostMessage発行を必要としない。hostが受信するその他の連携messageにもorigin・payload・要求identity検査を適用する。

編集中sourceを無断でnetwork送信しない。読込・保存・download・共有はhostの明示操作とし、失敗、保存容量上限、権限拒否を状態に反映する。未保存編集を例選択で上書きする場合の確認は実装契約として設ける。

標準assetは同じ公開artifactから取得し、runtimeのCDN取得を前提にしない。local storageはproject/document/schema版でnamespaceを分ける。例共有は例IDを基本にし、ユーザーsourceをURL queryへ自動追加しない。明示共有のfragment/importも容量とdecode予算を検査する。

## 7. 根拠

Model/View/Updateの分離は [Elm公式ガイド](https://guide.elm-lang.org/architecture/) に基づく。Workerの終了は [HTML Standard](https://html.spec.whatwg.org/multipage/workers.html#dom-worker-terminate) と [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Worker/terminate) を参照する。採用する型境界とcross-platform検証は [ユーザーの設計指針](https://zenn.dev/bem130/articles/1b352797de94e7) に合わせる。Web専用の副作用を純粋coreへ混ぜず、未検証の対応環境を完成扱いしない。
