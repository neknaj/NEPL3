<!-- Generated from doc/spec/01&#45;architecture.nepld; renderer nepl3-tools.markdown-annotated-pages/3; page architecture; source SHA-256 e40e874325fb78cc5c7e5d545e9e42a159a81d2f630edd246641d59e84ae5ee4; alias input SHA-256 68e8d477eace79d400f01a46f2499b7a01ce26b7033726f493fd00c7c0ad90d5; document digest 7dd9239e9a6c041123e3c2d6ffb792ad73ed84f9effa5b85022fdf77b14df972; page input SHA-256 9ab5fc4c8f965446b505628243926c10d8b1e4eed619669c888a4c1f165a4eba. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="01-repositorycrate依存方向"></a>

# 01\. Repository・crate・<ruby>依存方向<rt>いぞんほうこう</rt></ruby>

[正本（NEPL3d）](<01-architecture.nepld>)

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## <ruby>構成<rt>こうせい</rt></ruby>と<ruby>依存関係<rt>いぞんかんけい</rt></ruby>の<ruby>設計方針<rt>せっけいほうしん</rt></ruby>

<ruby>初期<rt>しょき</rt></ruby>repositoryは `NEPL3` のmonorepoとする。<ruby>意味上<rt>いみじょう</rt></ruby>の<ruby>境界<rt>きょうかい</rt></ruby>とcrateの<ruby>依存方向<rt>いぞんほうこう</rt></ruby>\{consumerからdependencyへの<ruby>向<rt>む</rt></ruby>き\}を<ruby>一致<rt>いっち</rt></ruby>させ、<ruby>言語<rt>げんご</rt></ruby>と<ruby>出力<rt>しゅつりょく</rt></ruby>backendを<ruby>分離<rt>ぶんり</rt></ruby>する。crateの<ruby>全一覧<rt>ぜんいちらん</rt></ruby>と、<ruby>直接依存<rt>ちょくせついぞん</rt></ruby>の<ruby>許可集合<rt>きょかしゅうごう</rt></ruby>は `design/dependencies.json` に<ruby>記<rt>しる</rt></ruby>す。

<a name="n-6c61796f7574"></a>

<a name="1-配置"></a>

## 1\. <ruby>配置<rt>はいち</rt></ruby>

<ruby>次<rt>つぎ</rt></ruby>の<ruby>配置<rt>はいち</rt></ruby>は、<ruby>各<rt>かく</rt></ruby>crateとツールの<ruby>責務<rt>せきむ</rt></ruby>を<ruby>示<rt>しめ</rt></ruby>す<ruby>目標構成<rt>もくひょうこうせい</rt></ruby>である。<ruby>現在<rt>げんざい</rt></ruby>のworkspace memberは `Cargo.toml`、<ruby>各機能<rt>かくきのう</rt></ruby>の<ruby>実装状態<rt>じっそうじょうたい</rt></ruby>は `implementation-status.json` で<ruby>確認<rt>かくにん</rt></ruby>する。

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

ファイル<ruby>名<rt>めい</rt></ruby>の<ruby>階層<rt>かいそう</rt></ruby>は、ディレクトリで<ruby>表<rt>あらわ</rt></ruby>す。<ruby>例<rt>たと</rt></ruby>えば `source_span_map.rs` を<ruby>階層<rt>かいそう</rt></ruby>の<ruby>代<rt>か</rt></ruby>わりに<ruby>使<rt>つか</rt></ruby>わず、`source/map.rs` とする。<ruby>各<rt>かく</rt></ruby> `src` の<ruby>直下<rt>ちょっか</rt></ruby>には、`lib.rs` または `main.rs` を<ruby>置<rt>お</rt></ruby>く。

<a name="n-646570656e64656e63696573"></a>

<a name="2-主な依存方向"></a>

## 2\. <ruby>主<rt>おも</rt></ruby>な<ruby>依存方向<rt>いぞんほうこう</rt></ruby>

<ruby>依存<rt>いぞん</rt></ruby>の<ruby>向<rt>む</rt></ruby>きは `consumer -> dependency` と<ruby>表<rt>あらわ</rt></ruby>す。

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

