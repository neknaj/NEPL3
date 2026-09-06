# 09. Rust以外へ置換するための契約

## 方針

意味モデル・操作・診断・source対応のschemaを公開境界とする。Rustのメモリ表現やtrait objectをABIにしない。native fast pathとportable pathを同時に提供し、その意味を比較する。

## 1. 三つの層

(1) 言語中立のschema: field順、variant、必須制約、整数/有理数、範囲と参照、operation signature。
(2) Rustの型付きAPI: schemaに対応するstruct/enumと関数。Rust内の通常利用ではserialize不要。
(3) transport adapter: schema値をNDF（下記）でencode/decodeし、別process/別実装へ渡す。

この分離により、将来のNEPL3プログラミング言語は同じschemaを生成/消費する実装を持てばよい。現在のRustのallocator、enum discriminant、pointer、Arc/Rc、usize、trait vtableは境界を越えない。

## 2. NDF/1

NDFは本仕様の型付き値をCBORで運ぶ符号化profile。RFC 8949のdefinite lengthと最短の整数/長さ表現を要求する。map、float、NaN、CBOR tag、null、indefinite lengthはNDFでは使用しない。CBORそのものの汎用機能を全部許可するわけではない。

値は次のtagged arrayで表す。

| tag | 配列 | 型 |
|---:|---|---|
| 0 | [0] | Unit |
| 1 | [1, bool] | Bool |
| 2 | [2, uint64] | Offset/Index等の有限unsigned |
| 3 | [3, negative:bool, magnitude:bytes] | 任意精度Integer |
| 4 | [4, numerator:Value(tag3), denominator:bytes] | Rational |
| 5 | [5, text] | Text（valid UTF-8） |
| 6 | [6, bytes] | Bytes |
| 7 | [7, [Value...]] | List |
| 8 | [8] | None |
| 9 | [9, Value] | Some |
| 10 | [10, SchemaRef, KindName, [Value...]] | Record、field順はschema |
| 11 | [11, SchemaRef, TypeName, VariantName, [Value...]] | Variant |

SchemaRefは `[packageName:text, revision:uint64, digest:bytes32]`。型名/kind名は登録済みschemaに照合する。naturalはtag3のnonnegativeを要求するfield制約。integer magnitudeはbig-endian、先頭zeroなし、zeroはempty bytesかつnegative=false。rational denominatorは正で先頭zeroなし、gcd=1、zero numeratorはdenominator=1。

NodeId/EntityId等は公開record内のindexとしてencodeし、同じbundleに対応するtableとschemaを含める。未定義参照、循環禁止構造の循環、重複ID、範囲外spanはdecode直後に拒否する。文字列型の違反を遅れてRustのdowncastで検出する方式にしない。

`interfaces/contracts.json` と各言語constructor表からcodecの網羅性検査を生成する。serdeのderive既定表現を契約にせず、custom codecでNDFへ写す。内部でserdeを使う場合も明示したarray/schemaに固定する。

## 3. 操作呼出し

CallはrequestId、OperationRef、input TypedRecord、sources/resources/environment、Limitsを持つ。OperationRefはpackage/revision/digestとoperation名。provider manifestにinput/output schema、必要capability、純粋性契約を宣言する。

ReplyはComplete / Invalid / Stopped / Awaitのvariant。DiagnosticとEventは成功値と別field。未対応operationを空の値や成功Unitで返さない。

Awaitは外部service要求とcontinuationを返す。hostはallowlistで照合し、同一親予算で要求を実行してresumeする。continuationはprovider revision、親request、snapshot digestに束縛し、別providerへ渡せない。中断中の要求を実装差替え対象にしない。

call graphはhostが追跡する。同じoperation/input/contextの循環依存はCyclicOperation。有限だが大きい再帰も共通Limitsで停止できる。provider内部のアルゴリズム固有costは性能情報であり、二実装でusageの数値一致を互換要件にしない。

## 4. native provider

Rustの各coreは型付き関数を公開する。suiteの登録時に、その関数とOperationRefの対応を固定する。wire boundaryを通る場合にだけTypedRecordとencode/decodeを行う。全tokenを常にCBOR化する設計にしない。

呼出し元へ渡すparse treeやsourceは借用してよいが、そのborrow/lifetimeをoperation schemaへ露出しない。wire要求では必要なsnapshot bundleを明示的な値として渡す。

## 5. process provider

apps/providerはstdin/stdoutの8byte unsigned big-endian length + NDF frameを処理する。最大frame長をLimitsで検査する。stdoutへlogを書かない。失敗診断はReply、hostの運用logはstderr。

frame kindはInvoke / Resume / Reply / Cancel / Close。複数要求をrequestIdで識別する。requestIdの重複、知らないcontinuation、schema mismatchをprotocol errorにする。再試行は純粋なoperationに限り、要求全体が同じ場合に行う。

runtime I/Oはこのadapter内に限定し、各coreの計算は純粋入力/出力に保つ。processによる別実装はnative APIの代替として使える。Wasm用adapterも同じCall/Replyを利用でき、wasm32-wasip2自体をこのwire ABIの別名としない。

## 6. 置換手順

一つのoperationについて新実装を登録し、同じconformance入力をnative Rust経路とNDF経路へ渡す。比較対象は意味正規形、定義済みdiagnostic code/位置、参照先、source対応、出力artifactのcanonical内容。traceの内部手順や実行時間は比較対象外。

新実装がpassingになったoperationだけdispatchを切り替える。残りをRustで実行する混在を許す。各coreが別言語coreへ直接依存していないため、単一言語・単一操作から交換できる。

最初のRust実装段階からencode/decode loopbackと別process providerを受入対象にする。「後でABIを決める」作業を残さない。ただし将来言語のコンパイラ・runtimeそのものが既に存在すると仮定しない。
