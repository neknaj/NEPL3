# 実装指示

## 規範

`doc/spec/` と `design/forms.json` を実装契約とする。既存の会話にある説明用の文法へ勝手に戻さない。設計の誤りが発覚した場合は、仕様・schema・全影響箇所・conformanceを同じ変更で修正し、変更理由を記録する。実装困難な部分を省略して完了と報告してはならない。

## コード品質

全coreは `#![no_std]` と `extern crate alloc`。プラットフォームI/O、時計、環境変数、ネットワーク、stdout/stderrはappsまたは開発用toolsだけで扱う。通常の失敗は型付きResult、未存在はOption。入力によるpanic、unwrap、todo、unimplemented、黙殺fallbackは禁止。内部不変条件のassertも外部入力の検査後に限る。

core内の公開データはstruct/enumで表す。schemaの動的境界を除いて、文字列キーの万能辞書や `Any` を使わない。外部境界では必ずschema validationを行う。coreの標準出力へのログは禁止。診断・eventを返り値として返す。

`design/dependencies.json` にないworkspace依存を追加する前に依存仕様を修正し、DAG・no_std・責務を再検査する。doc/math/circuit core同士の直接依存は禁止。production側からtools/appsへの依存は禁止。ファイル名へ擬似的な階層を `_` で詰めず、ディレクトリで責務を表す。

## 完成条件

各タスクに記載した失敗系・境界系・roundtrip・cross-target・portable-provider比較を実行する。goldenの更新を正しさの代わりにしない。期待値の根拠をテストに記述する。未実行を成功として報告しない。

## 作業の順序

`design/tasks.json` の依存順に、利用する前段成果物が成立した範囲を実装する。`depends_on` は全タスクの完了条件でもあり、全依存がcompleteになるまでは当該タスクをcompleteにしない。段階着手を依存先全体の完成と混同しない。段階ごとにbuildできることと、最終仕様を縮小することは別である。後段の未実装を成功Resultやダミー成果物で覆わない。実装状態は `implementation-status.json` で仕様状態と分離する。

Rust以外によるcore実装への置換では、同じ操作schemaとconformanceを使用する。公開契約をRust型のABI、serdeのデフォルトenum表現、ネイティブpointerに依存させない。

## このリポジトリでの作業