`core` は、typed schema、<ruby>位置<rt>いち</rt></ruby>、<ruby>診断<rt>しんだん</rt></ruby>、<ruby>公開値<rt>こうかいち</rt></ruby>を<ruby>所有<rt>しょゆう</rt></ruby>する。<ruby>各<rt>かく</rt></ruby>domainの<ruby>意味<rt>いみ</rt></ruby>モデルと、それを<ruby>表<rt>あらわ</rt></ruby>すenumは、<ruby>対応<rt>たいおう</rt></ruby>するdomain coreが<ruby>所有<rt>しょゆう</rt></ruby>する。domain coreは<ruby>共通<rt>きょうつう</rt></ruby>のParsed treeを<ruby>受<rt>う</rt></ruby>け<ruby>取<rt>と</rt></ruby>り、そのdomainのモデルへlowerする。engineは<ruby>共通<rt>きょうつう</rt></ruby>の<ruby>構文<rt>こうぶん</rt></ruby>・<ruby>束縛<rt>そくばく</rt></ruby>・<ruby>照会<rt>しょうかい</rt></ruby>の<ruby>契約<rt>けいやく</rt></ruby>を<ruby>扱<rt>あつか</rt></ruby>い、DocやRubyなどの<ruby>意味<rt>いみ</rt></ruby>は<ruby>各<rt>かく</rt></ruby>domainが<ruby>定義<rt>ていぎ</rt></ruby>する。

Doc<ruby>内<rt>ない</rt></ruby>のMathとMath<ruby>内<rt>ない</rt></ruby>のDocは、suiteのbridgeが<ruby>処理<rt>しょり</rt></ruby>する。doc\-coreはmath\-coreをimportしない。<ruby>出力<rt>しゅつりょく</rt></ruby>backend<ruby>同士<rt>どうし</rt></ruby>も、<ruby>相互<rt>そうご</rt></ruby>にimportしない。suiteが<ruby>依存関係<rt>いぞんかんけい</rt></ruby>に<ruby>従<rt>したが</rt></ruby>って<ruby>埋<rt>う</rt></ruby>め<ruby>込<rt>こ</rt></ruby>みを<ruby>準備<rt>じゅんび</rt></ruby>し、backendsへ<ruby>型付<rt>かたつ</rt></ruby>きの<ruby>解決済<rt>かいけつず</rt></ruby>みfragmentを<ruby>渡<rt>わた</rt></ruby>す。

ui\-coreは、<ruby>純粋<rt>じゅんすい</rt></ruby>なModel・Msg・update・viewと、commandおよびsubscriptionの<ruby>記述<rt>きじゅつ</rt></ruby>を<ruby>所有<rt>しょゆう</rt></ruby>する。suiteやDOMを<ruby>実行<rt>じっこう</rt></ruby>してはならない。<ruby>実行<rt>じっこう</rt></ruby>・Worker・editor widgetは、host adapterが<ruby>担当<rt>たんとう</rt></ruby>する。

<ruby>目標<rt>もくひょう</rt></ruby>は20 crateで、そのうち15 crateが `no_std + alloc` である。<ruby>実装済<rt>じっそうず</rt></ruby>みのworkspace memberは、ルートの `Cargo.toml` に<ruby>記載<rt>きさい</rt></ruby>する。

<ruby>文書<rt>ぶんしょ</rt></ruby>とsiteの<ruby>生成<rt>せいせい</rt></ruby>は、toolsの<ruby>明示的<rt>めいじてき</rt></ruby>な<ruby>段階<rt>だんかい</rt></ruby>で<ruby>行<rt>おこな</rt></ruby>う。`build.rs` を<ruby>通<rt>とお</rt></ruby>して、compilerと<ruby>文書<rt>ぶんしょ</rt></ruby>rendererを<ruby>循環依存<rt>じゅんかんいぞん</rt></ruby>させてはならない。

<a name="n-626f6f747374726170"></a>

<a name="3-surface-descriptorとbootstrap"></a>

## 3\. surface descriptorとGrammarのbootstrap

`languages/*/syntax.neplg` は、<ruby>各言語<rt>かくげんご</rt></ruby>のsurface<ruby>定義<rt>ていぎ</rt></ruby>である。Grammar compilerがpackageを<ruby>生成<rt>せいせい</rt></ruby>し、その<ruby>生成済<rt>せいせいず</rt></ruby>みpackageもcommitする。<ruby>通常<rt>つうじょう</rt></ruby>のbuildで、toolsやgrammar compilerをbuild dependencyにしてはならない。`cargo run -p nepl3-tools -- generate --check` により、sourceとの<ruby>一致<rt>いっち</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。

