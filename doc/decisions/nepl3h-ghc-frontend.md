# NEPL3h: GHCへ接続する独立frontend案

状態: **設計草案・実装未着手**。2026-09-13のユーザー提案を整理した文書である。
今回の変更は文書のみとし、draft PRで止める。repo作成、依存追加、compiler試作、
配布、タスク・受入状態の変更をこの文書から自動的に開始しない。
NEPL3側の基準は `3fed238a0021aca770d87c19ecc50fea080e62b1`。
以下のNEPL3h、型名、module名、拡張子は、既存契約への参照を除いて提案名である。

## 目的と既存契約との関係

NEPL3hを、NEPL3の共通表層で記述し、通常のGHC Haskellとして名前解決・型検査・
コンパイルするlanguage packageとする。独自のHaskell風評価器や標準ライブラリを
作る計画ではない。Haskellの意味論は当該packageが所有し、NEPL3全体の意味論にしない。

[外部拡張の22章](../spec/22-external-extensions.md)、[reader](../spec/03-reader.md)、
[交換契約](../spec/09-portability.md)、[統合](../spec/10-integration.md)、
[TEA/Worker](../spec/14-web-ui.md)を接続先とする。
foundationへ言語一覧enumやGHC依存を追加せず、package/schema/operation/providerを使う。
Rust APIの互換性とschema revision・digest、言語版、GHC版、SDK構成は別に管理する。

新規NEPL3hは最初から独立repoを予定し、NEPL3の公開APIへ固定revisionで依存する。
固定revisionから開発できることは、foundation単独の配布・T26/X01/X02の成立を証明しない。
未完成のprocess/provider交換経路を含む22章の受入条件は、実装時に別途満たす。
22章のmonorepo維持は既存実装の運用として維持する。既存Doc・Grammar・Circuitを
NEPL3d/g/c等へ抽出する際は同章の配布・外部consumer・native/portable比較条件を満たす。
Mathやmarkup、統合製品の配置も責務に従い別途決め、既存の四言語・全体受入を縮小しない。
この草案は22章や正式catalogの置換ではなく、新規外部packageの段階案である。

## GHCへ接続する位置

```text
NEPL3h source → reader / prefix engine → 版付き構文モデル
                                              ↓
                                      独立GHC adapter
                                              ↓
普通のHaskell source → GHC parser ───────→ GHC parsed AST (GhcPs)
                                              ↓
                               renamer / typechecker / desugaring
                                              ↓
                              最適化・コード生成・GHC runtime
```

FinkelのBuilderはGhcPsの式・型・宣言等を構築する参考実装である。
GHC Coreへ直接lowerすると名前解決・型検査をこちらで担う問題が生じるため、parsed段階へ接続する。
Finkel固有のコード表現やマクロ体系を恒久依存にせず、NEPL3hの構文から直接変換する。
具体的なhook/constructorはGHC版依存であり、Finkelの実装を無検査で移植しない。

Finkelのdriver hook方式では、依存解析用headerと本体の処理を分ける。
候補はT_HsPp/T_Hsc相当の経路であり、単に式ASTを作るだけでは混在buildにならない。
通常parser後のparsedResultActionだけを任意構文の入口と見なさず、固定GHCの
driver/APIまたはcarrier module経路を検証する。pluginは入口の一つとし、adapter本体は
直接呼出し可能なライブラリにする。cross compilerのhost pluginとtarget libraryを区別する。

## 表層とHaskellの意味を保存する

基本の前置構築をそのまま使う。

```text
apply apply f x y
    → Apply(Apply(Reference(f), Reference(x)), Reference(y))
    → (f x) y

apply apply M.fromListWith + pairs
    → M.fromListWith (+) pairs
```

