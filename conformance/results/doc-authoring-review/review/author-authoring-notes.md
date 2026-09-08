# 執筆担当Bの記録（独立レビュー前）

- 基準commit: `bc1d5d3443d214457aa6527250d372ee1f2b5896`
- branch: `docs/authoring-languages`。開始時working treeはclean。
- AGENTS.md、doc/authoring.md、doc-signatures.md、各原文を通読して手作業で執筆した。機械変換器で原稿を生成していない。
- Markdown正本・runtime・schema・registry・toolsは変更していない。
- 通常本文は自然な一文単位のSentence literal。inline codeとリンクの文は明示sentence。コードblockはRawCodeで非評価表示。翻訳対がないためParallelは追加していない。
- Rubyは漢字部分のみ。Math章の自由記号/free symbolは原文にある同義語をAnnoへまとめた。訳文を新設していない。
- 相対リンクは原文の論理的なdoc/spec位置を基準としたURIを保持。authored配置そのものからリンクを自動再解決する正本切替えは未実施であり、公開registry側で元の論理位置と対応付ける必要がある。
- 構造auditは5章すべて成功。これは外側prefix構造だけで、実parser/lower/HTML成功ではない。rootが実CLIの資源計量上の障害を修正中であり、上限を理由に表現を削っていない。
- scratchの独立したMistune ASTとDoc構造audit ASTによる本文投影照合で、各原文paragraph/heading/table cell/codeblockの内容が順序どおりの本文に残ることを自己確認した（空白は比較時のみ無視し、09の(1)(2)(3)は構造的ordered listの番号として扱う）。この自己確認を独立レビューや意味同等性の正式受入にはしない。

## 05-document

- 元SHA-256: `74beb5cb2ed1cd7b002fd5bf2e76597f540aa5109f1971145ced9128b5c96c9a`
- 見出し対応: 方針→policy、1→types、1.1→arena、1.2→elements、1.3→inspection、2→literal、2.1→linebreak、3→equivalence、4→parallel、5→labels、6→embedding、7→html、8→public、8.1→plaintext、8.2→print。
- EBNFとprefix例のコード内容をRawCodeへ保持。literal例やdelimiterはInlineCodeであり、本文として再解釈しない。
- snapshot/digest domain、32byte/64hex/20entry/各wrapper数、DG監査件数、proofの限界、停止とUsage、source閉包、PlainText/Printの失敗を保持。
- 17章へのリンクを保持。現時点の検査・移行完了を主張しない。

## 06-math

- 元SHA-256: `c22ab7b0fae9acb9edecd3886fb641cefcf53c9b688dc8c5ffe7cba97eebd028`
- 見出し対応: 方針→policy、1→expression、2→structure、3→evaluate、4→mathml、4.1→arena、5→resources、6→public。
- 13項の演算リストを保持。0^0、負指数、完全冪、空の総和、形状エラー、Symbolic分類を保持。
- 原文の「自由記号を許可し、free symbolは…」は同義語をAnnoにまとめ、許可と非エラーの二文へ書き直した。
- Number有限十進とRationalの定義域を混同せず、表示順・binding power・29form/3root・元source・17章リンクを保持。

## 07-circuit

- 元SHA-256: `0005924591477574e490affa8df48d7446dfb41d81cd08beb55d03372a0dbfed`
- 見出し対応: 方針→policy、1→surface、2→declarations、3→operations、4→elaboration、5→time、6→nor、7→svg、8→public。
- 宣言のまとまりを5項のunordered list、検査手順を6項のordered listへ構造化。
- 幅条件、前方参照、独立instance、同時state更新、pre-edge比較、4node/2sink、NOR等式とcarry、決定的SVG配置を保持。
- 数式の波括弧はliteral escapeで文字として保存。新しいMath評価や図を加えていない。

## 08-editor

- 元SHA-256: `1311cb50ce019ed53c733f9d8e414a7ea42b70f975fb0d60d01314762d79fa39`
- 見出し対応: 方針→policy、1→snapshot、2→derived、3→region、4→relations、5→recovery、6→incremental、7→lsp、8→diagnostics、9→trust。
- AnalysisKeyの全identity、sidecar None/Some、priority/depth/順序、ownerごとのmap、RegionQueryとOccurrence入口、rename二段操作とproofの条件を保持。
- encoding/LSP3.17、trust、stale排除、非成功の途中候補禁止、native費用とserialization費用の境界を保持。

## 09-portability