Grammar<ruby>自身<rt>じしん</rt></ruby>の<ruby>初期<rt>しょき</rt></ruby>packageは、<ruby>同一<rt>どういつ</rt></ruby>のGrammar syntaxを<ruby>表<rt>あらわ</rt></ruby>す<ruby>検査済<rt>けんさず</rt></ruby>みseedとする。<ruby>別<rt>べつ</rt></ruby>の<ruby>小<rt>ちい</rt></ruby>さなGrammar dialectは<ruby>作<rt>つく</rt></ruby>らない。seedで<ruby>自身<rt>じしん</rt></ruby>のsourceを<ruby>読<rt>よ</rt></ruby>み、compileしたdescriptorの<ruby>意味正規形<rt>いみせいきけい</rt></ruby>とseedを<ruby>比較<rt>ひかく</rt></ruby>する。これは<ruby>自己<rt>じこ</rt></ruby>ホスト<ruby>前<rt>まえ</rt></ruby>の<ruby>永続的<rt>えいぞくてき</rt></ruby>なbootstrap<ruby>契約<rt>けいやく</rt></ruby>であり、<ruby>将来<rt>しょうらい</rt></ruby>の<ruby>実装<rt>じっそう</rt></ruby>でも<ruby>同<rt>おな</rt></ruby>じseedを<ruby>使<rt>つか</rt></ruby>える。

<a name="n-72757374"></a>

<a name="4-rust設定"></a>

## 4\. Rust<ruby>設定<rt>せってい</rt></ruby>

workspaceはedition 2024、resolver 3とする。MSRVは1\.85\.0<ruby>以上<rt>いじょう</rt></ruby>とし、<ruby>実装開始時<rt>じっそうかいしじ</rt></ruby>に<ruby>使<rt>つか</rt></ruby>う<ruby>一<rt>ひと</rt></ruby>つのstable toolchainを `rust-toolchain.toml` に<ruby>完全<rt>かんぜん</rt></ruby>な<ruby>版番号<rt>はんばんごう</rt></ruby>で<ruby>固定<rt>こてい</rt></ruby>する。MSRVと<ruby>開発<rt>かいはつ</rt></ruby>toolchainは<ruby>同一視<rt>どういつし</rt></ruby>しない。<ruby>外部依存<rt>がいぶいぞん</rt></ruby>はworkspace\.dependenciesで<ruby>集中管理<rt>しゅうちゅうかんり</rt></ruby>し、Cargo\.lockをcommitする。

core<ruby>系<rt>けい</rt></ruby>crateは<ruby>常<rt>つね</rt></ruby>に `#![no_std]` とし、allocを<ruby>使<rt>つか</rt></ruby>う。coreをstd<ruby>化<rt>か</rt></ruby>するfeatureは<ruby>設<rt>もう</rt></ruby>けない。<ruby>標準<rt>ひょうじゅん</rt></ruby>ライブラリが<ruby>必要<rt>ひつよう</rt></ruby>なアダプタはappsへ<ruby>置<rt>お</rt></ruby>く。native\-only crateがcoreに<ruby>入<rt>はい</rt></ruby>り<ruby>込<rt>こ</rt></ruby>むことを、cargo metadataとtarget buildによって<ruby>検出<rt>けんしゅつ</rt></ruby>する。

math\-coreは `num-bigint` \/ `num-rational` \/ `num-integer` \/ `num-traits` を、default\-features\=falseで<ruby>利用<rt>りよう</rt></ruby>してよい。<ruby>対応<rt>たいおう</rt></ruby>する<ruby>互換<rt>ごかん</rt></ruby>version<ruby>集合<rt>しゅうごう</rt></ruby>を<ruby>一度<rt>いちど</rt></ruby>resolveし、lockする。<ruby>異<rt>こと</rt></ruby>なるBigIntの<ruby>版<rt>はん</rt></ruby>を、<ruby>言語間<rt>げんごかん</rt></ruby>で<ruby>露出<rt>ろしゅつ</rt></ruby>させてはならない。<ruby>公開<rt>こうかい</rt></ruby>する<ruby>意味値<rt>いみち</rt></ruby>には、この<ruby>仕様<rt>しよう</rt></ruby>のcanonical integer・rational<ruby>型<rt>がた</rt></ruby>を<ruby>使<rt>つか</rt></ruby>う。

<ruby>安全<rt>あんぜん</rt></ruby>なRustを<ruby>原則<rt>げんそく</rt></ruby>とする。FFIや<ruby>高速化<rt>こうそくか</rt></ruby>のためにunsafeが<ruby>必要<rt>ひつよう</rt></ruby>なら、<ruby>該当<rt>がいとう</rt></ruby>adapterの<ruby>内部<rt>ないぶ</rt></ruby>へ<ruby>局在<rt>きょくざい</rt></ruby>させ、unsafe<ruby>契約<rt>けいやく</rt></ruby>と<ruby>試験<rt>しけん</rt></ruby>を<ruby>添<rt>そ</rt></ruby>える。domainからpointerを<ruby>外部<rt>がいぶ</rt></ruby>へ<ruby>公開<rt>こうかい</rt></ruby>してはならない。

<a name="n-616c6c6f7765645f646570656e64656e63696573"></a>

<a name="5-許可集合の検査"></a>

## 5\. <ruby>許可集合<rt>きょかしゅうごう</rt></ruby>の<ruby>検査<rt>けんさ</rt></ruby>

