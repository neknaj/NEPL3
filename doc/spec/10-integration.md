# 10. Profile・埋め込み・実行入口

## 方針

ソース上の相互埋め込みとcrate依存を分離する。suiteが登録済みlanguage package、domain operation、output adapterを接続する。

## 1. Profile

Profileはlanguage alias→SchemaRef、category mode、provider allowlist、operation bridge、resource snapshot、Limitsを持つ不変値。root languageはファイル拡張子またはCLI引数で選び、全ソースを一律のlexerで先にtoken化しない。

配布拡張子は `.neplg`、`.nepld`、`.neplm`、`.neplc`。汎用 `.nepl` ではlanguage指定を必須にする。既存NCGやGlossのファイルを新言語として黙って解釈しない。

## 2. 標準bridge

| host slot | source入口 | guest root | 操作 |
|---|---|---|---|
| Doc InlineMath | Math | Math.Expr | lower/check/render_mathml |
| Doc DisplayMath | Math | Math.Expr | 同上、display style |
| Doc CircuitFigure | Circuit | Circuit.Design | lower/check/elaborate/diagram |
| Doc Code | Grammar/Doc/Math/Circuit | 対応する根 | source/viewの表示のみ |
| Math Label | Doc | Doc.Sentence | lower/check/render phrasing |

MathのDoc注記は文書全体やparagraphを受け入れない。これでMathMLの注記位置へblock文書が混入しない。Code内の不正なguestはRecover構文として保持できる。render時にもevaluate/compileを開始しない。

言語の切替えは一つの引数の範囲に限定し、終了時にhostのmodeへ戻る。たとえばDoc/math/Math/add内からDoc sentenceへ戻っても、そのguestの外側のtriviaを読み過ぎない。

## 3. 相互依存と再帰

ソース上の有限な入れ子は許す。ForeignSyntaxはroot node、language revision、originと環境参照を保持する。操作グラフは対象nodeとoperationの組を頂点に持ち、suiteが必要な依存順に処理する。

Doc→Math→Docの有限の注記は循環ではない。同じnodeのrenderが再び自分のrenderを要求する等の循環はCyclicOperation。生成によって構造が増える場合も共通予算とoriginを維持する。

## 4. 環境の受渡し

各guestの名前空間は既定で新しく分離する。DocLabel、MathSymbol、CircuitSignalを一つの名前辞書へ入れない。bridgeは必要に応じて `EnvironmentProjection` として、どのnamespace/entityをどの型の外部値として渡すかを明示する。

配布bridgeはMathのfree symbol値をRenderContextから明示的に受け取り、DocLabelとCircuitSignalは自動exportしない。Doc annotation内のlabelはそのsentenceの局所scopeで検査する。呼出し側のlabel参照が必要ならprofileのprojectionで明示する。

## 5. artifactの準備

backendsに任意raw HTML stringを渡さない。suiteがforeign subtreeをtyped MarkupFragmentへ変換し、そのslotに適合する内容モデルを検査する。doc-htmlはPreparedEmbedsの対応表を入力として受け取る。math-mathml/circuit-svgを直接importしない。

asset参照は固定内容とdigestを持つResourceSnapshot。coreがpathから読んだりURLへ接続したりしない。HTML出力は既定でnetwork無しのCSS/MathML/SVG。fontはartifactに同梱しない。外部資源を追加する場合は明示的なartifact dependencyとして報告する。

## 6. CLI

`nepl3 parse --language doc input.nepld --format ndf|json` はRecover treeと診断を出す。
`nepl3 check input.nepld` は必要なdomain検査と参照検査を行う。
`nepl3 render input.nepld --output out.html` は埋め込みをprepareしHTMLを出す。
`nepl3 render input.neplm --output out.html` はMathMLを含むHTMLを出す。
`nepl3 evaluate input.neplm --bindings bindings.ndf` はExact/Symbolic/Invalidを構造化出力する。
`nepl3 grammar compile input.neplg --output out.ndf` はLanguagePackageを出す。
`nepl3 circuit test input.neplc` は全testを実行する。
`nepl3 circuit compile input.neplc --target nor --output out.ndf` はNOR IRを出す。
`nepl3 circuit diagram input.neplc --output out.svg` はSVGを出す。
`nepl3 format input --style prefix|compact` は明示的なformatter。既定はstdoutで、--write時だけファイルを置換する。

終了code: 0 成功（Symbolicは操作が許す正常結果）、1 入力/検査/テスト失敗、2 CLI/config/protocolエラー、3 limit/cancel、4 provider内部違反。stdoutは成果物だけ。sourceのdecode失敗を成功空文書にしない。

## 7. browser/native/WASI

同じsuite APIを利用する。browserではWorkerで計算し、未応答時はWorkerを終了できる。ファイル/ネットワーク権限はWeb shellに限定する。wasm-bindgenのJS undefined/nullは境界でOption/Resultへ変換し、domainへ流さない。

wasm32-wasip2 CLIはWASI I/O adapterを使う。nativeのprocess provider呼出しをbrowser/WASIへ無条件に持ち込まない。該当hostが提供するregistry/runnerのcapabilityを検査する。組込み4言語の基本操作は全targetで使える。
