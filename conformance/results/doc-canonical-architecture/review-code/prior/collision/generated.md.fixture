<!-- Generated from doc/spec/01&#45;architecture.nepld; renderer nepl3-tools.markdown-annotated-pages/1; page architecture; source SHA-256 8d338b3ce98f3030e503ecd0aea9374c439c606cfbeb48bcd6b777a3f45654b2; alias input SHA-256 68e8d477eace79d400f01a46f2499b7a01ce26b7033726f493fd00c7c0ad90d5; document digest 8528136458369fae27f505fec8d9d6105fb3f07f8084c0c7cabddaaa5f76ed1e; input PageSet digest 3507bd705a27cb5a4e7587f55cd9ba3c02cf55ce5d253c88a17d0067ae2b5337; input context SHA-256 93eb9eefb2e0a57e6ffb508b7fe9a7e59f3617e4dc32c66894f8797a9e433f60. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="01-repositorycrate依存方向"></a>

# 01\. Repository・crate・依存方向\[いぞんほうこう\]

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## 方針\[ほうしん\]

初期\[しょき\]repositoryは `NEPL3` のmonorepoとする。意味上\[いみじょう\]の境界\[きょうかい\]とcrateの依存方向\[いぞんほうこう\]\{consumerからdependencyへの向\[む\]き\}を一致\[いっち\]させ、言語\[げんご\]と出力\[しゅつりょく\]backendを分離\[ぶんり\]する。crateの全一覧\[ぜんいちらん\]と、直接依存\[ちょくせついぞん\]の許可集合\[きょかしゅうごう\]は `design/dependencies.json` に記\[しる\]す。

<a name="n-6c61796f7574"></a>

<a name="1-配置"></a>

## 1\. 配置\[はいち\]

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

ファイル名\[めい\]の階層\[かいそう\]は、ディレクトリで表\[あらわ\]す。例\[たと\]えば `source_span_map.rs` を階層\[かいそう\]の代\[か\]わりに使\[つか\]わず、`source/map.rs` とする。各\[かく\] `src` の直下\[ちょっか\]には、`lib.rs` または `main.rs` を置\[お\]く。

<a name="n-646570656e64656e63696573"></a>

<a name="2-主な依存方向"></a>

## 2\. 主\[おも\]な依存方向\[いぞんほうこう\]

依存\[いぞん\]の向\[む\]きは `consumer -> dependency` と表\[あらわ\]す。

```text
reader -> core
engine -> core, reader
grammar-core -> core, reader, engine
doc-core -> core
math-core -> core
circuit-core -> core
markup -> core
doc-html -> doc-core, markup, core
math-tex -> math-core, core
math-mathml -> math-core, markup, core
circuit-svg -> circuit-core, markup, core
suite -> engine, reader, grammar-core, domain cores, output backends, core
wire -> core
ui-core -> core (公開操作のデータ契約のみ)
apps -> suite, core (+ wire where required, web -> ui-core)
tools -> grammar-core, suite, foundation
```

`core` は、すべてのdomainのenumを持\[も\]つものではない。typed schema、位置\[いち\]、診断\[しんだん\]、公開値\[こうかいち\]を所有\[しょゆう\]する。domain coreは共通\[きょうつう\]のParsed treeを受\[う\]け取\[と\]り、そのdomainのモデルへlowerする。engineはDocやRubyなどの意味\[いみ\]を知\[し\]らない。

Doc内\[ない\]のMathとMath内\[ない\]のDocは、suiteのbridgeが処理\[しょり\]する。doc\-coreはmath\-coreをimportしない。出力\[しゅつりょく\]backend同士\[どうし\]も、相互\[そうご\]にimportしない。suiteが依存関係\[いぞんかんけい\]に従\[したが\]って埋\[う\]め込\[こ\]みを準備\[じゅんび\]し、backendsへ型付\[かたつ\]きの解決済\[かいけつず\]みfragmentを渡\[わた\]す。

ui\-coreは、純粋\[じゅんすい\]なModel・Msg・update・viewと、commandおよびsubscriptionの記述\[きじゅつ\]を所有\[しょゆう\]する。suiteやDOMを実行\[じっこう\]してはならない。実行\[じっこう\]・Worker・editor widgetは、host adapterが担当\[たんとう\]する。目標\[もくひょう\]は20 crateで、そのうち15 crateが `no_std + alloc` である。この目標\[もくひょう\]を、実装済\[じっそうず\]みmemberの数\[かず\]と同一視\[どういつし\]してはならない。文書\[ぶんしょ\]とsiteの生成\[せいせい\]は、toolsの明示的\[めいじてき\]な段階\[だんかい\]で行\[おこな\]う。`build.rs` を通\[とお\]して、compilerと文書\[ぶんしょ\]rendererを循環依存\[じゅんかんいぞん\]させてはならない。

<a name="n-626f6f747374726170"></a>

<a name="3-surface-descriptorとbootstrap"></a>

## 3\. surface descriptorとbootstrap

`languages/*/syntax.neplg` は、各言語\[かくげんご\]のsurface定義\[ていぎ\]である。Grammar compilerがpackageを生成\[せいせい\]し、その生成済\[せいせいず\]みpackageもcommitする。通常\[つうじょう\]のbuildで、toolsやgrammar compilerをbuild dependencyにしてはならない。`cargo run -p nepl3-tools -- generate --check` により、sourceとの一致\[いっち\]を検査\[けんさ\]する。

