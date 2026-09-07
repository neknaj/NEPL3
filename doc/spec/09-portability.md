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

### 2.1 intrinsic値の閉じた記述

`interfaces/contracts.json` の `intrinsic_types` は、NDF/1をdecodeした論理値の型を定義する。`NdfValue` の12caseは上表のtag 0–11に一対一で対応する。`NdfScalar` はUnit、Bool、U64、Integer、Rational、Text、Bytesの部分型、`TypedValue` はRecord、Variantだけの部分型である。subsetのcase列は集合であり、順序をwire IDにしない。Unitは追加payloadを持たず、Noneとは異なる。NaturalはIntegerの非負制約、Bytes32はBytesの長さ制約であり、追加のwire tagを持たない。

このintrinsic記述は通常のdomain sumではない。たとえば `NdfValue.Integer(value: Integer)` の論理fieldを一般のVariantとしてtag11で包まず、上表のtag3へ直接写す。Integerのnegative/magnitude、Rationalの分子tag3と分母bytesは上表の専用表現を使う。tag10/11のfieldsは裸のCBOR arrayであり、Listのtag7を付けない。そのheader内のSchemaRefも裸の `[text,uint64,bytes32]` であり、通常のRecordのtag10を付けない。論理fieldのTextやU64も、上表で裸のCBOR text/uint64を指定する位置へ余分なNDF tagを付けない。codecの物理表現は上表を正本とする。

intrinsicの識別子 `nepl3.ndf/1` は本符号化profileに組み込まれた固定識別子である。intrinsic自体へdomain SchemaRefや自己hashを要求しない。Record/Variant headerのSchemaRefは運ばれるdomain値のdescriptorを識別し、intrinsicの識別子とは別物である。domain descriptorの登録・digest検証は必要であり、TypedValueという型名やtag10/11であることだけでは検査済みにならない。通常のfieldとしてのSchemaRefは `nepl3.foundation` revision 1のSchemaRef recordをtag10で運ぶ。そのheaderのSchemaRefだけが裸のtupleとなるため、無限のwrapper再帰は生じない。

### 2.2 descriptorの型参照検査と残る契約

fieldとunion参照の型式は `Name | List<Type> | Option<Type>` とする。NameはASCII英字で始まる英数字/underscoreのsegmentを `:` または `/` で接続する。空segment、空白、余分なtoken、未宣言のgeneric、引数の過不足は拒否する。List/Optionは予約された1引数constructorであり、名義型として宣言できない。開発checkerは128階層を超える型式を拒否する。この型名文法と上限は設計descriptorの記述に限り、利用者の言語中の名前・kind文字列・source・NDF Textの文字集合を制限しない。runtime入力のLimitsも代替しない。

builtinはUnit、Bool、U64、Integer、Natural、Rational、Text、Bytes、Bytes32である。所有者は本profileであり、modelのscalar_typesとcontractsのscalar_aliasesはそれへの参照・説明である。model.types、contracts.records/enums/intrinsic_typesの名義型定義は重複を許さない。modelの外部参照はexternal_typesへ明示し、存在する外部所有型へ解決する。modelの可視名は自身の定義と明示したscalar/external importに限る。既存の再帰的な型graphは許すが、値の循環可否・source/Origin tableの整合は操作ごとの値検査で別に判定する。

型名がすべて解決することと公開操作契約が完成することを区別する。27操作のinput/outputには説明用の式が残り、型付きの具体化・provider frame・Profile・Doc移行のDG01–DG06は対応する実装とともに完成させる。進捗と実行証拠は [実装記録](../progress/foundation-runtime.md) を参照する。

### 2.3 実行可能なschema descriptor

`interfaces/contracts.json` のTypeDescriptorはUnit/Bool/U64/Integer/Natural/Rational/Text/Bytes/Bytes32、intrinsicのNdfValue/NdfScalar/TypedValue、List、Option、Namedを区別する。Namedはpackage/revision/nameの記号参照でありdigestを入れない。Recordは順序付きfield、Variantは名前付きのvariantと順序付きpayloadを持つ。constraintsはschema所有の意味制約IDの集合である。

SchemaDescriptorはpackage、revision、types、operationsを持つ。canonical JSONではtypesとoperationsを名前keyのobject、型定義を `{constraints,record}` または `{constraints,variant}` とし、fieldを `[name,TypeJSON]` の順序付きarrayにする。TypeJSONはbuiltin/intrinsic名の文字列、`{list:TypeJSON}`、`{option:TypeJSON}`、`{named:{name,package,revision}}` のいずれか。operationは `{input,output,pure}`。constraintsは重複を拒否して名前順に並べる。13章のkey順・escape・domain separatorでSHA-256を求める。

