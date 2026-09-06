# 02. 共通データ契約

## 方針

source、種類、構造、意味、解析結果の出自を独立に保持する。エラーやeditor結果を文字列から再解析しない。

## 1. SourceとRange

`SourceId`はworkspace内の不透明ID。`Revision`はそのsourceの版。`SnapshotId = (SourceId, Revision, contentDigest)`。SourceSnapshotはUTF-8の不変byte列とURIを持つ。ファイルだけでなくメモリ文書・生成文書も許可する。

`Span = (SnapshotId, start:u64, end:u64)`。半開区間 `[start,end)`、`0 <= start <= end <= source.len`、UTF-8 scalar境界であることを構築時に検査する。挿入位置には空区間を使用できる。空区間は左/右へのaffinityを必要な操作で別に持つ。行・列・画面幅をSpanへ保存しない。

CRLFは元の2byteを維持する。LF、CRLF、CRをそれぞれ一つの改行としてLineIndexで扱う。BOMは先頭だけのtriviaとして記録し、勝手に除去して以後のoffsetをずらさない。不正UTF-8はSourceDecodeFailureであり、文字置換して位置を捏造しない。

診断・query・編集は必ずsnapshotを指定する。最新revision以外の結果を、単にoffsetを保ったまま最新文書へ適用しない。

## 2. schemaとkind

`SchemaRef = (packageName, revision, digest)`。`KindRef = (SchemaRef, LocalKindId)`。LocalKindIdとfieldの並びはpackage schemaで定義する。Word/String/Variable/Function等を共通の閉じたTokenKindとして置かない。

共通の型言語は Unit / Bool / Natural / Integer / Rational / Text / Bytes / List<T> / Option<T> / Record / Variant / Reference。全Record/Variantのfieldとvariantは登録済みschemaで検査する。domain内部は対応するRustのstruct/enumを使う。

種類と表示classは独立。表示classは拡張可能なIDとfallback roleを持つ。共通fallbackはcontent、marker、delimiter、name、quantity、annotation。これは構文の意味分類ではない。

## 3. Tokenと構文

Tokenは、kind、head span、payload、内部view、triviaへの関連を持つ。arityは別のHeadShapeから取得する。SentenceLiteral内部のviewはprefix childrenへ追加しない。

`HeadShape = {arity, arguments, transitionRecipe}`。argumentsの数はarityと等しい。引数ごとのcontext recipeは既読の子を参照できるが、headのarityは変更できない。

Parsed nodeはkind、head span、enclosing source cover、子NodeId列、opaque payloadへの参照、OriginIdを持つ。通常のsource nodeのchildrenは同一source上で重複しない順序を持つ。生成nodeはOrigin graphを使用し、存在しない連続source範囲を作らない。

token/trivia/sourceのlossless保存により元ソースを再現できる。意味正規形のprinterによるroundtripと、元sourceをそのまま出すlossless roundtripは異なる操作。

## 4. 内部view

`ViewElement = {kind, span, fields, roles, relations}`。一つのtokenに複数のViewElementが対応してよい。子は親の範囲に含まれる。ただし変換後のviewはSourceMapを介した別snapshot上に置く。外側parserはViewElementの木を歩いて構文を決めない。

readerが内部viewを公開しない場合にもtoken全体の位置は必須。その場合、内部の詳細なeditor機能が利用可能であると広告しない。

## 5. Origin graph

`Origin = Direct(Span) | Composite(List<OriginId>) | Generated(operation, callsite, inputs) | Synthetic(reason, anchor)`。

脱出列をdecodeした文字列、macro生成物、回路flatten、数式簡約には多対多の対応があり得る。`SourceMap`は区間同士の関係を表す。単なる定数offsetに限定しない。mapの循環は拒否する。

診断のprimary位置は、問題の直接入力へ正確に対応する範囲を優先する。生成物に対応がなければ生成呼出し箇所をprimary、template/argumentをrelatedとする。sourceがなければ位置なしのglobal診断とし、0行0列に仮置きしない。

renameの逆変換は、一意かつ可逆な対応だけ許す。1対多・合成・正規化で戻せない箇所は、明確な理由付きで編集を拒否する。

## 6. 名前・関係

`EntityId`はsnapshot/analysis内の宣言同一性。綴りのhashだけにしない。`Occurrence`は定義・参照等の役割とsource上の位置を持つ。`ScopeId`、名前空間、親scope、export/import edgeを別に持つ。

参照結果はResolved(entity)、Unresolved(name)、Ambiguous(candidates)、Deferred(requirements)のsum type。definition jumpを単語検索で代用しない。

DocのCorrespondsToは翻訳対応、BindingReferenceは名前参照、Originは生成元。異なる関係を一つの「同じsymbol」へ統合しない。

## 7. 診断とevent

Diagnosticはcode、severity、stage、schema/provider、構造化args、primaryのOption、related列、fix列を持つ。codeはschema所有enumのID。表示言語と文章はrenderer/catalogが決める。

Fixは前提snapshotと非重複TextEdit列。古いsnapshotに自動適用しない。expected text/digestを検査する。複数sourceのfixは一つのtransactionとして返す。

EventはParseStarted/RuleTried/RuleCommitted/BindingResolved/OperationFinished等のschema所有kind、operation path、必要な範囲、構造化payload。domainログが全て文字列である必要はない。TraceLevelはOff/Summary/Detailed、イベント件数にも予算を設け、overflowは一度だけsummary化する。

backtrackingで取り消されたcandidateの診断やeditor factsを成功結果へ混入させない。debug traceだけが試行の記録を保持できる。coreはclockやloggerを呼ばず、hostが時刻と出力先を付ける。

## 8. 予算と結果

LimitsはsourceBytes、work、depth、nodes、allocationUnits、outputBytes、diagnostics、eventsを持つ。全再帰で共有し、言語を切り替えてリセットしない。論理的なlimit検査とOS allocatorの物理OOMは同一ではない。trusted native codeの無限loopを呼出し後の検査で停止できるとは主張しない。

sourceBytesを超える入力は、文字数ではなく元のUTF-8 byte数で判定し、Stopped(SourceLimit)を返す。source snapshotを正常に構築した扱いにせず、他のlimitや構文エラーへ置き換えない。

操作結果はComplete(value, diagnostics, events, usage)、Invalid(partial, diagnostics)、Stopped(reason, partial)を区別する。partialをchecked値として扱わない。入力の問題、未解決の要求、未対応の操作、上限超過、provider違反を別codeで返す。