applyは二項constructor、参照された関数は値である。ライブラリ関数ごとの解析arity登録は不要。
これはNEPL3hで共通prefix規則を使う例であり、foundationや全DSLへapplyの意味を強制しない。
構文arity、callability、saturationを区別する。別言語は先行import/宣言でshapeを確定した
関数やcomponentを直接headにできる。未確定の対象は、categoryが明示的に許す参照leafまたは
明示参照formを使い、固定arity 2のapplyへ適用構造を移す。未知headの自動leaf化はしない。
前方のcontext更新と既読shape不変の詳細は[統合案](multilanguage-hca.md)を参照する。
readerにはqualified name、foldl'、記号名、module名とliteralを定義する。
構文headと同名の関数を参照する明示形式も必要だが、その綴りはschema設計時に確定する。
名前のhyphen等を黙って変換しない。

構文モデルの候補categoryはExpr/Pattern/Type/Declaration/Import/Export/Module。
Apply→HsApp、lambda→HsLam、if→HsIf、let→HsLet、case→HsCase、数値→HsOverLit等を
対応候補とし、GHCの全内部AST・型検査状態をRust側に複製しない。
GHC APIの引数・注釈・parser後処理の前提を固定版で確認する。

型推論、型クラス、ADT、多相性、非正格評価、再帰束縛、IOはGHCへ委譲する。
例えば `apply apply const 1 undefined` は非正格性の比較例にする。
解析順を評価順にせず、再帰letを単純なlambda置換へ、ifを先行評価する通常関数へ、
overloaded literalを固定幅整数へ勝手に簡約しない。
未対応拡張は明示的な能力不足として拒否する。既存libraryの内部で使われる拡張と、
NEPL表層から直接書ける拡張は別の対応範囲である。

## 責務とCabal統合

| 部分 | 所有する責務 |
| --- | --- |
| NEPL3h frontend | no_std + allocのreader、構文schema、解析とSource/Origin |
| GHC adapter | parsed AST構築、版差、compiler session、型・診断の投影 |
| native/cross/browser driver | 環境別起動、package集合、入出力、停止 |
| plugin | Cabal/GHC統合用の入口。adapterと変換を共有 |
| DSL adapter | NDF binding、Doc等の構造生成、境界と挿入先の検査 |

Haskell間では通常の型・関数・module/packageを利用する。containers/text/aeson等を
Cabalの通常依存として解決し、各関数へFFI wrapperやRust往復serializationを挟まない。
NEPL3専用package managerでHaskell依存を再解決しない。

`.neplhs`等の独自拡張子は仮案。Cabalのsource discovery/前処理登録か、build directory内の
`.hs` carrier生成を明示的に選ぶ必要がある。plugin指定だけで発見されると仮定しない。
carrierを使う場合はmodule/importと正本identityを保持し、本体は構文モデルからASTへ変換する。
header解析と本体lowerは同じsnapshotを参照する。frontendからproviderを再起動する循環を作らない。
双方向互換は「Haskell依存AをNEPL3h library Bが使い、普通のHaskell consumer CがBをimport」
するbuildで確かめる。複雑な未対応構文は通常の.hs moduleからimportできる経路を保つ。

## Wasm生成とブラウザ内コンパイル

| 経路 | compilerの場所 | 実行するもの・追加条件 |
| --- | --- | --- |
| native生成 | native host | 通常のGHC実行ファイル |
| Wasm cross生成 | native host | Wasm target向けGHCで生成・linkした独立.wasm |
| 配布済みプログラム/マクロ | 生成時のみcompilerを使う | browser/Wasmtimeでcommandまたはreactorを実行 |
| 対話的編集・実行 | browser compiler Worker | Wasm化GHCがbytecodeを生成し、同runtimeで実行 |
| browser内.wasm export | browser | codegen・assembler/linker等まで別途対応が必要 |

GHC 9.14.1資料はtarget専用buildを要求する。native compilerのflagだけでWasm化しない。
GHCのwasm32-wasiとNEPL3のwasm32-wasip2を同じABIと扱わず、初期は別実行単位と
有限NDF値で接続する。Component Model接続は追加adapterとして検証する。
browser runnerはWASI、明示的な仮想FS、引数、stdout/stderr、必要なJS接続を提供する。

