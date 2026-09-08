# 0004: Pages復旧とDoc設計前のinventory

- 日付: 2026-09-06
- 状態: 採用
- 対象設計: `nepl3-design-2026-09-06-r3`
- 対象レビュー: commit `25a096b` の公開後失敗処理とDoc移行順序

## Pages公開後の失敗

従来仕様はpublic smokeの失敗を記録するだけで、切替済みの公開物を復元する処理がなかった。T20/S06へLKGの永続保存、公開writerの全区間排他、candidate identityの照合、有限の旧payload再deployと再smokeを追加する。復旧成功後も元runはfailedを維持する。

復旧archiveはpublic smokeに合格した元tarをimmutable recovery releaseへ保存し、現行LKGと直前世代を期限で消さない。通常のActions artifactは短いretentionやrun削除で失われるため、復旧保存の正本にしない。payload保存→download検証→journal昇格の順序を守る。保存未確定、復旧失敗、初回LKG不在、run消失は独立した状態にし、推測による新たな公開を止める。

Pages APIにはexpected-currentによるatomic切替の公開契約がない。全writerを同じconcurrency groupに限定し、保護journalとAPI receiptと公開identityを合わせて対象を確認する。新しい健康な公開や対象不明の公開を、古いcandidateの失敗処理で上書きしない。詳細と公式資料の照合は [15章](../spec/15-site.md) に記録する。

## Doc schemaの設計前にinventoryを使う

現在のDoc schemaには表/list/一般link/汎用code/図等の不足がある。T21の変換直前に初めて本文を調べると、T01/T02/T07で確定した公開型・wireを再び広く変える必要がある。そこで現存文書のbaseline inventoryとgap reportを早期に作成し、Docに関係する共通型・wire・意味モデルの設計前に消費する。

早期監査の完了とR014全体の解消を分ける。T01/T02/T07は監査から必要な表現を確認して設計するが、T21や未実装backendの完成を前提にしない。T21は実装・wire・backend・conformanceが整った後にページ単位の移行を完成させる。この順序により循環依存と二重手書き保守を避ける。

今回の変更はr3の公開失敗条件と移行順序の具体化であり、言語のform・意味・NDF表現をまだ変更しないためdesign revisionはr3を維持する。source/spec identityは差分により変わり、古い実行証拠を現在の証拠へ転用しない。新しいDoc constructor等を採用する段階では、仕様識別と全影響箇所を改めて更新する。