登録は期待するSchemaRefとdescriptorの計算digestを照合する。一つのregistryで同じpackage/revisionに異なるdigestを同時選択しない。全packageを登録後にfinalizeし、使用されていないvariantやoperationも含むすべてのNamed参照を解決する。相互参照する型・packageの登録は許すが、finalize前の値検査・実行は拒否する。

`interfaces/foundation.json` はcontractsから `cargo run --locked -p nepl3-tools -- foundation --write` で生成する実際の `nepl3.foundation` descriptorである。build.rsから生成せず、通常の検査で正本との一致とproduction core registryによる登録・参照閉包を検査する。未完成のoperation説明表を実行可能なoperationsへコピーしない。このpackageは共通値・transport recordのschemaを提供し、言語操作の実装を広告しない。

同じ明示生成で `crates/foundation/core/src/schema/foundation.rs` を作り、productionの `foundation::descriptor` が型付きdescriptorを構築する。構築前に割当・work予算を計上し、toolsのJSON parserをproductionへ依存させない。通常の検査はJSONとRust投影の両方を正本と比較する。wireのsource/Span adapterはこのdescriptorを登録したregistryで構造を検査し、さらにsource digest・宣言順・identity・locator・snapshot・UTF-8境界を検査してnative型へ戻す。

raw encode/decodeはNDF intrinsicのcanonical性を検査する。公開操作の境界ではexpected TypeDescriptorとfinalize済みregistryを渡すchecked encode/decodeを使い、受信したschema/kind/variant/field型を照合する。得られるStructuralValueは構造検査の証明であり、constraintsに列挙したsourceの対応・回路の幅等の意味検査を代替しない。coreの対応constructorまたはdomain操作で必要な不変条件を検査してから使用する。

## 3. 操作呼出し

CallはrequestId、OperationRef、input TypedRecord、sources/resources/environment、Limitsを持つ。OperationRefはpackage/revision/digestとoperation名。provider manifestにinput/output schema、必要capability、純粋性契約を宣言する。

ReplyはComplete / Invalid / Stopped / Awaitのvariant。DiagnosticとEventは成功値と別field。未対応operationを空の値や成功Unitで返さない。

Awaitは外部service要求とcontinuationを返す。hostはallowlistで照合し、同一親予算で要求を実行してresumeする。continuationはprovider revision、親request、snapshot digestに束縛し、別providerへ渡せない。中断中の要求を実装差替え対象にしない。

call graphはhostが追跡する。同じoperation/input/contextの循環依存はCyclicOperation。有限だが大きい再帰も共通Limitsで停止できる。provider内部のアルゴリズム固有costは性能情報であり、二実装でusageの数値一致を互換要件にしない。

### 3.1 Reader Transformの操作返信

Transformのdomain結果と外側OperationReplyは次のように対応する。内側のtyped TransformReplyはsources/sourceMapsを全結果で所有する。

| TransformOutcome | OperationReply | typed payload |
|---|---|---|
| Complete | Complete | TransformReply |
| Failed | Invalid | partial=Some(TransformReply) |
| Stopped(reason) | Stopped(reason) | partial=Some(TransformReply) |

内外のdiagnostics/events/usage/traceOverflowは同じReportの正確な再掲であり、不一致を拒否する。再掲によるstorageと符号化の費用は計上するが、診断・eventを再発行したことにはしない。Stoppedの理由も一致を要求する。CompleteにFailedを隠す、Invalidを成功Unitに変える、Transformに未定義のAwaitを返す、といった返信は拒否する。

partial=NoneのInvalid/Stoppedは、domain Transform結果が得られる前のdispatch失敗である。adapterは型付きの拒否結果と正式Reportをhostへ返し、Readerの待機slotへ成功やdomain失敗として適用しない。この場合の位置参照は元の保存要求と正式checkpointのsource閉包だけに限り、そのsource/mapsを拒否結果が所有する。新しい生成source上の診断を返す場合は、明示source tableを持つtyped TransformReplyを使う。Reportが不正な返信は待機を消費せず、訂正返信を再検査できる。

この対応の実codecは保存要求proofを使うTransform返信のnative/NDF比較を提供する。ReadのMatched/NoMatch/NeedMoreは正常なreader結果としてCompleteへ運ぶ対象だが、その操作返信adapter、初回provider要求、全Await継続、process transportの実装完了をこのTransform比較から推定しない。

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