GHC開発版のghc-api-browser試験はbytecodeBackend/LinkInMemoryを使う先行例である。
これはNEPL3hの実行証拠でも、GHC 9.14.1正式配布だけで同じ組合せが動く保証でもない。
native GHCとbrowser interpreterをbrokerで結ぶ `-fghci-browser` と区別する。
静的Playgroundではcompilerもbrowserに置く案とし、外部変換サーバーを必須にしない。
browser経路ではRust CLI、GHC、Cabal、preprocessorのnative process起動を要求しない。

compiler・interpreter・既存packageは事前にWasm向けに構築する。
SDKにはpackage database、interface、runtime、C系依存、loader、仮想FSの配置も必要になる。
通常Cabalをbrowserでそのまま動かす前提にせず、利用可能package集合を明示する。
native binaryを流用せず、非relocatableなpackage配置にも注意する。
GHC commit/build、ghc-wasm-meta revision、WASI toolchain、adapter、NDF/DSL binding、
loaderとpackage集合を一組で固定する。現時点では採用buildを確定していない。

## DSL構造生成、構文マクロ、TH

共通の生成slot・操作契約・複数producerと構文コメントの所有関係は、
[統合案](multilanguage-hca.md)で定義する。Hはその操作の実装言語の一つであり、
Doc/C/Aに必須の評価器にはしない。以下のHからDocへの生成は具体例であり、
全言語向けの標準呼出しをH専用の関数型へ固定するものではない。
統合案のAnnotated<T>はparse treeへ保持し、host loweringだけが注釈を実行意味から射影する。
標準構文はarity 2の`annotate Sentence target`に統一する。
Sentenceは独立したNEPL3sentenceが所有し、NEPL3aは文章とtargetの付与関係を所有する。
Dの本文とMathの文章注記はNEPL3sentenceを利用し、Aを必須の本文経路にしない。
旧lexical commentの恒久互換、#:による特殊trivia、独立comment、commented aliasを追加しない。
現Grammarにgeneric categoryがあるとは扱わず、各host surfaceに具体的な固定shapeを登録する。
wrapperはtargetの意味に加えbinding/exportも保存する。文章の名前空間は独立させる。
通常の.hsファイルの`--`や`{- -}`はGHCの入口が扱う。NEPL3h tokenizerに同じskip commentを
追加する許可にはならない。NEPL3h側の説明は正式なannotate構文として保持する。

最初の合成例は、普通のHaskell関数で既存libraryによる集計結果をDocの表へ変換するものとする。

```haskell
-- 説明用の型案。現行SDK APIではない。
expand :: ExpansionInput -> Either ExpansionError DocFragment
```

hostが明示的に操作を選択して評価し、埋め込みの解析・表示だけでは実行しない。
Haskell間では通常の値を使い、NEPL3境界では有限NDFデータを使う。
HValue、closure、pointer、compiler sessionをportable valueにしない。
JSONデータ処理用aesonとcanonicalなNDF codecも区別する。
結果はwire、schema、Source/Origin・参照、Doc意味条件、挿入slotの順に検査する。
EnvironmentProjectionで渡した値だけを共有し、guest namespaceを既定で分離する。

コンパイル済みmacro.wasmの呼出しはcompiler全体のbrowser配布に先行できる。
マクロを編集する場合はbrowser GHCで再コンパイルして再展開する。
「マクロ関数のコンパイル」「DSL処理中の関数実行」「生成構造の検査」を分ける。
普通の関数によるDSL生成にTHやFinkel式構文マクロを必須にしない。
NEPL3h自身の構文マクロは段階・依存順・生成名の衛生性・Origin・再展開を別に定義する。
renamerだけで変数捕捉が防げるとは扱わない。TH spliceも表層構文とtarget/capabilityごとに検証する。
readerをNEPL3hで作る場合は依存を先に準備し、当該readerでの解析完了を準備の条件にしない。

## 診断、identity、停止と権限

GHC診断・型・名前解決結果を元NEPLソースへ戻す。位置不明nodeへの一括置換を避け、
UTF-8 byte、tab、行列、日本語、補助平面文字、エディタ位置を区別して検査する。
HLS/formatter/renameが別表層を自動理解するとは扱わない。

