# 19章の独立内容レビュー

原稿92267e730b6c7bedd2978d8bdea09589006e4f7cの318行を、c90d306の元spec/19-html-fragment.md全文と直接照合した。生成文書や実装担当の説明を本文の基準にしていない。修正を要する内容欠落・条件反転は見つからなかった。

原本はsources.jsonにGit commit/path/bytes/SHAを記録した5ファイル。freeze.pyで再採取できる。check.pyは固定formsからの制限付き独立原文parser。実行記録はexecution.json/run.log、詳細結果はresult.jsonである。

## 確認した契約

- safe-markup/2・markup revision2、旧revisionを新proofとして受理しない条件、Doc参照とreceiverの同時更新を保持。HTML fragment限定と後続のMathML/SVG/KaTeX profile・shell・asset・PreparedArticle・完全Reportを完成扱いにしない。
- arena/typed attribute/子参照、Rust ordinal非依存、未知tag・RawHtml・script/style等拒否、深いdrop安全性の要件を保持。Block/Phrasing、a子孫の入れ子拒否、table順序とtbody明示、caption/thの子孫制限、figcaption位置・個数、void子禁止、img必須src/altと空alt例外を全部保持。
- 全参照・cycle・未到達・表示出現ごとのDAG/ID重複検査を保持。decoded HTML IDの非空XML、U0000–0020/U007F–009F禁止、日本語・先頭数字・大小文字と非正規化、percent非decodeを保持。data/classは元ASCII制約のまま。Fragmentの実ID一致と属性名重複拒否も残る。
- HtmlPolicyの自己申告はCSSの信頼・実在証明ではない。登録class、重複/空列、Lang、role、正整数ARIA・画像寸法、ol.startの0以上i32上限、th.scope限定を保持。
- Artifact/srcの字句、空/`.`/`..`拒否、著者IDと配布pathの別変換を維持。BetweenArtifactsのsource親とtargetの共通prefixによる相対path生成、未検査の著者`../`とは異なること、非root/オフライン配置を保持。shellとのsource一致・target/fragment実在・同じ版・最終出力はhost/Doc準備の義務として残り、raw自己申告から権限を得ない。
- constrained External URIの小文字scheme、ASCII、percent、host/port/userinfo/空白/backslash/network-path条件と、全URL parserでない限定を維持。到達性やrevisionは推定しない。
- ASCII属性順、XML escape、二重引用符、void、pre先頭LF保存、shell別責務を保持。全操作の累積Budget、表示出現・caller Depth、semantic escapeがSourceBytes/Nodes/Depth入場を代行しない条件、Output先行課金非巻戻し、完全文字列のみ成功、物理OOM保証との区別を残す。
- 初回NDFでschemaとnative構造を再検査し、pointer/以前の登録/proofを受信根拠にしない。stylesheet/asset完成proofとは区別。
- 実HTML parser/ブラウザ比較、Ruby base/rt/rpの全条件、表cell数一致とゼロ列例外を保持。enqueue/初期化前Work、単調Usageを保持。512/600のDivが511へ平坦化された元観測と出力profile検査義務を残し、任意DOM深度を構造proofから保証しない。
- Fragment/Artifact/BetweenArtifactsのUTF-8 uppercase percent encoding、unreserved維持、元percent→%25、DOM IDはHTML escapeだけという二層を保持。旧見出しalias・URL互換・正本切替は別工程。

## 著述と範囲

元17個のInlineCodeと5リンク先を順序・bytesとも一致確認。元に表/fenced code/節見出しはなく、原稿にもそれらを架空追加していない。463個のRubyのbaseは漢字/々、readingはかな。本文とリンクラベルの漢字部分を実Rubyとし、通常説明はliteral、code/linkのある文だけprefix。用語説明をRubyへ誤って押し込まず、義務・例外の文単位を保っている。

この照合は元規範の保存であり、元規範の全外部標準を今回新規に検証したこと、production parse/serializer/browser試験、最新実装状態、正本切替や人による意味レビューの充足を意味しない。特に元の後続実装という時点記述を、今回の実装未完/完了の新判定として扱わない。
