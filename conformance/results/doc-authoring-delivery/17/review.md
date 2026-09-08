# 17章の独立内容レビュー

固定対象は f514205e41f1bdd9797cd2188d8b1051c8fe1137 の原稿17章、比較元は c90d3064b71634388a13bbbd5f34570634ab3f24 の spec/17-math-html.md。sources.json の5つのGit原本を読み、原稿505行と元仕様の全節を直接比較した。生成Markdownを意味の基準にしていない。本文の修正要求となる欠落・条件の反転は確認されなかった。

## 内容の照合

- 1節：生成時のKaTeXPreferred、閲覧時JavaScript不要、独立MathMLの全constructor対応、暗黙Evaluate禁止を保持。CLI・Worker・閲覧の3行表と、Worker renderToString、ブラウザでNode/serverを要求しない条件、optional module失敗時のMathML継続、別math Worker推奨、rendererEpoch、Wasm不在時の生成不可・外部serverへの黙示fallback禁止がある。
- 2節：math-core、TeX emitter、MathML backend、host、suiteの責務を分離。両emitterのno_std + alloc、入力escape、忠実変換できないRuby/Anno等は式全体をMathMLへ戻す条件、公開前の型/schema/codec/検査を保持。名前を置くだけで実装済みとはしていない。
- 3節：失敗表8行を照合。通常変換不能・通常構文エラーと、偽identity・未要求資源・危険出力のprovider違反を区別。全体Stoppedを成功へ変えず、局所renderer期限の継続は親の明示条件と残資源に限定。既存の同request MathMLの表示は保存成功ではなく、新要求の明示再生成が必要。CSS/font異常もhtmlAndMathmlによる自動検出とせず、MathMlOnly再生成へ分けている。
- 4節：version/options/macros/resourcesと全epoch/request/source/profile identityを保持。maxExpandを一般catchで成功にせず、maxSizeのclampを忠実成功としない。外部KaTeX内部の未観測Work/Allocationを0や推計で完全Reportへ埋めない。Worker終了、親停止、局所期限、予約精算を区別。I/O・DOM非依存module、shellへTeX文字列を埋めないこと、式ごとの新macro objectと将来共有時の先行契約を維持。
- 5節：KaTeX実出力棚卸と専用typed検査、未知要素削除による成功禁止、一般Doc権限を広げないことを保持。class/stylesheetを第一候補とし、制限inline styleは明示profileと独立レビュー・負例・視覚試験を先行。読み上げの二重化回避、scriptなしiframe、doctype、CSPとWorker/font CORS/非root pathの正負試験を保持。同artifactのpreview/exportとSaveFinished、同package CSS/fontのpath/MIME/bytes/digest/license、相対font参照、CDN非依存、全asset保存が残る。自己完結化は検査済みassetの束縛であり、再renderやiframe DOM exportではない。
- 6節：T22〜T25順序、既存T08構造と後続責務の維持、M04/U02/U03/U07/S01〜S05の条件、3ブラウザ実行、CLI/WASIのhost不在時診断付きMathMLを維持。
- 7節：根拠URLと、章の追加だけではadapter/backend/Web実行の完成を示さない限定を保持。

## 著述と再現可能な補助確認

check.py は固定formsを用いる独立の制限付き原文parserであり、production parserの置換や意味証明ではない。実行は execution.json/run.log、全結果は result.json。21のInlineCodeを元Markdownとbyte列・順序とも一致確認。6つのリンク先も完全一致。表は3列3行と2列8行（それぞれheader別、Default alignment）、節IDは7つで重複なし。元仕様にfenced codeはない。

703個のRubyを抽出し、baseを漢字・々、readingをかなへ限定して検査。本文は意味のまとまりをSentenceとし、通常文はliteral、code/link等を含む文はprefix Inline列で記述されている。表セルも文として保たれ、語ごとの機械的文分割をしていない。専門語説明・例外条件は本文に残り、注釈量やbase projection一致だけを意味レビューの代用にはしていない。

このレビューは固定原稿の内容保存・authoring境界である。新たなKaTeX外部API仕様の確認、production parse/HTML、ブラウザ表示、相対link解決、正本切替、要求された人による意味レビューの完了は主張しない。予算不足を本文削減の根拠にしていない。