cacheと応答identityにはsource集合/revision、schema/frontend、GHC build/target/flags、
Cabal依存、adapter/SDK、参照資源、Profile/options、要求IDとWorker epochを含める。
GHC sessionを同時変更せず直列化し、古い応答を新しい文書に採用しない。
純粋TEA updateはCmdを返し、hostがcompiler Workerを実行し、結果をMsgで返す。
UI coreからGHC・Worker・I/Oを呼ばない。応答採用は14章の完全な要求identityに従い、
sessionEpoch・operationIdentityも照合する。上記の列挙を14章の項目を削る定義にしない。
本体だけの変更、依存マクロだけの変更もcacheを失効させる。

pureな型やSafe Haskellは終了性・OS隔離を保証しない。compiler内のTH等も実行権限の対象。
compile、評価、遅延結果のserialization、出力検査の全段階に制限を適用する。
GHC内部workをNEPL3 Budgetへ自動換算せず、host期限、出力/ノード/メモリ上限と
強制停止を別に設計する。停止時はepochを更新し、失敗を再予算化で成功へ変えない。
Workerは停止手段であって権限隔離の全てではない。純粋DSLマクロには任意JSFFI、network、
plugin、preprocessorを既定で渡さず、一般アプリ用capabilityと分ける。
compiler不在は能力不足とし、サーバーへ原稿を黙って送信しない。
Doc previewのscript禁止と生成済み文書の閲覧条件は、このcompiler Workerから独立して維持する。

## ライセンスと配布方針

自作NEPL3hはMITを予定し、NEPL3の現行MITを維持する。方式の参考とコードの流用を分け、
Finkel/GHCの具体的コードを流用する場合はBSD条件・著作権・由来を保持する。
自作部分のMITはcompiler/SDK全体をMITだけで再表示する許可ではない。
これは配布設計案であり、未作成binaryや全推移依存の法的適合性を確定した監査ではない。

標準Wasm配布ではGMPを使わないnative bignumを第一候補にする。
ここでnativeは整数backend名であり、Wasm対応を外す意味ではない。
ghc-wasm-metaのrelease flavourが使うGMPとnative flavourを区別し、対応GHC版も確認する。
targetのGMPを外しただけでcompiler/SDK全体がGMPなしとは判定しない。
GMP利用profileは排除せず、実際に選ぶLGPL等の配布条件を明示する。

GMPの公式条件は本体をLGPLv3以降またはGPLv2以降とし、tests/demo等は別に扱う。
LGPL経路の静的Wasm配布では通知・条文・対応ソース/patchに加え、改変libraryとの
再結合に必要なアプリ側材料・toolchain・手順を揃える。repo URLやSBOMだけで履行としない。
動的リンクやrepo/process分割だけで義務が消えるとも扱わない。

release時はsource archive、native compiler、browser SDK/rootfs、macro.wasm、利用者向け
出力ごとに、実際の同梱ファイル・リンク物を調べる。GHC/BSD、GMP、WASI shim、LLVM、
Haskell/C/JS依存の条件を個別に保持し、容量削減で必要なNOTICE/LICENSEを除かない。
build時だけのtoolと出力へ入るruntimeを区別する。
自作LICENSE、第三者通知、条文、流用由来、配布物別component一覧はNEPL3h/製品側が所有する。
release gateは版・由来の確定、必要表示の同梱、該当時のソース/再リンク材料の提供とする。

compiler利用だけでユーザーのプログラムや生成DocをMIT/BSDへ変更しない。
出力にコピーしたruntime、template、文章・画像・コードには元の条件を適用する。
Wasm exportには実際に組み込んだ依存に対応する通知を添える。

## 将来の実装段階と受入案

以下は未実行の案であり、現在のtask/catalogへpassedを追加しない。
診断・identity・停止・配布条件は各段階から維持し、最後まで先送りしない。

