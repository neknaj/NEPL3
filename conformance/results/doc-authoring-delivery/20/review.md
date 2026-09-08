# 20章の独立内容レビュー

46a54fb8b4be41668696895f63918213ccc6d1fa の原稿231行と、c90d306のspec/20-doc-html.md全段落を直接比較した。内容欠落や義務・例外の反転による追加blockingはない。生成Markdownや著者の自己比較を意味の判定基準にしていない。

freeze.py/sources.jsonは原稿・元Markdown・authoring・AGENTS・formsの5つのGit原本を固定する。check.pyは制限付き独立原文parserを用い、12個のInlineCodeと2リンク先を元順序/bytesで照合した。節ID2個は重複なし。元と原稿に表やfenced codeはない。401個のRubyは漢字/々だけのbaseとかなreadingである。

## 直接照合した内容

- doc-core/markup依存の純粋backend、ファイル/DOM/guest意味処理なし、localは外部要求ゼロArticleだけという境界を保存。後段のpage/asset/foreign/shell/Reportを完成した扱いにしない。
- requestの所有、codec構造/source閉包の再検査とraw options、prepare_localの全要求・label検査、完全planをNeedsResolutionで返す条件を保持。外部URI合法性やMath意味エラーの判定とは区別。対象document/optionsを借用する非公開constructor proofを保持。
- Rows/Columns全variant元順、Single言語規則・case-insensitive重複拒否・明示fallback順・MissingVariant、空Sentenceが実在variantである例外を維持。表示出現別の生成番号groupも保持。
- Article/title・Section n-hex IDと深い見出しrole/aria、Anchor/Reference、Paragraphの前後pと子Block div、順序・空白を保持。Sentence/Concat、em/strong/Break、Ruby/Anno全層、table header/body/alignment/空表、list/checkbox/ol.start範囲拒否、RawCode hint非実行を全部保持。
- 反復arena構築のnode/属性/文字列/queue/depth課金、共通markup再検査、XML/内容違反の削除成功禁止、OutputDepth256と元Doc node、入力モデルを平坦化しないこと、shellの包囲深度検査を保持。
- RenderedFragmentのdocumentDigestとRenderOptions、HtmlRequest、各生成Text/elementへの出力順原因を保存。原Span/Origin/source参照、架空位置禁止、固定CSSと自己申告の証明範囲を区別。
- portable受信は明示prepared document/options・schema/markup/32byte digest検査・同backend再実行・canonical CBORに正確なdomainとゼロbyteを前置。markup/原因対応/document/options改変をproofにしない。将来外部renderer授権やArtifact完成検査と別で、同Budgetの停止/単調Usageを保持。
- T23/T21をlocalだけで完成にせず、外部要求を満たしてから16/18章条件で正本切替。
- 開発host exportのコマンドと3出力名、UTF-8、10,000,000 bytes読取り上限、超過解析禁止、生成後に新directoryを作り既存出力非上書き、I/O途中失敗の不完全directoryとmanifest最後を保持。
- 検査済bootstrap→実Grammar compilerと正式CLI/suite/資源解決の区別、source/lower/render各独立Budgetを文書一操作完走へ読み替えないことを明示。固定shell/CSS、CSP、input/Profile/schema/output/options identity、非root/JS無効実browser、NeedsResolutionで省略出力しない条件を保存。
- CSS Ruby reading1/base2とbaseline-source:last、Anno base1/notes2とfirst、空行を挟まない条件、入れ子baselineと改行の基準、固定offset/JS禁止を保存。CSS草案の存在をbrowser実装証拠にせず実試験、狭幅でも任意分割しないmax-content box、overflowし得る場合の閲覧/印刷と移動手段の別途検証を残す。

通常文はliteral、code/linkを含む文はprefixで、文単位の自然な意味・数値・例外が維持されている。Rubyで送り仮名を包んだり、説明用の訳語をreadingに入れる箇所は確認されない。元の本文を短縮して処理上限へ合わせてはいない。

これは固定原稿の意味・authoringレビュー。production HTML/ブラウザを今回再実行したこと、リンク先の最新外部仕様確認、現在の全実装状態、移行完了、人による意味レビューの充足は主張しない。元の時点記述と後続要件もそのまま保持し、今回の実装完了判定とは分ける。
