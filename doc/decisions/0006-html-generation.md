# HTML先行利用と生成時KaTeX

この文書は採用時の理由と指示の記録であり、現在の作業順・実装状態の正本ではない。成果物と順序は [T22〜T25](../../design/tasks.json)、層間の接続は [第18章](../spec/18-html-delivery.md)、実装・受入状態は [implementation-status.json](../../implementation-status.json) を参照する。

当時の基準として提示された`933f0bb`へ戻さず、既存の`lower::document`・`labels::check`・`plain_text`・source printerと、進行中だったMath arena・shape・lower・codecを育てる方針を採った。この実装経緯を、現在の内部関数構成や特定commitを要求する恒久API契約とは扱わない。

2026-09-07の直接指示により、4言語製品の最終範囲を保ったまま、文書HTMLを先行利用するT22〜T25を追加する。元のT10/T13全体依存を、Doc単独HTMLや表示用Mathの先行実装まで待たせる条件として使わない。既存parser・Doc処理・Math arenaを育て、toolsに別の利用者向け処理系を作らない。

HTMLは生成環境でKaTeXを実行して完成HTMLとCSS/fontを配布する。CLIはhost、WebはWorkerを使用する。独立MathMLをfallbackとアクセシビリティ表現に残し、表示時の評価や閲覧時KaTeX実行を要求しない。このため10章のfont同梱禁止を訂正し、純粋Math→TeX backendを依存表へ追加した。空crateは作らない。

CSSのclass化と自己完結previewを第一候補とし、script禁止・same-origin権限なしを維持する。独立レビューで確認したmaxExpandの例外分類、maxSizeのclamp、強制終了時の未観測Usage、doctype、CSP/CORSを17章へ反映した。局所renderer失敗と文書全体Stoppedを区別する。API/型/codecの具体化と実ブラウザ試験はH0〜H3の未完部分として残る。

実装はメインagent、独立レビューはsubagentという最新の直接指示をAGENTS/CODEX/developmentへ反映する。補助指示書13節の以前の分担は再導入しない。T22〜T25や本判断の追加だけで、runtime受入・Pages公開・文書移行を完了しない。