- テキストはUTF-8。PowerShellで読み書きする場合はEncoding UTF8を明示する。通常ファイルはLF、source位置試験のfixtureは元byte列を維持する。
- Docの例・解説は [執筆指針](doc/authoring.md) に従う。通常の本文はsentence literalを基本とし、強調・参照・埋め込み等が必要な文は明示的なsentence構築を使う。parallelは文単位の意味対応を保ち、教材では両表記を適切に残す。
- ユーザー指示を方針・契約に照らして検証し、問題があれば根拠を示す。ライブラリ仕様が曖昧ならWeb検索で公式資料を確認する。
- `.tmp/` は説明用のローカル資料でありGit管理・ビルド・CI・配布へ含めない。正式な入口は `doc/README.md`。
- 現在は共通runtimeの実装を進めている。20 crateは目標構成で、実際のmemberはCargo.tomlが示す。R006/R009/R014は実装開始を禁止する条件ではなく、実装とともに解消し公開契約を確定する前に検証する課題である。型付きschema・Rust API・codec・不変条件試験を対応させ、空のcrateや成功stubを揃えない。
- 実装状態の正本は `implementation-status.json`、タスク定義の正本は `design/tasks.json`。タスク本文・索引は `cargo run --locked -p nepl3-tools -- tasks --write` で生成する。
- 変更後は `doc/development.md` の検査を実行し、diffを確認する。未実行の受入試験をCI成功から推定しない。
- `tools` は開発host用。JSONなどの動的入力境界でのみ動的な値を使い、言語の意味モデルの代用にしない。
- Web UIは純粋TEAのnepl3-ui-coreとhost adapterを分け、UI coreからsuite・DOM・Worker・I/Oを実行しない。UI/Workerの型・codec・不変条件を実装してR006/R009の関連課題を解消し、公開契約を確定する前に検証する。
- 最終文書はNEPL3 Doc DSLへ移行する。未移行ページはMarkdownを正本とし、T21の表現gap監査・意味同等性・リンク/安定ID・bootstrap検査後にページ単位で切り替える。移行済みページの正本はdoc/canonical.jsonを参照し、生成Markdownを手編集しない。AGENTS等の必要なMarkdownはDoc正本からの生成projectionを許すが二重手書き保守は禁止。
- 日本語文書の振り仮名は [GlossのRuby指針](https://github.com/neknaj/gloss#ruby) に従い漢字部分へ付け、仮名・数字・記号をRubyの本体へまとめない。送り仮名や助詞は通常のTextとし、語句全体の英訳はAnnoへ置く。prefix構築では必要に応じてAnnoのbaseをConcatで組み、漢字部分のRubyと通常Textを併存させる。これは日本語文書の執筆方針であり、Rubyによる他言語の発音注釈をDSLの型やparserで禁止する規則ではない。
- 受入群と必須targetはdesign/acceptance.jsonを正本とし、T16は全required群を動的に要求する。CI成功や証拠pathの存在だけでruntime/Web/文書移行をpassedへ変更しない。

Doc/Mathの数式HTMLは `doc/spec/17-math-html.md` に従い生成時KaTeX優先・独立MathML fallbackとする。CLIはhost、WebはWorkerで生成し、preview/書出し済み文書内ではKaTeXを再実行しない。

## 実装・独立レビュー・統括の分担

- 実装はメインagentが担当し、subagentには独立レビューを依頼する。設計具体化・コード・試験・指摘修正の実装をsubagentへ委任しない。
- ユーザーが明示許可したDoc文書の書き直しに限り、subagentによる並行執筆を行う。各担当はauthoring.mdと原文を読み、機械的置換ではなく文・読み・注釈・構造を判断する。執筆者と別のsubagentが原文・執筆指針・変更原稿を独立レビューする。メインagentが共通契約・実行検証・正本切替を統括する。この例外を通常の処理系実装へ拡張しない。
- メインagentは実装と統括を担当する。作業範囲、依存関係、受入条件を定め、仕様・差分・実行証拠と独立レビューを確認して統合・報告する。共通契約の編集責任もメインagentへ集約する。
- 実装担当の自己確認だけで独立レビューを代替しない。レビュー担当は元の要求・仕様・変更差分・試験を自分で確認し、設計上の誤りと検証不足も指摘する。
- subagentの完了は通知を優先し、待機が必要な場合は原則60秒間隔とする。短い間隔で完了状態を繰り返しpollせず、独立して進められる作業を先に行う。
- 指摘の修正はメインagentが行い、必要な再レビュー・再検証を経てから完了とする。subagentの利用上限や未実行をレビュー成功へ読み替えず、独立して進められる実装・試験を続ける。
- 専用branch/worktreeで未保存変更を保全し、区切りごとにcommit・pushする。未レビューのcheckpointはその状態を明記し、mainへの統合には独立レビューと必須CIを要求する。
- 統合と必要な証拠の保全が済み、使用しなくなったworktreeは適宜削除する。削除直前に絶対path・HEAD・未保存変更・進行中process・他作業からの参照を確認し、Gitのworktree登録とディレクトリを整理する。必要な資料は現在の作業場所または保存済みartifactへ移し、未保存変更とbranch/commitを無断で失わない。
- ChatGPT作成の設計や過去の検証記録を無条件に採用しない。各契約を本文・schema・文法・例・受入条件および必要な公式資料と照合し、訂正理由と影響範囲を残す。

証拠収集は `tools/conformance/README.md` に従う。reviewごとに保存scriptやtracked source snapshotを `conformance/results/` へ追加しない。過去の記録を保全し、新しい実行logicは管理されたtools/testへ、結果はdataへ分離する。
