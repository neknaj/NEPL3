# 00. 対象と設計上の決定

## 方針

本仕様は、実装すべき言語と操作を閉じた契約として定義する。未実装の機能を将来の曖昧な判断へ委ねて現在の成功経路に置かない。拡張点も入力・出力・失敗・許可範囲を定義する。

## 1. 実装対象

Grammar: readerとprefix構造、束縛、表示分類、外部readerの接続を定義し、検査済みLanguagePackageへcompileする。
Doc: 再帰的な文書構造、sentence literal、ruby/anno、sentence単位parallel、相互参照、数式・回路・コードの埋め込みを保持し、HTMLを生成する。
Math: 構造化した数学表現、束縛、厳密有理数と配列の計算、MathML Core出力を行う。
Circuit: 二値・固定幅・同期離散時間の階層回路を宣言し、検査、elaboration、step、テスト、NOR IR、SVG出力を行う。

完成品にはnative CLI、wasm32-wasip2 CLI、browser worker用Wasm、汎用LSP server、portable operation providerが含まれる。

汎用NEPL3プログラミング言語、動画DSL、完全なHTML処理系、アナログ/伝播遅延回路、CAS、証明器、独自フォントrasterizerは本パッケージの言語ではない。これらの名前でstubを提供しない。追加実装は登録済みschema/operation/readerの公開契約から行う。MathML/HTML/SVG出力は正式な出力backendであり、後で捨てる仮のrendererとしない。

## 2. 保存する意味上の境界

- arityはheadを識別した時点の既知のschema/contextから確定する。子の評価結果で親のarityを変更しない。
- listは既知の `cons`(2) と `nil`(0) を展開した構文で表す。listofはschema compilerのcombinatorであり、可変arityの抜け道ではない。
- 複数引数の関数適用を導入する将来言語ではbinary applyを反復する。今回のDSL constructorを関数値適用へ強制変換しない。
- token内部readerの木と共通prefix構文木は別の構造である。editor用のviewを公開しても共通parserの子の数は変わらない。
- Parsed、Resolved、Checked、Preparedを区別する。全言語へ同じ必須pipelineを課さない。操作ごとの前提は各章に記す。
- 意味値だけから元のsource表記やsource spanを逆算しない。

## 3. 試作の扱い

この設計のAPI・モデル・エラー・責務を、後で交換する粗い仮設計として実装しない。後方互換を維持するために誤設計を保存することも要求しない。変更時は新しいdesign revisionを与え、関連する全契約と試験を同時に更新する。

`v1` は契約識別に用いるrevisionであり、将来の無期限のABI互換を約束する呼称ではない。

## 4. 不変条件

INV01 prefixの一意境界。INV02 未知arityの推測禁止。INV03 読み過ぎの禁止。
INV04 source snapshotへの所属。INV05 source mappingの明示。INV06 domain-specific kind。
INV07 値生成と実行対象の区別。INV08 不正な構文の保存可能性。
INV09 言語coreの依存DAG。INV10 portableな意味/操作契約。
INV11 failureを成功に変えない。INV12 同じresource snapshotでの決定的結果。
INV13 sentenceの対応単位は著者が指定。INV14 providerとworkspaceのtrustを区別。

## 5. 具体的な綴り

以前の会話の `Fn`、`value`、`splice` は今回の4言語の組込みではない。構文生成は各言語の公開constructor APIで実際に行える。将来言語は同じconstructor schemaを呼び出して実装する。Grammarの `call` / `map` / `then` は現在もRust providerへ接続できる完全な拡張点である。

各言語の全constructorとarityは `design/forms.json`。そこで未定義の綴りは、該当カテゴリの明示的な識別子leaf規則に一致する場合だけ名前参照として読める。未知のformをarity 0と推測しない。
