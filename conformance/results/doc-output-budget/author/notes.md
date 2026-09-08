# 21章 output budget 追補の手動著述

依頼どおり authored/21-doc-pages.nepld だけへ追加3段落を手書きした。元の Nodes / Work 段落と最終 Section 留保の間へ配置。既存前後全文のbyteは不変で、原本Markdownは編集していない。root が並行実装中の同worktreeへ置いたため、著者からcommit/pushはしない。

原本の意味を追加翻訳せず、一文ごとにSentenceとした。InlineCodeを含む文は明示sentence、他はliteral、通常本文の漢字部分だけRubyとし、Anno用の新しい訳注は作っていない。全8fieldの順序、u64非負整数の範囲、省略とnullの区別、0有効、未知/欠損/負数/非整数拒否、無制限値なし、初期停止/累積Usage、呼出側のborrowed Budget、部分成功なし、I/O留保、計上範囲外、意味と実行identityの分離、hashのdomain/NUL/32byte/8byte big-endian/全数値順、旧停止と移行受入を保持する。

構造audit・Ruby基底の段落全文一致・code値順・UTF8/LF・前後原稿byte保存を自己確認。初回の自己検査scriptは結果件数のfield名をsentencesと仮定してKeyErrorとなり、正式formsのParagraph.itemsへ訂正した。構造parseと本文比較はその前に成功しており、production parserエラーではない。

これは著者の構造・内容自己検査であり、独立レビューやproduction parser/lower/HTML、canonical切替合格ではない。原本採取後rootがb84e9a3へcommitしたことを確認。原本採取hashと現在原本hashの一致は最終確認し、稿は未commitのままrootへ返す。