- 元SHA-256: `6a946ba4c8d01e8fbe022e641739f1a3d762ce94ac60b45e5cc37e64274683ca`
- 見出し対応: 方針→policy、1→layers、2→ndf、2.1→intrinsic、2.2→references、2.3→descriptor、3→invocation、3.1→transform、4→native、5→process、6→replacement。
- 三層をordered list化。NDF全12tagの3列表、Transform3結果の3列表をDoc Tableに保持。
- NDF arrayはliteralの注釈へ誤解釈させずInlineCodeで表示。
- 27操作の歴史的な未完成記述、128階層checker上限、8byte big-endian frame、実装記録リンク、非認証/proof境界を原文どおり保持。仕様の時点更新はこの執筆変更では行わない。

## 未確認

- 実production parser/lower/HTMLはrootへ依頼中。
- 独立レビュー、legacy anchor写像、公開Pagesでのリンク、正本切替え、正式T21受入は未完了。

## 追加担当10〜13章

- 着手commit `50c6fde11fa2c5ab62edaf7c8edecdef4d25ae5e`、原文固定commitは引き続き `bc1d5d3443d214457aa6527250d372ee1f2b5896`。
- 未レビューcheckpointを `e0d71af`（10章）、`6e05e99`（11/12章）、`33eb2e96ec54da387e3b0ea8b730b0b3f228474f`（13章と12章の正式訂正反映）でpush。
- すべて手動執筆。内容の列挙はlist/table、本文は自然な一文単位のliteral、コードとリンクの文は明示sentence。資源停止を理由に削減しない。
- 5章の実CLI結果はrootから報告あり: 06はparse/lower成功し17章リンクのNeedsResolution、05はparse AllocationLimit、07/09はtree-validation NodeLimit、08はparse NodeLimit。rootが会計・重複検査を改善中。候補を実行可能/正式移行済みとは報告しない。

### 10-integration

- 原文working tree byte SHA-256: `042382308ccbbd2deb615983aaed9906c17571c58aed5e4e11494d67cd4b717d`
- 見出し: 方針→policy、1→profile、2→bridges、3→recursion、4→environment、5→artifacts、6→cli、7→hosts。
- 標準bridgeの5行4列表、10コマンド、終了code0〜4、full Profile未完成との区別、Limits上限、alias別context、17章リンクを保持。

### 11-conformance

- 原文working tree byte SHA-256: `e8b3e4e8da02f3311541fef79121ed1b7c2cab7258dc7f2c38b2c65241426b2b`
- 見出し: 方針→policy、1→required、2→properties、3→ci、4→stages、5→evidence。
- G/P/D/M/C/E/W/A/U/S/Jの全55 IDを順序どおり保持し、群ごとのlistへ構造化。
- S06失敗系の一文中列挙(a)〜(i)は、導入文「次の場合を含める」と9 list itemへ書き直した。条項の意味・順序・後続3条件は維持。
- TaskEvidence/AcceptanceEvidence、required target、run種別、log検査、証拠の限界、未実行とfailedの区別を保持。17章リンクあり。

### 12-model-invariants

- 基準原文working tree byte SHA-256: `9ff06dc714c2c789ae6c512a099d96dcd3a789b5d0ec190b42d3f32df5c687f4`
- 見出し: 方針→policy、1→values、1.1→constraints、2→checked、3→circuit、4→markup、5→operations。
- foundation制約ID17行2列表、局所ID/Foreign/Origin/map/Stoppedの境界、回路node数とbit順を保持。
- 原文4節の旧HTML許可集合（表/list/imgなし、external href禁止）をrootへ報告した。rootが正式仕様を `5657677e9b4a1cc44830b9a365206822920983fe` で訂正。原稿にもその2段落の差分を手動で取り込んだ。
- 訂正済み原文Git blob（LF）SHA-256: `81b22608f80254f7d5979b754436b9d62da0bf25aefddf01a6fec772fdcf9cc5`、保持先 `.tmp/12-model-invariants-corrected.md`。
- 新たに19章への正式リンクを保持。型付きhref4variant、内部target/route照合、HTML imgのartifact内pathと別のasset解決、SVG image/任意style等の禁止を明示。執筆者独断で旧規範を削ったものではない。

### 13-reproducibility

- 原文working tree byte SHA-256: `81d032e6f04730fd4c52a15c11464bff0ed879c1030a059be19d10b4d8c9a16d`
- 見出し: 方針→policy、1→digest、Package意味正規形→semantic、具体実行identity→execution、2→wire、3→artifacts、4→limits。
- digest domain、canonical JSONの文字規則、全tuple field順、明示NodeRef再採番と他ID空間、環境内容digestの限界、HTML/XML escapeのすべてを保持。
- RFC3986§3.1とRFC3987の外部リンクを保持。escape表記はInlineCodeで二重に解釈しない。

