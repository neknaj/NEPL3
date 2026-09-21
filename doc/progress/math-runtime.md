# Math runtime の段階実装

## 現在状態を確認する入口

T08の状態と検証記録は[実装状態の正本](../../implementation-status.json)、到達条件は[タスク定義](../../design/tasks.json)を参照する。現在の契約は[第06章](../spec/06-math.md)、Sentenceの所有と接続は[第23章](../spec/23-sentence-annotation.md)で確認する。

以下は構造・数値helper導入時の記録である。Doc Sentenceの旧接続や「残り」の記述は、その段階の状態を示す。現在のMath印字・MathML等の実装判断には、実装状態の正本が指す検証記録を用いる。

## 構造・数値helper導入時の記録

T08は進行中。`doc/spec/06-math.md` と `design/forms.json` を最終契約とし、以下の構造・数値helperの実行を式evaluateやT08全体の完成へ読み替えない。

`nepl3-math-core` はno_std + alloc。workspace production依存は共通coreだけで、Doc coreやengineへ依存しない。任意精度の型付き有理数はcoreのInteger/Rationalを使い、Math側のnum-bigint/num-traitsは既定featureを無効にした明示外部依存である。Math固有のNumber有限十進制約と予算付き算術helperをMath側に持つ。

MathValueは全constructorの平坦なarena、MathSyntaxは元source/Origin/token-local View/SourceMapとForeignClosureを保持する。構造proofは自由記号の解決やCheckedExpressionの証明ではない。Number(1/3)は拒否するが、評価用の一般Rational(1/3)は合法。`construct::expression_from_rational` は有限十進をNumber、それ以外を整数Numberを子とするFracへ明示構築する。lowerはこのhelperで著者のFracを折り畳まない。

`lower::expression` は実compilerが生成したMath表層schemaとchecked SyntaxBundleを受け、同一Budget/SourceAdmissionで再検査・変換する。正式文法の29 formと2 leafを、process不要の保存seedから実compile→ParseSession→lower→初回CBOR受信へ通す。Mathに埋め込むDoc Sentenceは実Doc readerで構文を作り、意味不正な注釈もguest syntaxのまま保持する。診断位置の元になる名前operandとconstructor NodeRefを保持し、Spanを本文検索で作らない。

管理対象の数値helperは1/2+1/3=5/6、0.1+0.2=3/10、128bitを超える整数、除算ゼロと先行予算停止を検査する。これは式のbinding、左から右の評価順、Exact/Symbolic分類や部分評価の完成ではない。構造側はmatrix/vector/fence/root degree、100k深いarenaのClone/Eq/drop、共有最長Depth、source閉包不足、first-receiver停止と元入力不変を検査する。

残りは名前scopeとfree_symbols、CheckedExpression、式evaluateの全exact/symbolic規則と部分結果Report、Math印字/MathML backend、Doc annotationのsuite準備、各操作の完全な要求/返信包絡である。MathValue/MathSyntaxの値schemaだけをこれらのoperation実装済みとしない。M01/M02/M03の全target達成も未宣言のまま保持する。
