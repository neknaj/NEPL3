# 03. Reader / tokenizer

## 方針

Readerは通常の字句解析以上の再帰・状態・外部実装を許す。共通prefix parserに対しては一つのtokenを返す境界を保つ。正規表現だけを表現能力の上限にしない。

## 1. 呼出し契約

`ReadRequest = {snapshot, start, limit, finalInput, context, state, limits}`。
`ReadReply = Matched(value, end, newState, view, facts, diagnostics) | NoMatch(expected, furthest) | NeedMore(expected) | Failed(diagnostic, recovery) | Stopped(reason) | Await(externalRequest, continuation)`。

limitはこのreaderが参照できる入力末尾。start/end/limitは同一snapshotのUTF-8境界。通常tokenの成功は `start < end <= limit`。combinator内部のlook/optional等では空の成功を許すが、token化・skip・反復の反復単位では進捗を要求する。

NoMatchとNeedMoreはstate/facts/確定診断を変更しない。Failedは形式を確定してからの不正入力。finalInput=trueで入力が終わる場合はNeedMoreを返さず、NoMatchまたはFailedにする。StoppedをNoMatchへ変えて別候補へ進んではならない。

`Await`はプログラム製reader/transformの呼出し要求。engine/suiteがmanifestで許可したproviderだけを呼び、同じ予算で再開する。continuationはproviderとsnapshotに束縛される。対象ソースに書かれた任意コードを自動実行しない。

## 2. combinatorの意味

ReaderExprの全signatureはgrammar-signaturesを参照。

- literal: 指定のUTF-8文字列に完全一致しUnitを返す。空literalを認めるが進捗検査の対象。
- scalar: 一つのUnicode scalarが指定classに属すればTextとして返す。
- seq: 順に実行し結果のListを返す。失敗時に状態とfactsをtransaction開始位置へ戻す。
- choice: 順に試し、最初の成功を返す。NoMatchだけ次の候補を試す。NeedMore/Failed/Stoppedはそこで伝播する。
- many/some: 0回以上/1回以上。NoMatchで終了。成功が入力を消費しなければNonProgress。NeedMoreは終了扱いにしない。
- repeat min max p: 有限回のmany。0 <= min <= maxが必要。max到達後はそれ以上読まない。
- optional: NoMatchをNoneにする。その他の失敗は伝播。成功はSome。
- look/not: 元のcursor/state/factsを変更しない。lookは成功/不一致の結果だけ保持。notはMatchedとNoMatchを交換しUnitを返す。NeedMore/Failed/Stoppedは伝播。
- commit p: pのNoMatchをFailedへ変換する。prefixを認識した後の残りを包む。例えば `seq [literal quote, commit bodyAndClose]`。
- capture: 子の成功範囲にfield名を付ける。regionは表示roleを付ける。両者は意味値を変更しない。
- node: 子のmatch treeを指定kindのViewElementで包む。kindは現在packageのschemaへ登録する。
- discard: 成功値をUnitへ変える。sourceと確定診断は失わない。
- ref: 同じpackageのreaderを呼ぶ。進捗のない再帰は静的検査できる範囲で拒否し、実行時にもcallsite+cursor+contextの再入を検出する。
- decode: named pure decoderを成功後に適用し、元→decode後のSourceMapも返す。
- map: named Transform providerを成功値とsource/viewへ適用する。結果は宣言済み型と一致しなければProviderContractViolation。
- then: 最初のparserの値とendを依存parser providerへ渡す。providerが後続の読み取りを行う。返す値は後続parserの値、全体範囲は開始から後続end。前半のsource/factsは保持する。
- call: named Reader providerへ要求全体を渡す。reader全体をRust等で実装できる。
- eof: limitで成功。ただしfinalInput=falseならNeedMore。
- takecount n: n個のUnicode scalarを読む。byte数と混同しない。
- until d: 最初の非空delimiter dの直前までをTextとして読み、delimiterは消費しない。不在なら未確定入力ではNeedMore、確定入力ではNoMatch。

choiceの途中の失敗診断は、最終的に全候補がNoMatchになった場合にだけ、最遠到達位置のexpected集合へ統合する。成功した別候補への誤ったエラーを残さない。work予算は巻き戻さない。

