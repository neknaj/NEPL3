# GitHub Pages の情報設計と安定 URL

Status: planned

この文書は、NEPL3 ecosystem の GitHub Pages で公開する主要 surface の安定 URL と、その役割を定める。Pages は NEPL3d 等の正本から得られる公開 projection と interactive surface の配布先であり、HTML 自体を正式文書の正本とはしない。

## 1. 安定 URL

標準 origin は `https://neknaj.github.io/NEPL3/` とし、NEPL3 本体 repository の Pages では次のトップレベル path を長期に安定させる。

| path | 役割 |
|---|---|
| `/NEPL3/` | NEPL3 ecosystem と共通基盤の入口 |
| `/NEPL3/playground/` | NEPL3 共通基盤と言語 composition を対話的に試す Playground |
| `/NEPL3/tutorial/` | NEPL3 の共通原理と言語作成・composition を段階的に学ぶ Tutorial |
| `/NEPL3/docs/` | NEPL3 共通基盤の正式文書・仕様・設計の公開 projection |
| `/NEPL3/api/rust/` | 現 NEPL3 repository の Rust API reference。利用者向け Tutorial/Docs とは分離する |

`playground`、`tutorial`、`docs` は NEPL3d、NEPL3c、NEPL3h 等の個別 guest language の所有物としない。NEPL3 本体 repository は、共通 infrastructure、language composition、共通 protocol/schema、ecosystem の接続方法を主対象とする。

既存計画の `/NEPL3/docs/tutorials/` は公開 URL として固定せず、Tutorial の安定入口は `/NEPL3/tutorial/` とする。実装前の planned route であるため、公開互換性のための恒久 alias とは扱わない。公開後に route を変更する場合は、安定 page ID と redirect/alias を明示して silent break を避ける。

## 2. repository 分割後の文書所有

NEPL3d、NEPL3c、NEPL3h、NEPL3g 等が独立 repository へ分割された後、各言語固有の詳細 reference、完全な syntax/semantics reference、言語固有 tutorial、言語固有 API/CLI、言語固有 examples は各 repository が所有する。

NEPL3 本体 repository の Pages に各言語の詳細 reference を複製して集約しない。NEPL3 本体側には、各 guest language の存在、役割、共通基盤との接続点、互換性・package identity、対応 repository への安定 link など ecosystem-level の情報だけを置く。

repository 分割前に同一 repository 内へ存在する言語固有文書についても、将来の所有者を意識し、NEPL3 本体の共通文書と混同しない。分割時に URL を維持する必要がある公開済み entry point は明示的 redirect/alias で移行する。

## 3. Docs の位置付け

正式な文書の正本は最終的に NEPL3d source とする。GitHub Pages 上の `/docs/` 以下の HTML は NEPL3d renderer が生成する公開 projection であり、HTML を別の正本として手で保守しない。

Markdown から NEPL3d への移行期間は、既存 Markdown 正本と移行済み NEPL3d 正本を同じ安定 page ID / public route に投影できるようにする。移行の目的は HTML site 自体の高度化ではなく、NEPL3d が実際の仕様・設計文書を損失なく表現し、NEPL3 ecosystem の正式文書言語として利用できることを成立させることである。

HTML/browser 側の検査は、安全な出力、主要 route/link、asset の成立、主要 browser での閲覧可能性など公開に必要な範囲を扱う。rustdoc や最終 HTML の完全閉包検査を、NEPL3d 本体・文書移行の critical path に置かない。

## 4. Playground の立場

Playground は NEPL3h、NEPL3c、NEPL3d 等の特定 guest language の専用 IDE としない。NEPL3 の共通基盤を直接観察・試験できる中立的な surface とする。

Playground では少なくとも次を扱えるようにする。

- 共通の括弧なし前置記法と arity/context による構文確定を観察する。
- 小さな LanguagePackage / grammar を定義し、最小の NEPL3 guest language を作る。
- 作成した言語を parse/check/print し、構文・診断・Source/Origin 等の共通機構を確認する。
- 別 LanguagePackage を import/composition し、一つの source 内で複数言語を統合する。
- `annotation` のような基礎的な別言語を import し、自作 guest language の syntax に対象付き注釈を組み込む。
- `sentence` のように多数の言語から共有されることを意図した基礎言語を利用する。

個別 guest language は example/profile として Playground から選択できるが、Playground の navigation・概念モデル・操作契約そのものを一つの guest language の semantics に合わせない。

## 5. Tutorial の学習目標

Tutorial も特定 guest language の入門書を中心にしない。NEPL3 の共通原理から始め、利用者自身が言語を構成し、他言語を composition するところまでを基本課程とする。

初期 Tutorial は概ね次の順序を想定する。

1. NEPL3 の目的: 多数の異なる言語を、共通の括弧なし前置記法と共通 infrastructure の上で組み合わせる。
2. token、head、arity、context、prefix parse 等、言語非依存の構文確定原理を Playground で確認する。
3. 最小の guest language を定義し、literal と少数の prefix form を parse/check/print できるようにする。
4. LanguagePackage、schema、operation/provider、Profile 等、他言語と composition するための共通契約を確認する。
5. `annotation` を別言語として import し、自作言語に対象付き注釈を組み込む。annotation を自作言語の特別構文として再実装しない。
6. `sentence` 等の基礎言語を必要に応じて composition し、複数言語が一つの source / document / structure 内で共存する例を作る。
7. Source/Origin、diagnostic、binding、portable representation など、composition 後も共通 infrastructure が維持する情報を確認する。
8. 実在する個別 guest language は共通原理の応用例として軽く扱い、詳細学習は各言語 repository の文書へ接続する。

この Tutorial の到達点は、既存の一言語を使えるようになることだけではなく、NEPL3 上で小さな言語を作り、独立した別言語を import して統合できることとする。

## 6. 基礎言語の扱い

`annotation`、`sentence` など、多数の guest language から利用されることを意図する基礎言語は、個別 domain language より広い共通教材として Playground/Tutorial から利用可能にする。

ただし、それらを NEPL3 core に暗黙統合された特別構文として扱わない。Tutorial でも「別 LanguagePackage を composition する」という NEPL3 の通常の仕組みを見せる。これにより、NEPL3 の共通基盤が特定 language の syntax/semantics を内蔵しなくても language integration を成立させられることを教材自体で検証する。

## 7. URL 安定性の原則

- public route は表示名や repository 内 source path から自動生成しない。
- page / tutorial lesson / playground preset には安定 ID を与える。
- source file の移動・正本形式の Markdown から NEPL3d への変更で public URL を変更しない。
- Playground の内部状態は shareable な deep link を持てるようにするが、トップレベル `/playground/` は固定する。
- Tutorial の lesson URL も安定 ID を基準にし、見出し変更だけで URL を破壊しない。
- repository 分割に伴い個別 guest language の詳細文書を移動する場合は、既に公開済みの入口について明示的な移行先を定める。

この URL 設計は Pages 実装を先行させるためのものではない。NEPL3d 文書移行、Playground、Tutorial、各 repository 分割が進んでも公開 path を不用意に変更しないため、先に namespace を予約する計画である。
