# 外部workspaceでの検証

`distribution.py`は、管理対象のsourceから独立したCargo workspaceを抽出する。
toolchain、license、workspace設定、依存するpackageのlock記録を保持する。
出力先には、存在しないディレクトリを指定する。

## Foundation

```sh
python -m pip install -r tools/extensions/requirements.txt
python tools/extensions/distribution.py /tmp/nepl3-foundation
cargo test --locked --manifest-path /tmp/nepl3-foundation/Cargo.toml --workspace
```

抽出対象はcore、reader、engine、wireの4 crateである。
`run.py --distribution`は、抽出したFoundationに接続する外部Hello consumerも検査する。

## Docの抽出実証

```sh
python tools/extensions/distribution.py /tmp/nepl3-doc --doc /tmp/nepl3-foundation
cargo test --locked --manifest-path /tmp/nepl3-doc/Cargo.toml --workspace
```

Windowsでは、上記の`/tmp/...`をリポジトリ外の絶対pathへ変更する。
Doc core、Doc HTML、markupの3 crateと、`languages/doc/syntax.neplg`を抽出する。
crateに属する試験とHTMLのCSSも含む。Foundationの4 crateは先に抽出したworkspaceを参照する。
Foundationのファイル集合と内容は、抽出元の同一revisionに一致することを要求する。
Cargo metadataでローカル依存のmanifestとtarget sourceの配置を検査し、lock記録の変更を拒否する。

この手順の検証範囲は、Docの意味モデル・検査・出力crateとその試験の独立buildである。
通常のNEPL3d入力を扱う開発hostのSentence reader、Math composition、公開サイト生成の接続は、後続の抽出範囲である。
Sentenceとannotationの所有移行、独立版管理の互換性、X01・X02の正式受入も継続して検証する。

WASIでの実行には、固定toolchainの`wasm32-wasip2`とWasmtime runnerを設定し、
上記の`cargo test`へ`--target wasm32-wasip2 -- --test-threads=1`を追加する。
必要な環境設定は[開発手順](../../doc/development.md)に従う。