## 3. 文字集合と基礎reader

配布profileではUnicode 16.0.0のXID_Start / XID_Continueを使用し、underscoreを開始にも許す。識別子を勝手にNFC変換しない。比較はscalar列の完全一致。将来のUnicode版変更はpackage revisionに含める。

基礎 `Name`: 上記識別子一つ。`Nat`: `0` または `[1-9][0-9]*`。`Number`: optional `-`、Nat、optional `.` と1桁以上の数字。指数表記はこのsurfaceにはない。`Text`: 通常の二重引用符文字列。`Lang`: ASCIIのwell-formed BCP47 tag。BCP47のtag比較はASCII case-insensitive、元の綴りは保存。登録状況のnetwork照会は行わない。

通常Textのescapeは `\\`、`\"`、`\n`、`\r`、`\t`、`\u{1〜6 hex}`。surrogateとU+10FFFF超は拒否する。未知escapeはエラー。Textではruby/annoを認識しない。

standard triviaはASCII space/tab/CR/LFと `#` から行末直前までのcomment。literal内はtrivia処理を行わない。readerは前側のtriviaだけを処理する。子の最終tokenの後で子言語のtriviaを勝手に消費しない。

これらは配布4言語のprofileであり、全NEPL3言語へ同じ字句規則を強制しない。新しいpackageは別modeとreaderを定義できる。

## 4. modeとtoken決定

modeのskip規則を宣言順に試し、成功して進捗したら最初から反復する。skipがどれも一致しなくなった位置で、take規則を宣言順に試す。NoMatch以外は決定を妨げるので上記に従って伝播する。

Word reader自身が一つの識別子を最長で読む。`letx`を `let` と `x` に分割しない。自然数等の基本readerは後続がidentifierContinueならBoundaryMismatchとし、数値の途中でtokenを確定しない。headのspellingは元のlexemeで比較し、payloadの表示文字列から逆算しない。

構文カテゴリごとのform照合はtokenを読んだ後で行う。同一category・同一spellingの異なるshapeはpackage compile時に拒否する。modeのordered choiceによる優先順位は明示的な仕様である。優先順位が欲しいケースで暗黙の最長一致へ切り替えない。

## 5. Sentence reader

Doc章の完全な再帰規則を参照する。最初の二重引用符を認識したらcommitする。終了引用符は未escapeのものだけ。LF/CRをliteral内に直接置くことは許可しない。明示的な `\n` は内容として許可する。これにより未閉じliteralが後続の文書全体を飲み込み続けることを防ぐ。

sentence decoderはquoted範囲の生のescapeを保持して注釈を認識する。先に全escapeを展開してから `[` の意味を決めない。例えば `\u{5B}` は内容の `[` でありruby開始ではない。

prefix側のarityは常に0。内部viewには本文・delimiter・base・reading・noteの位置を保持する。

## 6. HTML相当の複雑なtoken

conformance用のAngleTag readerは `<`、ASCIIの名前、引用符付き範囲を含む任意のtag内部、`>` の境界を一つのtokenとして認識する。引用符内の `>` で終了してはならない。これは開始tagの境界readerであり、属性の文法・HTMLのtree構築・full HTML適合性までは検査しない。

任意HTML fragmentを扱うproviderは、明示的な入力上限またはheredoc delimiterを受け取り、その範囲内で独自parserを実行できる。外側prefix parserへDOMの子を渡す必要はない。HTMLの安全性検査と内容の解釈はprovider/domain側の操作であり、token化しただけでsafeとしない。

## 7. 解析器の性質

ReaderPlanの空成功・再帰・出力型・参照先を検査する。手書きproviderの停止性を一般に証明できるとは扱わない。trusted native providerは協調的なbudget pollを契約とし、untrusted providerはhostの隔離runnerへ限定する。隔離runnerがない環境ではTrustRequiredで拒否し、workspaceから勝手にnativeコードをbuild/loadしない。

memoizationを使う場合はsnapshot、start、limit/finalInput、reader revision、context/state digest、provider digestをキーに含める。cacheの利用で現在のsourceに属さないspanを返してはならない。性能最適化は結果とdiagnostic codeを変えない。