dependency checkerはproduction<ruby>依存<rt>いぞん</rt></ruby>とbuild<ruby>依存<rt>いぞん</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>し、dev<ruby>依存<rt>いぞん</rt></ruby>を<ruby>別<rt>べつ</rt></ruby>の<ruby>集合<rt>しゅうごう</rt></ruby>として<ruby>出力<rt>しゅつりょく</rt></ruby>する。productionからappsやtoolsへの<ruby>経路<rt>けいろ</rt></ruby>、domain core<ruby>同士<rt>どうし</rt></ruby>の<ruby>経路<rt>けいろ</rt></ruby>、<ruby>閉路<rt>へいろ</rt></ruby>、std<ruby>依存<rt>いぞん</rt></ruby>を<ruby>検出<rt>けんしゅつ</rt></ruby>する。<ruby>外部<rt>がいぶ</rt></ruby>crateの<ruby>採用<rt>さいよう</rt></ruby>によって、coreへOS<ruby>機能<rt>きのう</rt></ruby>が<ruby>混入<rt>こんにゅう</rt></ruby>しないことを<ruby>確認<rt>かくにん</rt></ruby>する。

<ruby>単一<rt>たんいつ</rt></ruby>repositoryであっても、<ruby>各<rt>かく</rt></ruby>coreは<ruby>独立<rt>どくりつ</rt></ruby>して `check --no-default-features` できるものとする。<ruby>将来<rt>しょうらい</rt></ruby>のrepository<ruby>分割<rt>ぶんかつ</rt></ruby>を、<ruby>現在<rt>げんざい</rt></ruby>の<ruby>依存環境<rt>いぞんかんきょう</rt></ruby>へ<ruby>強制<rt>きょうせい</rt></ruby>しない。<ruby>公開<rt>こうかい</rt></ruby>schemaと<ruby>操作<rt>そうさ</rt></ruby>の<ruby>単位<rt>たんい</rt></ruby>で、<ruby>実装<rt>じっそう</rt></ruby>を<ruby>交換<rt>こうかん</rt></ruby>できるようにする。

<ruby>外部言語<rt>がいぶげんご</rt></ruby>の<ruby>追加<rt>ついか</rt></ruby>でfoundation sourceを<ruby>変更<rt>へんこう</rt></ruby>しない<ruby>条件<rt>じょうけん</rt></ruby>と、repository<ruby>分離前<rt>ぶんりまえ</rt></ruby>の<ruby>実証<rt>じっしょう</rt></ruby>は [<ruby>外部拡張契約<rt>がいぶかくちょうけいやく</rt></ruby>](<22\-external\-extensions\.md>) に<ruby>従<rt>したが</rt></ruby>う。<ruby>現時点<rt>げんじてん</rt></ruby>ではmonorepoを<ruby>維持<rt>いじ</rt></ruby>し、<ruby>独立<rt>どくりつ</rt></ruby>workspaceの<ruby>公開<rt>こうかい</rt></ruby>API<ruby>利用<rt>りよう</rt></ruby>から、<ruby>配布<rt>はいふ</rt></ruby>・provider・<ruby>互換性試験<rt>ごかんせいしけん</rt></ruby>へ<ruby>進<rt>すす</rt></ruby>む。

<a name="n-63757272656e745f7374616765"></a>

<a name="現在の作業段階"></a>

## <ruby>現在<rt>げんざい</rt></ruby>の<ruby>作業段階<rt>さぎょうだんかい</rt></ruby>

<ruby>以上<rt>いじょう</rt></ruby>は、<ruby>目標<rt>もくひょう</rt></ruby>とする<ruby>構成<rt>こうせい</rt></ruby>である。<ruby>現在<rt>げんざい</rt></ruby>のworkspace memberはルートCargo\.tomlを、<ruby>実装状態<rt>じっそうじょうたい</rt></ruby>はimplementation\-status\.jsonを<ruby>正本<rt>せいほん</rt></ruby>とする。crateは、その<ruby>責務<rt>せきむ</rt></ruby>を<ruby>実装<rt>じっそう</rt></ruby>する<ruby>段階<rt>だんかい</rt></ruby>で<ruby>追加<rt>ついか</rt></ruby>する。<ruby>開発<rt>かいはつ</rt></ruby>toolchainとMSRVの<ruby>選定理由<rt>せんていりゆう</rt></ruby>はdoc\/development\.mdを、リポジトリ<ruby>整備<rt>せいび</rt></ruby>の<ruby>判断<rt>はんだん</rt></ruby>はdoc\/decisions\/0001\-repository\-foundation\.mdを<ruby>参照<rt>さんしょう</rt></ruby>する。
