非Doc fileはmanifestの省略可能な `files` listへ、pageと同じid/source/route/inputで
明示登録する（省略は空、nullは禁止）。最大128件で、Doc原稿と合わせた実入力byte数は
10MBまで。入力path・root境界の検査はpageと共通で、fileの内容はUTF-8へ変換せず保持する。
`generate_with_resources` も明示されたEntryとbyte列だけを受ける。linkを根拠に周囲のfileを
探索しない。返却HTMLが参照するfileを、hostは同じ出力集合へbyte完全一致で保存する。
manifestには登録元・実input・route・byte数・SHA-256を記録し、fileのMIMEは
`application/octet-stream` とする。これは任意fileを検査済みHTML・CSS・画像として
認定するAPIではない。公開host側のMIME配信設定や内容実行の許可は別契約である。

文書とfileを合わせてid/source/routeの衝突を検査する。衝突診断のregistration indexは
pagesの後にfilesを連結した順序。`LinkTarget.Page` はDocだけを検索し、`Relative` は
正規化後のsourceが一致するDocまたはfileを検索する。fileにはDoc anchorがないため、
fragment付きは `FileFragment { page, node, file }` として拒否する。fragmentの文字列を
勝手に落とさない。fileへのHTML hrefは実route間の相対参照となる。

PageSetのcanonical NDFはfileのregistrationと実byte列を含む。byte列・route・登録順が
変わればidentityが変わり、portable receiverは古いplan/HTMLを再利用できない。fileの
境界codec・hash・出力コピーも有限の共通output Budgetに累積し、cancel/停止を引き継ぐ。
生成するHTML、stylesheet、完了manifestとのpath衝突は保存前に拒否し、stylesheetと
byte列が偶然一致してもfile登録を共有扱いにしない。この拡張はDoc schemaのdigestを
更新するため、旧digestのPageSetを新しい2-field recordとしてdecodeしない。