Grammar自身\[じしん\]の初期\[しょき\]packageは、同一\[どういつ\]のGrammar syntaxを表\[あらわ\]す検査済\[けんさず\]みseedとする。別\[べつ\]の小\[ちい\]さなGrammar dialectは作\[つく\]らない。seedで自身\[じしん\]のsourceを読\[よ\]み、compileしたdescriptorの意味正規形\[いみせいきけい\]とseedを比較\[ひかく\]する。これは自己\[じこ\]ホスト前\[まえ\]の永続的\[えいぞくてき\]なbootstrap契約\[けいやく\]であり、将来\[しょうらい\]の実装\[じっそう\]でも同\[おな\]じseedを使\[つか\]える。

<a name="n-72757374"></a>

<a name="4-rust設定"></a>

## 4\. Rust設定\[せってい\]

workspaceはedition 2024、resolver 3とする。MSRVは1\.85\.0以上\[いじょう\]とし、実装開始時\[じっそうかいしじ\]に使\[つか\]う一\[ひと\]つのstable toolchainを `rust-toolchain.toml` に完全\[かんぜん\]な版番号\[はんばんごう\]で固定\[こてい\]する。MSRVと開発\[かいはつ\]toolchainは同一視\[どういつし\]しない。外部依存\[がいぶいぞん\]はworkspace\.dependenciesで集中管理\[しゅうちゅうかんり\]し、Cargo\.lockをcommitする。

core系\[けい\]crateは常\[つね\]に `#![no_std]` とし、allocを使\[つか\]う。coreをstd化\[か\]するfeatureは設\[もう\]けない。標準\[ひょうじゅん\]ライブラリが必要\[ひつよう\]なアダプタはappsへ置\[お\]く。native\-only crateがcoreに入\[はい\]り込\[こ\]むことを、cargo metadataとtarget buildによって検出\[けんしゅつ\]する。

math\-coreは `num-bigint` \/ `num-rational` \/ `num-integer` \/ `num-traits` を、default\-features\=falseで利用\[りよう\]してよい。対応\[たいおう\]する互換\[ごかん\]version集合\[しゅうごう\]を一度\[いちど\]resolveし、lockする。異\[こと\]なるBigIntの版\[はん\]を、言語間\[げんごかん\]で露出\[ろしゅつ\]させてはならない。公開\[こうかい\]する意味値\[いみち\]には、この仕様\[しよう\]のcanonical integer・rational型\[がた\]を使\[つか\]う。

安全\[あんぜん\]なRustを原則\[げんそく\]とする。FFIや高速化\[こうそくか\]のためにunsafeが必要\[ひつよう\]なら、該当\[がいとう\]adapterの内部\[ないぶ\]へ局在\[きょくざい\]させ、unsafe契約\[けいやく\]と試験\[しけん\]を添\[そ\]える。domainからpointerを外部\[がいぶ\]へ公開\[こうかい\]してはならない。

<a name="n-616c6c6f7765645f646570656e64656e63696573"></a>

<a name="5-許可集合の検査"></a>

## 5\. 許可集合\[きょかしゅうごう\]の検査\[けんさ\]

dependency checkerはproduction依存\[いぞん\]とbuild依存\[いぞん\]を検査\[けんさ\]し、dev依存\[いぞん\]を別\[べつ\]の集合\[しゅうごう\]として出力\[しゅつりょく\]する。productionからappsやtoolsへの経路\[けいろ\]、domain core同士\[どうし\]の経路\[けいろ\]、閉路\[へいろ\]、std依存\[いぞん\]を検出\[けんしゅつ\]する。外部\[がいぶ\]crateの採用\[さいよう\]によって、coreへOS機能\[きのう\]が混入\[こんにゅう\]しないことを確認\[かくにん\]する。

単一\[たんいつ\]repositoryであっても、各\[かく\]coreは独立\[どくりつ\]して `check --no-default-features` できるものとする。将来\[しょうらい\]のrepository分割\[ぶんかつ\]を、現在\[げんざい\]の依存環境\[いぞんかんきょう\]へ強制\[きょうせい\]しない。公開\[こうかい\]schemaと操作\[そうさ\]の単位\[たんい\]で、実装\[じっそう\]を交換\[こうかん\]できるようにする。

外部言語\[がいぶげんご\]の追加\[ついか\]でfoundation sourceを変更\[へんこう\]しない条件\[じょうけん\]と、repository分離前\[ぶんりまえ\]の実証\[じっしょう\]は [外部拡張契約\[がいぶかくちょうけいやく\]](<22\-external\-extensions\.md>) に従\[したが\]う。現時点\[げんじてん\]ではmonorepoを維持\[いじ\]し、独立\[どくりつ\]workspaceの公開\[こうかい\]API利用\[りよう\]から、配布\[はいふ\]・provider・互換性試験\[ごかんせいしけん\]へ進\[すす\]む。

<a name="n-63757272656e745f7374616765"></a>

<a name="現在の作業段階"></a>

## 現在\[げんざい\]の作業段階\[さぎょうだんかい\]

以上\[いじょう\]は、目標\[もくひょう\]とする構成\[こうせい\]である。現在\[げんざい\]のworkspace memberはルートCargo\.tomlを、実装状態\[じっそうじょうたい\]はimplementation\-status\.jsonを正本\[せいほん\]とする。crateは、その責務\[せきむ\]を実装\[じっそう\]する段階\[だんかい\]で追加\[ついか\]する。開発\[かいはつ\]toolchainとMSRVの選定理由\[せんていりゆう\]はdoc\/development\.mdを、リポジトリ整備\[せいび\]の判断\[はんだん\]はdoc\/decisions\/0001\-repository\-foundation\.mdを参照\[さんしょう\]する。
