# Docページ集合とリンク解決

`nepl3_doc_core::pages` は明示されたページ集合内でリンク先を検査する。
filesystem、URLへの接続、HTML生成、guest評価は行わない。
ページの意味上の存在と、特定の出力でanchorが表示されることは別の条件である。

`interfaces/doc.json` の順序付きrecordを交換契約とする。
`PageRegistration` は安定した `id`、入力の論理 `source` path、出力artifact内の
`route` を持つ。`PageDocument` はregistrationと完全な `DocumentSyntax` を持ち、
`PageSet` はその順序付き非空listである。登録順が返却indexの名前空間となる。

- idは空でないASCII英数字・`-_.` の一segmentで、`.` と `..` は禁止する。
- sourceはUTF-8の相対file path。segmentは空でなく、`.`、`..`、制御文字、
  `\ : ? # %` を含めない。URI decodeやOS固有のcase foldingは行わない。
- routeはartifact rootからのASCII相対file path。各segmentはidと同じ文字集合。
- idの重複、source/routeの重複とfile/directory衝突を拒否する。
  例：`docs/a` と `docs/a/index.html` は同時に登録できない。

`resolve` は全documentを既存の構造・source・label検査へ通す。同じ操作内で同じ
source/revisionを別の内容へ置き換えることはできない。文書間で同じlabel名を使う
ことは許す。未選択のParallel variantも含めて文書全体を検査する。

開発用Doc source hostのdecode先IDは、元source IDのUTF-8 byte長、ID、revision、
予約counterを `length:id:revision:counter` と連結する。同じsourceを同じ設定で
読み直した場合は同じIDを生成し、別sourceのdecode結果を単独counterで衝突させない。
入力bundle側が予約namespaceと衝突すれば既存source検査で拒否する。短い表現を使い、
長い解説例もWork 100M / Allocation 500Mの既存上限で検証する。

## HTMLページ集合

`nepl3_doc_html::pages::render_pages` の入力はPageSetと共通RenderOptions。
全ページを解決してから同じbackendで生成する。remainingがあれば要求を返して停止し、
外部リンク・画像・foreignを空の表示へ変えない。結果のfragmentsは登録順であり、
PageSet identityと各document digest、実際のoptions、markup、origin対応を保持する。

各リンクは `BetweenArtifacts` として実際の元/先routeを結び、fragmentを共通のhex ID
規則で変換する。全ページの生成後、出力された各hrefのfragmentがリンク先HTMLに
実在することを検査する。Single表示で隠れたanchorを参照した場合は
`MissingOutputAnchor { page, node, target }` とし、壊れたリンクを成功で返さない。
リンク元自身が非表示の場合は出力hrefがないため、この表示上の失敗にはしない。

portableは全PagesHtmlRequestから生成を再実行し、RenderedPages全体を照合する。
route、options、source、targetの変更後に古い結果を採用しない。この結果はHTML
fragment集合であり、fileの保存完了や公開確認の証拠ではない。

`LinkTarget.Page` は登録idで検索する。`Relative` はリンク元sourceの親directory
から解決し、`.` を除去、`..` を一段ずつ戻す。明示したrootより上への移動、絶対path、
空segment、percent encoding、末尾のdirectory指定を拒否する。
正規化後のsourceと完全一致する登録だけを採用する。暗黙の拡張子追加、index補完、
最寄りページへのfallbackは行わない。fragmentはリンク先DocのSection/Anchorの
意味上のidと完全一致しなければならない。HTMLの `n-` hex IDへの変換はbackendの責務。

`PageLinkPlan` のidentityは `NEPL3.Doc.Pages.v1\0` とPageSetのcanonical NDF byte列
を連結したSHA-256。登録順、route、各documentのsource・origin・内容を含む。
linksはページ順、その中でDoc arenaのnode順。各linkに元page/node、先page、
元のfragmentを保持する。外部URI、asset、foreignの要求は `remaining` に元pageと
一緒に保持し、成功した内部リンクへ読み替えない。

失敗は空集合、登録値不正（page/field）、衝突（page/previous/field）、不正な相対path
（page/node）、未登録ページ（page/node）、未存在fragment（page/node/target）、
既存の構造・label・codec失敗を区別する。全走査・複製・キュー拡張は同じBudgetを使い、
最初の停止を保持する。部分的なplanを成功として返さない。

portableのset receiverは型・source・Doc構造を検査したraw dataを返す。
`resolve` を呼ぶまでリンク検査済みではない。plan receiverは元PageSetに対して
解決を再実行し、identity・順序・全link・remainingを比較する。古いregistryの結果、
linkの省略・先の改変・偽造された成功を受け入れない。

この段階はDoc移行用の意味上のページ索引であり、完全なPreparedArticleではない。
HTML hostは実際の配置routeとの一致、選択言語でのtarget anchorの出力、全page fileの
存在を確認する。asset/guest解決、旧URL/anchor対応、Markdown projection、意味レビュー、
Pages配信と復旧の受入は別途必要である。文書inventoryの過去baselineをこの登録の
代わりに使わず、移行時の実際の文書集合から作成する。
