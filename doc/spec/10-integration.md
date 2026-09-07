# 10. Profile・埋め込み・実行入口

## 方針

ソース上の相互埋め込みとcrate依存を分離する。suiteが登録済みlanguage package、domain operation、output adapterを接続する。

## 1. Profile

Profileはlanguage alias→SchemaRef、category mode、provider allowlist、operation bridge、resource snapshot、Limitsを持つ不変値。root languageはファイル拡張子またはCLI引数で選び、全ソースを一律のlexerで先にtoken化しない。

`design/profile.json` はsource manifestであり、解決済みruntime Profileではない。R009の解消では生成結果の閉じた型、各schema/package/provider digest、許可capability、resource identityと整合検査を先に定める。T05/T11で実際の検査済みpackageから生成・差分検査し、UI/Workerはこの値を利用する。R006の操作・bundle型の未定義を文字列signatureや仮digestで補わない。

解析の実入口はengineの`ParseProfile`とし、suite Profileから渡す不変なprojectionとして扱う。`interfaces/engine.json`にlanguage登録、category mode、選択schema、provider要件、operation allowlist、resource identity、Limitsの型を置く。`resolve`は独立したhost RuntimeCatalogとfinalize済みregistryを使い、実package意味digest、schema参照閉包、guest category/mode、host provider登録の実装identity、resourceの実byte列digestを照合する。外部Profileの自己申告をそのままhost登録へ複写して検査済みとしない。

provider identityはhostが実assetまたは版管理された実装manifestから確定する。coreはbinaryを読み込まず、binary自身のhashを同じbinary内の定数へ埋める自己hash循環も要求しない。operationのschema署名が既知でもcallbackが登録されたことにはならない。parseに使うreader providerは許可済み実登録を要求し、未実行のfacts等のextensionは署名を検査するが自動実行しない。

解析Profileのidentityは`NEPL3-PARSE-PROFILE-1`、zero byte、canonical JSONのSHA-256。id、language登録、schema、category-mode指定、providerの実装identity、allowlist、resource identity、Limitsを含める。宣言はalias/idまたは完全OperationRef/SchemaRef順、Limits列はsourceBytes/work/depth/nodes/allocationUnits/outputBytes/diagnostics/events順とする。実resource bytesはhash照合し、Profileへ全文複製しない。

HeadProviderの登録はheadProvidersのHeadRegistration(alias,category,provider)で選ぶ。同じpackageを登録した別aliasの設定を共有したと推定しない。(alias,category)重複を拒否し、shape/childContextの両OperationRefについて純粋なHeadCall→HeadReply署名、allowlist、独立host catalogの実装identityを検査する。標準操作名はheadShape/headChildContextだが、登録済みの同署名操作も選択できる。Profile identityにはheadProviders keyを含め、alias/category順の `[alias,category,shapeOperation,childContextOperation]` 列で記述する。列挙順だけの変更はidentityを変えず、操作の役割・alias/categoryへの割当変更は変える。

ここでのallowlistはoperation呼出可否を表す。providerのtransport、隔離、ネットワーク等の権限、強制停止方法、bridge、EnvironmentProjectionを含むfull suite Profileの契約は引き続き実装対象である。この解析projectionだけではR009全体を完了しない。hostは実行環境のcapabilityを別途検査し、解析projectionはその承認を代行しない。

配布拡張子は `.neplg`、`.nepld`、`.neplm`、`.neplc`。汎用 `.nepl` ではlanguage指定を必須にする。既存NCGやGlossのファイルを新言語として黙って解釈しない。

ParseProfile.limitsは解析操作に対するresource別上限であり、単なる既定値ではない。操作開始時に実Budgetの全LimitsがProfile上限以下であることを照合し、超える場合はLimitsMismatchで拒否する。小さい操作予算を使うことは許す。既に消費したUsageはresetせず、継続も同じ操作Limitsと単調なUsageを保持する。Profileのresolve自体を行う開発・host側Budgetはこの解析操作Budgetとは別であり、解決時の消費を解析へ済んだものとして移さない。

EntryContextはaliasを明示保存する。同じpackage identityを異なるaliasで登録してcategory-mode overrideだけを変えることを許し、子のLocal解決も親の実aliasを使う。Profileのlanguage列の並び順は意味に含めず、aliasから選ぶ対応を継続へ保持する。alias別のreader stateとenvironmentも明示し、guest不足をUnitやhost環境で補わない。

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