### 追加4章の検査範囲

- 外側構造audit: 全4章成功。
- scratchの原文block本文照合: 10/13は全一致。11は記録済みS06列挙の構造化のみ本文差。12は正式訂正済みblobで再照合する。
- 本文比較は空白無視の自己確認であり、実rendererや独立レビューの代替ではない。
- 10〜13のproduction parser/lower/HTML、相対リンクregistry、独立レビューはrootへ引き継ぐ。

## 14-web-ui checkpoint
Base: bc1d5d3443d214457aa6527250d372ee1f2b5896; source SHA256 276e2eef46c0ee67adc3cae08fcc431630abc50489de9d19844ab40aae7ed22a
Seven source sections retained, five operation rows plus header; original links and inline code retained. Manual prose editing and sentence boundaries; outer structure audit PASS. Unreviewed, production HTML not executed. Prefix Text Ruby coverage remains to finish; checkpoint is not completion. Pausing 15-17 to repair independent review Ruby findings in 05-09; 10-13 need same selfcheck.

## 06 Ruby correction
All narrative literal and prefix Text kanji annotated manually, including relative-link label. Code strings and target preserved. .tmp/check-ruby-06.py: outer structure PASS, pre/post whole base-text exact equality PASS, unannotated narrative kanji search empty. Independent re-review requested; production export remains root-owned.

## 07 Ruby correction
All remaining prefix prose kanji annotated manually. Corrected readings for syntax-tree and revision. .tmp/check-ruby-07.py: outer structure PASS, pre/post whole base-text exact equality PASS, unannotated narrative kanji search empty. 05/08/09 and 10-14 still require further corrections.

## Ruby corrections and source supplement 2026-09-09

- 08-editor: 8ef40fd; 09-portability: 39b49af; 05-document: f840e51.
- 10-integration: e3a4de2; 11-conformance: ebac378; 12-model-invariants: 9dd5a5c; 13-reproducibility: 7e9e52c; 14-web-ui: d50f168.
- Each correction was manually authored and compared to 24d7eac with the scratch outer Parser and Ruby-elided text projection. Base text matched exactly and no unannotated narrative kanji remained. This is author self-check, not runtime or independent review evidence.
- 05 new source supplement: doc/spec/05-document.md at main 90f4552d9d8de662bada4d76c688c8cf30afa595, SHA256 c6b0878316fec5554402ef0f0c2f5edb66ffd36bb363e17e2033b77fb53f27be. Four new source paragraphs under section 3 were manually rewritten as authored sentences. SentencePayload field order, owner source closure, head constraints, mapped DocumentSyntax compatibility, explicit sentence-v2 signature, failure/no-retry policy retained.
- check-sentence-payload-addition.py compares all four new paragraphs against the source Markdown projection and verifies removing them leaves exactly the prior draft text at d50f168. Source bytes and manifest stored in .tmp/05-sentence-payload-source.md/json. Runtime and independent review remain separate.
# Guide authoring checkpoints

References: 9fa716d uses frozen337057d doc/spec/references.md source SHA c9ba86ece67d1bfe46b420d8b91b2dce95eb6e57a6b63d8c14e17534cafe9217. Eighteen bare source URLs are explicit External Link with identical display bytes; scratch comparison enables URL recognition, not current network reachability. Original acquisition date and unprovided Library limits retained.

Review work in progress: 31e150e is only a prefix of doc/review.md (source full SHA 5e69673b46bc5893b0d77f9833b34b34f8251e1243ee412a1438f48619727b6a at337057d), through the complete first r4 foundation review section. Next source heading: 組込みreaderとSourceMap接続の追加レビュー. No remaining history removed or declared complete; later paragraphs are not yet authored. Prefix fulltext, table cell order, exact inline code, 21 link targets and Ruby self-check pass using .tmp/check-guide.py review 337057d with the next heading as boundary. Evidence .tmp/guide-review-checks.json explicitly records prefix scope; production/independent/runtime checks remain unrun. Draft SHA e981e73738d40494b7ea9377cdb8a6eaa37d6fae890b94eec23f94a3d061ed96.