| 段階 | 必要な証拠 |
| --- | --- |
| reader/schema/AST | applyの入れ子、名前衝突、literal、通常GHC parser由来ASTとの比較。不正形を拒否 |
| module/type/import | GHCによる型推論・型クラス・非正格性・再帰let、元位置への型エラー |
| Cabalと双方向import | containers/text/aeson等、A→B→Cの通常package build、header/body snapshot一致 |
| native DSL生成 | 集計→Doc表、NDF再検査、Origin/slot不適合・偽造結果の拒否 |
| 固定browser基準 | upstream例を固定buildで再現後、同adapterからNEPL3hを型検査・bytecode実行 |
| cross Wasm | command/reactorをWasmtimeとbrowserで実行、nativeとの有限値比較、32bit整数差の明示 |
| macro利用/編集 | compiler不在で既製macro実行、編集macroの再compile、依存変更と循環の検出 |
| 実行・配布境界 | stale応答、非停止、巨大出力、serialization中停止、権限不足、SDK/通知整合 |
| 追加対応 | TH・構文マクロ・エディタ、browser内独立.wasm exportはそれぞれ別の受入 |

最初の実用的な完成条件は、既存Haskell libraryを使うNEPL3h moduleを普通のHaskellから
利用でき、その関数がDoc等の検査可能な構造を返すこと。全GHC拡張・任意FFIの互換性を推定しない。
実装再開前にschema/operation、GHC build、Cabal方式、SDK集合と権限profileを具体化する。

## 根拠と未検証範囲

2026-09-13に公式資料と公開sourceを再確認した。以下のGit revisionは調査対象であり、
採用toolchainやNEPL3hでのcompile成功を表さない。外部sourceやraw logをこのrepoへ複製しない。

- [Finkel Builder](https://github.com/finkel-lang/finkel/blob/17f1cfc35a55d1977defdc96a4e422c91e5cdc80/finkel-kernel/src/Language/Finkel/Builder.hs) と [Hooks](https://github.com/finkel-lang/finkel/blob/17f1cfc35a55d1977defdc96a4e422c91e5cdc80/finkel-kernel/src/Language/Finkel/Hooks.hs): parsed ASTとdriver統合の参考。
- [GHC browser試験](https://github.com/ghc/ghc/tree/82c73b223a985bc0bcc00cb6252b2b535082d831/testsuite/tests/ghc-api-browser): README、playground001.hsのbytecode/session、SDK構成。
- [GHC 9.14.1 Wasm](https://downloads.haskell.org/ghc/9.14.1/docs/users_guide/wasm.html)、[GHC API/plugins](https://downloads.haskell.org/ghc/9.14.1/docs/users_guide/extending_ghc.html): target専用build、browser mode、API境界。
- [ghc-wasm-meta](https://github.com/haskell-wasm/ghc-wasm-meta/blob/358ea50b8496a69da6ce375c0c58bc049dbcb92d/README.md): flavour、WASI、配布構成。追跡branchを固定releaseと混同しない。
- [Haskell 2010 Expressions](https://www.haskell.org/onlinereport/haskell2010/haskellch3.html)、[Declarations](https://www.haskell.org/onlinereport/haskell2010/haskellch4.html)、[Cabal](https://cabal.readthedocs.io/en/stable/cabal-package-description-file.html): 意味保存・build設計の参照。
- [Finkel LICENSE](https://github.com/finkel-lang/finkel/blob/17f1cfc35a55d1977defdc96a4e422c91e5cdc80/LICENSE)、[GHC LICENSE](https://github.com/ghc/ghc/blob/82c73b223a985bc0bcc00cb6252b2b535082d831/LICENSE)、[GMP](https://gmplib.org/manual/Copying)、[LGPLv3](https://spdx.org/licenses/LGPL-3.0-or-later.html)、[LLVM](https://llvm.org/docs/DeveloperPolicy.html): 配布物ごとに確認する原文。

ユーザー提供の研究内容をNEPL3契約へ対応付けた草案であり、Finkel/GHCの試作、Cabal build、
browser実行、ライセンス全件監査は今回実施していない。NEPL3h実装は明示的な再開指示まで行わない。
