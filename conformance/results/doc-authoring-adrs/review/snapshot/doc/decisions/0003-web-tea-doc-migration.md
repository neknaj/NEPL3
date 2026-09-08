# 0003: Web・TEA・Doc DSL移行の必須計画

- 日付: 2026-09-06
- 状態: 採用
- 前設計: `nepl3-design-2026-09-06-r2`
- 現行設計: `nepl3-design-2026-09-06-r3`

ユーザーのWeb/TEA追補 `nepl3-web-pages-tea-2026-09-06-r1` と、最終的に文書をNEPL3 Doc DSLへ移行する直接指示を取り込む。提供された追補7ファイルを全て読み、manifestに記載された6 payloadのSHA-256が元byte列と一致することを確認した。基準commitは `4324078d18b8258531c7b8a15c0f7927d4933f3e`、照合記録は [取り込み記録](../history/web-tea-import.json)。元資料はローカルの説明用資料としてGit管理外に置く。manifest一致は実装やbrowser試験の成功ではない。

U01〜U08、S01〜S06は実物のchange-plan.jsonのID別意味を維持する。会話抜粋だけから作った作業中の分類は、merge前に実物のIDへ照合して訂正した。U04はlifecycle/subscription、U05はIME、U06は追加DSL、U08はschema/境界、S03は4言語browser操作、S04はJSなしのdocsと例、S05は外部backend不要とpreview、S06は配信/公開確認とする。accessibility・性能・リンク・決定的buildの補足は元の条件を置き換えず追加する。移行用J01〜J04は今回の直接指示に応じた追加条件。

純粋な `nepl3-ui-core` を19番目の目標crateに加え、実装済みCargo memberとは分離する。T15のWorker/Wasm境界を維持してT17〜T20へUI・Playground・site・Pagesを分け、T21をDoc移行の必須タスクとする。T16はT20/T21を含むDAGの終点とし、受入群数を37へ固定せず [acceptance catalog](../../design/acceptance.json) の全required群を要求する。

現在は仕様・repository検査の拡張であり、Webアプリ、実行Wasm、Pages公開、Doc変換の完了ではない。Markdownは移行審査まで正本である。表・list・一般リンク・汎用code・図等の表現不足をR014として先に設計・実装し、RawHtmlや内容の平坦化で埋めない。最初の公開に全文書変換は要求しないが、最終完了から移行を外さない。

追補本文はruntime不在でもdocs-only公開を可能とする一方、提案T19がT13/T18へ依存していた。これは早期静的公開を妨げるため訂正し、T19をruntime/UIに依存しないhost静的生成タスク、T20をT15/T18/T19のinteractive統合・配信、T21をDoc rendererによる移行とした。T17も追補どおりT01/T02を前提とし、engine実装を純粋UIの直接前提にしない。群のinteractive部分はT20までnot-runを維持する。

追補8節の「全仕様のDoc移植を要求しない」は初回公開の前提として維持するが、最終範囲はユーザーの後の直接指示を優先しT21を必須とする。T16 completeを配信前提にすると公開smokeと循環するため、配信gateは実行済みchecksで判定し、公開smoke後にT20/T16の証拠を記録する。

R006の閉じた操作schema、R009の生成される解決済みProfile型をUI APIの前提にする。型の名前だけを追加して解消扱いせず、旧指摘を維持する。操作証拠の誤受理を防ぐため、群ID・結果・source/spec identity・必須target・実行logを検査する型付き受入証拠を導入する。

計画の詳細は [Web UI](../spec/14-web-ui.md)、[site](../spec/15-site.md)、[Doc移行](../spec/16-doc-migration.md)。公開規格と元の方針の照合は各章に記録する。
