# 01. Repository・crate・依存方向

## 方針

初期repositoryは `NEPL3` のmonorepoとする。意味上の境界とcrateの依存方向を一致させ、言語と出力backendを分離する。crateの全一覧・直接依存の許可集合は `design/dependencies.json` に記す。

## 1. 配置

```text
NEPL3/
  Cargo.toml
  Cargo.lock
  rust-toolchain.toml
  AGENTS.md
  doc/
    spec/
    decisions/
    history/
  design/
  interfaces/
  languages/
    grammar/ syntax.neplg package.ndf
    doc/     syntax.neplg package.ndf
    math/    syntax.neplg package.ndf
    circuit/ syntax.neplg package.ndf
  crates/
    foundation/
      core/src/       source/ schema/ value/ syntax/ origin/ diagnostic/ budget/
      reader/src/     plan/ combinator/ execution/ extension/
      engine/src/     parse/ binding/ index/ query/ recovery/
      wire/src/       cbor/ schema/ protocol/
    languages/
      grammar/core/src/ bootstrap/ model/ check/ compile/
      doc/core/src/     model/ sentence/ lower/ check/ facts/
      doc/html/src/     render/ style/
      math/core/src/    model/ number/ lower/ check/ evaluate/ facts/
      math/mathml/src/  render/ precedence/
      circuit/core/src/ model/ lower/ check/ elaborate/ simulate/ nor/ facts/
      circuit/svg/src/  layout/ render/
    output/markup/src/  model/ validate/ serialize/
    integration/suite/src/ profile/ bridge/ operation/ workspace/
    ui/core/src/ model/ message/ update/ command/ subscription/ view/
  apps/ cli/ lsp/ web/ provider/
  web/src/ shell/ editor/ worker/ preview/ storage/ bindings/
  site/
  editors/ vscode/ neovim/
  tools/src/ generate/ contract/ dependency/ task/
  examples/
  conformance/
  tasks/
```

ファイル名の階層化はディレクトリで表す。例えば `source_span_map.rs` を階層の代用にせず `source/map.rs` とする。各 `src` の直下には `lib.rs` または `main.rs` がある。

## 2. 主な依存方向

`consumer -> dependency` とする。

```text
reader -> core
engine -> core, reader
grammar-core -> core, reader, engine
doc-core -> core
math-core -> core
circuit-core -> core
markup -> core
doc-html -> doc-core, markup, core
math-mathml -> math-core, markup, core
circuit-svg -> circuit-core, markup, core
suite -> engine, reader, grammar-core, domain cores, output backends, core
wire -> core
ui-core -> core (公開操作のデータ契約のみ)
apps -> suite, core (+ wire where required, web -> ui-core)
tools -> grammar-core, suite, foundation
```

`core`は全domainのenumを持たない。typed schema、位置、診断、公開値を所有する。domain coreは共通Parsed treeを受け取り、domainのモデルにlowerする。engineはDoc/Ruby等の意味を知らない。

Doc内のMathとMath内のDocはsuiteのbridgeが処理する。doc-coreはmath-coreをimportしない。output backendも相互にimportしない。suiteが依存関係に従って埋め込みを準備し、backendsに型付きの解決済みfragmentを渡す。

ui-coreは純粋なModel/Msg/update/viewとcommand/subscription記述を所有し、suiteやDOMを実行しない。実行・Worker・editor widgetはhost adapterが担当する。目標は19 crate（14 crateがno_std + alloc）であり、実装済みmember数と同一視しない。文書/site生成はtoolsの明示段階で行い、build.rsでcompilerと文書rendererを循環依存させない。

## 3. surface descriptorとbootstrap

`languages/*/syntax.neplg` は各言語のsurface定義。Grammar compilerがpackageを生成し、生成済みpackageもcommitする。通常build時にtoolsやgrammar compilerをbuild dependencyにしない。`cargo run -p nepl3-tools -- generate --check` でsourceとの一致を検査する。

Grammar自身の初期packageは同一のGrammar syntaxを表す検査済みseed。別の小さなGrammar dialectは作らない。seedで自分のsourceを読み、compileしたdescriptorの意味正規形とseedを比較する。これは自己ホスト前の永続的なbootstrap契約であり、将来の実装でも同じseedを使える。

## 4. Rust設定

workspaceはedition 2024、resolver 3。MSRVは1.85.0以上、実装開始時に使用する一つのstable toolchainを `rust-toolchain.toml` に完全な版番号で固定する。MSRVと開発toolchainを同一視しない。外部依存はworkspace.dependenciesで集中管理し、Cargo.lockをcommitする。

core系crateは常に `#![no_std]`。allocを使用する。coreをstd化するfeatureは設けない。標準ライブラリを要するアダプタはappsへ置く。native-only crateがcoreに入ることをcargo metadataとtarget buildで検出する。

math-coreは `num-bigint` / `num-rational` / `num-integer` / `num-traits` をdefault-features=falseで利用してよい。対応する互換version集合を一度resolveしてlockする。異なるBigIntの版を言語間で露出させない。公開意味値は本仕様のcanonical integer/rational型である。

安全なRustを原則とする。FFIや高速化にunsafeが必要なら該当adapter内へ局在させ、unsafe契約と試験を添える。domainからpointerを外部へ公開しない。

## 5. 許可集合の検査

dependency checkerはproduction・build依存を検査し、dev依存は別集合として出力する。productionからapps/toolsへの経路、domain core同士の経路、閉路、std依存を検出する。外部crateの採用によりcoreへOS機能が混入しないことを確認する。

単一repositoryでも各coreは独立して `check --no-default-features` できる。将来のrepository分割を今の依存環境に強制せず、公開schemaと操作の単位で実装を交換する。

## 現在の作業段階

上記は目標構成。現在のworkspace memberはルートCargo.toml、実装状態はimplementation-status.jsonを正本とする。crateは責務を実装する段階で追加する。開発toolchainとMSRVの選定理由はdoc/development.md、リポジトリ整備の判断はdoc/decisions/0001-repository-foundation.mdを参照。
