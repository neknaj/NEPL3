# 18. HTMLを先行利用する実装段階

2026-09-07の追加指示により、Doc入力からHTMLまでを先行して利用可能にする。基準として提示された933f0bbへ戻さず、既存lower::document・labels::check・plain_text・source printerを再利用する。Mathのarena/shape/lower/codecは進行中の表示用実装を利用する。source printerをHTML rendererへ読み替えない。

## 実装依存

| 正式タスク | 段階 | 成果物と終了範囲 |
| --- | --- | --- |
| T22 | H0 | Doc用markup、文書check/prepare、URI/asset、renderer、17章の型・codec・制約を具体化し、対応する公開APIの実装前提を揃える |
| T23 | H1 | 既存parser/lower/labelsを使うDoc単独HTML、検査済みpage/asset解決、markup/doc-html、安全なserializer、suiteとCLIの実入口 |
| T24 | H2 | Math表示用構造・binding、独立MathML、型付きTeX、Node host KaTeX、式ごとのfallbackを実NEPL3入力から接続 |
| T25 | H3 | 純粋TEA、共通Wasm、Worker生成、隔離preview、同artifact exportをNodeとの比較と3 browserで検証 |

依存はT22→T23→T24→T25の順に成果物を供給する。ここでの矢印は実装順を表す。design/tasks.jsonのdepends_onでは後段が前段を参照する。T22は先行foundation/Doc/Mathの実APIを検証して利用するが、T01〜T09の全体完了を待たせない。T23は未対応embedを型付きで拒否し、T24でMathの実解決を追加する。空fragmentで段階をまたがない。

T10/T11/T13/T15/T17/T18は先行経路を再利用して当初の全責務を実装する。Math evaluate、Circuit、LSP、汎用provider、4言語UI全体、Pages全受入、T21文書移行は残る。T16はT25と既存最終依存、全required受入条件を要求する。先行利用の成功から既存タスクや受入群全体をpassedにしない。

## H0/H1の契約

Docの既存arenaは表・list・page/relative/external link・image等を保持するが、構造proofやCheckedLabelsは完全なPreparedArticleではない。文書検査は表示対象language/parallel policy、外部page・asset・foreign requirementsを明示する。prepareは対象documentのidentity、全要求、解決結果の型・位置・資源閉包を照合する。backendへ未解決slotやhostの任意HTMLを渡さない。

markupはtable/header/cell、ordered/unordered listとstart/checkbox、img/alt/caption、一般linkに必要な要素・属性・内容モデルを閉じた型として持つ。同artifact anchor、page ID、相対path、external URI、asset参照を区別する。危険scheme、path traversal、重複path、未宣言asset、偽digest、wrong slotを拒否し、I/O権限はhostに残す。

Paragraphを入れ子pへ機械変換せず、文のrunと子blockのwrapperを使う。Sentence間に空白を追加しない。Ruby/Anno、Parallel、heading、空表、list start、codeの改行、画像の代替説明を保持し、ブラウザparserのtree補正で意味が変わっていないか確認する。一般DocとKaTeX双方を検査済みmarkupとserializerへ通す。

## H2/H3の契約と検証

17章を適用する。数式の正本は構造であり、本文のdollar探索、MathMLからのTeX推測、source printer文字列を数式として利用しない。非結合operand、負号/累乗、多文字Unicode記号、片側fence、Doc注記の忠実性を共通試験へ結び付ける。表示のためにevaluateを実行しない。

実.nepldからHTMLまでのpositive/negative、MathML-only、KaTeX不在、optional module失敗、局所失敗と全体Stopped、安全性違反、asset不足を検証する。NodeとDOMなし実Workerで固定版・入力・設定が同じ正準markup/manifestを生成することを比較する。

Chromium/Firefox/WebKitでscript禁止・same-origin権限なしのpreview、CSP/CORS・font・非root path、古い返信拒否、停止とepoch、同artifact exportを実行する。書出済みHTMLのJavaScript無効表示、offline bundle、zoom・狭幅・印刷・アクセシビリティtreeを確認し、DOM存在だけを視覚組版の成功にしない。WebKit自動試験とSafari/iOS実機確認を区別する。

各段階は実API・command・source/spec・target・ログ・独立レビューと未検証範囲を記録する。既存bootstrap、Doc source printer、source/Origin、予算、no_std/WASIの回帰を維持する。ローカル成功はPages公開の証拠ではない。

## 正式文書のDoc移行

T23で必要表現が使えるページからT21の棚卸し・変換を進め、数式を含むspecはT24の実生成経路を使用する。T21の完了依存をT24/T19へ接続し、Math評価・Circuit全体の完成待ちでDoc正本化を遅らせない。ページ単位の意味・リンク・安定ID・生成物対応とbootstrap条件は16章のまま維持する。README/AGENTS等のMarkdown入口は必要に応じて.nepld正本から生成し、二重手書き保守をしない。
