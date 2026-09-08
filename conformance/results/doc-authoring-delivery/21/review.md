# 21章の独立内容レビュー

原稿18d79ef54c64c155bbc7acfc41e7512f1e38dfeeの573行を、main07418665c3bea201315e6d8a69828715b24f3c84のspec/21-doc-pages.md全節と直接照合した。本文欠落・義務や例外の反転による追加blockingはない。

原本SHA d37b20a7c84abbbe6ae998bca690b13e35383b81f77c760075fd80a57f849c36、原稿SHA e6afe2c852df1a3c8667426ca94f92cbe2885296f0f74fbd1a5ba099ebe1a3b5は作者/parent通知値と一致。authors branchへ元mainをmergeしたとは仮定せず、明示main Git blobを採取して比較した。freeze.py/sources.jsonで5原本を再採取できる。

## 意味・例外の照合

- PageSet内の意味上の存在と出力anchor表示を分離。filesystem/接続/HTML/guest評価をcoreへ持ち込まない。ordered record、Registration/PageDocument/非空PageSet、登録順index namespaceを保持。
- id/source/routeの3字句を分離し、sourceのUTF-8相対path・禁止文字・非URIdecode・非OS case folding、idとrouteのASCII segment、重複・file/directory衝突を保持。例のdocs/aとdocs/a/index.htmlの同時登録禁止も一致。
- 全document/source/labelの検査、同source/revision内容のすり替え拒否、文書間同名label許可、未選択variantの全検査を保持。開発hostの長さ付き予約namespace、同入力同設定の再現性、counter単独衝突防止、入力namespace衝突拒否と既存Work100M/Allocation500Mを維持。
- render_pagesの全page解決後生成、ExternalのMarkupと同じ字句実装、未選択URIもInvalidExternalUri、推測/trim/補完禁止を保持。残る画像/foreign要求は完全planへ戻り、既検査externalも意味要求として残す。登録順fragments/identity/digest/options/originsを保存。
- BetweenArtifactsの実routes、hex fragment、生成後target anchor実在を保持。Singleで先anchorが隠れる場合のMissingOutputAnchorと、元リンクも非表示なら出力失敗としない例外を保持。portable全再生成・古いroute/options/source/target拒否、保存公開proofでない限定も残る。
- pages CLIのmanifest-root、version1、絶対/親移動/root外symlink拒否、64KiB/128pages/入力合計10MB、Rows固定を保持。同directory CSS共有と不一致/衝突拒否、大小文字segment不一致・末尾dot・Windows device名の保存前拒否、黙示改名禁止、.html終端を維持。
- 全検査後の新directory・非上書き、manifest最後、PageSet/原稿/Profile/digest/MIME/license、途中失敗の未完成directoryと成功manifestなしを保持。navigation/旧URL/Pages復旧は別scope。
- Page id検索とRelativeの親source基準を分離。`.`除去と`..`段階戻し、root越え/絶対/空segment/percent/末尾directory拒否、正規化source完全一致、拡張子/index/最寄fallback禁止、semantic fragment一致とbackend n-hexを保持。
- PageLinkPlanのdomain・canonical NDF・SHA256、登録順/route/source/origin/content、page/node順・元fragment、remainingと元pageを保存。型付き全失敗と同Budgetの全走査/複製/queue・最初の停止、部分plan成功禁止を保持。set raw受信だけではリンクproofにならず、planは元PageSetで再resolveし全列比較する。
- PreparedArticle/asset/guest/旧URL/意味レビュー/Pages復旧を完了扱いにせず、移行時の実際の集合を使い過去inventoryで代用しない。
- Markdown出力は通常parse/lower/inspect→Doc arenaであり、source正規表現から本文を抽出しない。同階層SectionまたはSectionなし、非空Sentence列・非空unordered/checkなし/単一段落itemを保持。Sentence境界に空白/改行/句点を補わず著者空白を残す。paragraph/parallelを平坦化せず、code隣接検査を境界越えで維持。
- projectionのText/InlineCode/途中Breakと非対応の全制限、外端空白/制御/空inline拒否、code bytes/空白保存、list indentation、Sentence端/連続Break/周辺空白拒否、Text LF非自動変換を保持。一般Docそのものを制限したことにしない。
- RawCodeは非評価fence、最長backtickより長く最低3、TAB/空白/Unicode/LF/末尾空行保持、空本文例外、非空末尾LF必須、CR/CRLF/他制御拒否、LF補充や改行正規化を同一としない。hint字句、list内非対応、別parserでbytes/hint/block検査、renderer /4を保持。
- typed markdown同BudgetとOutput先行、本文1MiB、部分出力なし、CLI source10MB・pathUTF-8/4096、provenance comment escape、新fileのみ、I/O失敗条件を保持。
- Nodes10Mはdesktop hostの累積訪問設定でありcore会計省略ではない。旧1M・512文/128段落/約14KBの停止例、Work100M/Allocation500M据置、停止後増量禁止、別host強制なし、大文書全完走を主張しない限定を保存。一般roundtrip・旧anchor・正本registry・人レビューは別で元Markdownを削除しない。

## 著述と確認範囲

check.pyで固定formsを用いる制限付き独立parserを実行。40個のInlineCodeと2リンク先は元順序・bytesで一致し、domain文字列の表示用backslashも保持。節ID3個に重複なし。最初の登録規則は4項目unordered list。元に表/fenced codeはなく、原稿にも架空の表やコード本文を追加していない。690個のRubyは漢字/々のみのbaseとかなreading。通常文はliteral、code/linkを含む文はprefixを用い、文の対応単位と条件のまとまりを保つ。

このレビューは固定本文とauthoring契約の照合であり、全runtime/リンク解決/HTML/Markdown/browser試験を今回再実行した証拠ではない。制限付き独立parserの成功だけでproduction parseを推定しない。人による意味レビュー、旧URL互換、移行完了を満たしたと扱わず、処理上限を理由に本文削減を要求しない。
