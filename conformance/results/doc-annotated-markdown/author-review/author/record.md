# 注釈付きMarkdown節の手動著述

対象worktreeは `C:/projects/NEPL3-doc-annotated-markdown`、採取時HEADは `7b449ca2d1af3ed0ef135b8a0b9ca929e8a54569`。
rootが編集中のspec21末尾「注釈付きMarkdown閲覧projection」を読み、対応する `doc/migration/authored/21-doc-pages.nepld` へ新Section `annotated` だけを追加した。
他のproduction・仕様・原稿は編集せず、commitしていない。これは著者の自己確認で、独立レビュー・production実行・canonical切替は未実施。

固定原文SHA-256: `243c4327c1f484536632395582bd1a923c109993ee31a8045857a28d9cf444b4`。
追記前の稿: `4ef947ee78b12fb5398f8940f0241e8d8d36e7b68416bc80d188e4e28bd126a6`。
追記後の稿: `f0c7b1748fcce16d8f29786256a4f8cef80d6f7dc24ec6f4fe11c7da3308afb5`。
終了時にも原文のSHAが採取時と一致することを確認した。

## 著述の判断

- 元の8段落を維持し、文ごとのSentenceへ分けた。各段落は3/5/6/6/7/7/5/8文、計47文。元の参照リンク・表・RawCodeはこの追加節にはない。
- 通常文はsentence literalとし、元のinline codeを含む文は明示sentenceにした。14個のcode payloadは順序・byteを完全に保持。コード例中の漢字や括弧はRuby化せず元の内容を保つ。
- 新しい英訳・意味注記は追加していない。元にないAnnoを数合わせで導入せず、既存のAnno表示契約を本文で正確に説明する。
- 漢字だけをRubyのbaseとし、「注釈付/ちゅうしゃくつ」＋「き」、「対応付/たいおうづ」＋「けて」、「見出/みだ」＋「し」、「上書/うわが」＋「き」など送り仮名を分離した。「正本」はせいほん、「一意」はいちい、「空Text」はから、「非空」はひくうとした。全258箇所の読みを `readings.txt` に記録し、文脈と読みを自己確認した。
- 閲覧表示の非可逆性、元のRuby/Anno/Sentence保持、固定strong/em生成と任意HTML禁止、外部URIと到達性の分離、透明構造越しの境界検査、6段見出し、未対応とNeedsResolutionを保持した。
- aliasesの必須name・null/省略section・未知/重複field・Unicode英数字とASCII許可記号・重複/不存在/未到達・n-hex衝突を保持。自動GitHub anchorや旧URLの受入を生成成功から推定しない条件も残した。
- 同一有限Budget、各資源停止/cancel、部分出力と予算再初期化禁止、1MiB/10MB/4096byte、commentの出典、既存file非上書き、I/O途中の未完成file、公開/registry切替を含まない範囲を保持した。

## 自己確認と限界

`check.py` で正式formsを用いた構造auditを原稿全体に実行し成功。初稿の新Section名 `annotated-markdown` はNameの構造auditで拒否されたため、既存稿にない正規の名前 `annotated` に訂正して再実行した。本文の変更や仕様変更による回避ではない。

追加部のRubyを基底本文へ戻した各段落は、原文のMarkdown code記号と空白を除いた比較で全文一致。14codeは空白を含む値を別に完全比較した。Ruby baseの漢字範囲、prefix Textの漢字漏れ0、UTF-8/LF、追加Section前の既存全文byteと末尾 `nil` の完全保持を検査した。空白を除いた本文比較をraw source byte一致や一般Doc roundtripと称さない。

差分は新Section65行だけ。`git diff --check` 成功。構造auditはRustのparse/lower/provider実行ではない。runtime・HTML・Markdown生成・portable・実ブラウザの試験は今回の著述自己確認に含めていない。既存root WIPの試験結果を本稿の成功証拠として流用しない。