Follow-up source correction: 3887fff applies only the root-owned development paragraph from 337057d71838c85a6fb9a304e3ec4164436bb310, compared against the original 90f4552 base. Corrected source SHA-256 f51f37998c2e606f3788ce892ef1f34f39b46e8a7533fbce5986087e6c634927, draft eb3fefe2e22ce0116a9827d08e724569e57efb186a8e7051a36b21e2e3287bbe. Full base text/code/link/Ruby self-checks pass; no other draft content changes.

Inventory draft: 074af5f, frozen source 337057d doc/doc-inventory.md SHA-256 13637103c1e5581238cc68a099f6e28553fbee21cc0bfd33fe557ca461093264, draft 8bb9b482152688083a0340322b5b131c59a7e1a84165d1e282821ae89ea3a821. Five original sections, four tables (classification, observations, projection variants, DG01-DG09), one exact raw code block and three exact link targets. Baseline claims are historical scope, not current production status. Remaining source Markdown documents under doc/ include decisions (six ADR), progress (three reports), history/README, migration/README, review, spec/references and four signature catalogs. These require separate authorship/derived-source classification and owner allocation; do not duplicate another author's draft or transform frozen JSON history. Raw source and check manifest saved under .tmp/guide-doc-inventory-*.

All three guide originals are frozen at 90f4552d9d8de662bada4d76c688c8cf30afa595.

- README: ca6081a, source SHA-256 4e87bdd9154599be5b364755da38e5905c15c4cc87255802a06ed34b3d4fc978. Reading order, status distinction, source tables and 59 link targets retained.
- authoring: 98eb0c2, source SHA-256 0659615a44794990f1c6356a660516e4940f8e8efe272903aff349907deeae08. All six sections retained. Four raw code blocks preserve hint and exact decoded bytes; inline code values and six link targets retain order. The fragment-only Japanese target and matching section Name are preserved. The scratch outer audit whitelists this exact Name because the repository audit only recognizes ASCII, and does not claim production Unicode XID or anchor validation.
- development: 446000e, source SHA-256 9fd123a202fb3700af89aaaa46a6ef34b17a38312b1f10492ee4bfab8322ece2. Portable execution, local tools, page candidates, distribution and review sections retained. Seven raw code blocks retain hints and exact bytes, inline code and 19 link targets retain order. Parent confirmed the old product/CI runner paragraph needs a source correction; original meaning is retained pending that separately owned correction.

Each draft was authored manually. Scratch comparison .tmp/check-guide.py verifies whitespace-normalized full Markdown base text, code values, link targets (URI-decoded for the Japanese Markdown fragment) and no unannotated narrative kanji. It is a self-check, not independent review or production parse/lower/HTML/target resolution. Raw frozen source and hash reports are .tmp/guide-{README,authoring,development}-source.md and corresponding -checks.json. Repository originals remain canonical; these are candidates only.

## Review and references checkpoint 6867500

Frozen source commit: 337057d71838c85a6fb9a304e3ec4164436bb310. References completed at 9fa716d; source SHA c9ba86ece67d1bfe46b420d8b91b2dce95eb6e57a6b63d8c14e17534cafe9217 and draft SHA c1dfe540bd87470fd3b301f9a864fc9496e4eb03f2954a148590c727234f06d1. Its eighteen historical reference URLs are preserved, with no claim of current external verification.

Review at checkpoint 6867500 was PARTIAL. It covered only the prefix before the paragraph beginning Facts operation envelope (original line 311). That checkpoint does not cover subsequent paragraphs.

## Review full draft, 2026-09-08

All 527 original lines from fixed 337057d71838c85a6fb9a304e3ec4164436bb310 now have a manually authored draft. Full original SHA-256 is 5e69673b46bc5893b0d77f9833b34b34f8251e1243ee412a1438f48619727b6a; full draft SHA-256 is d4d987c22c0549495f1646d83e7bf13e636b9cd89c646fff7b689324585e177d. Main headings, four tables, historical findings and their bounded before/after experiments, numeric counts and limits, source hashes and path references, retained limitations and final Doc preparation section are included. No current verification of historical claims or external URLs was performed.

Self-check command: python .tmp/check-guide.py review 337057d. Entire whitespace-normalized base text, exact inline code values, 32 link targets and order, and four tables with cell order agree. Outer grammar audit and narrative kanji annotation check passed; git diff --check passed. Evidence: .tmp/guide-review-source.md and .tmp/guide-review-checks.json. These are author self-checks only: independent review, production parse/lower/HTML, target resolution, visual review and canonical-source migration remain unclaimed. Canonical Markdown and shared runtime/schema/registry were not edited.
