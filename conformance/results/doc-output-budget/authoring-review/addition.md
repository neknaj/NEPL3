文書一式のbuildでは、pages入力manifestの省略可能な `output_limits` に、
`source_bytes, work, depth, nodes, allocation_units, output_bytes, diagnostics, events`
の全8fieldを非負のu64整数で指定できる。省略時は上記の開発host既定値を用いる。
null、一部fieldだけの指定、未知field、負数、非整数は拒否する。0も有効な上限であり、
その資源が必要になれば停止する。無制限を表す値や自動増額は設けない。
これは実行前にhostが選択する出力操作の設定であり、入力本文やproviderが変更する権限ではない。
parse/lower、bare-metal、previewの既定値やParseProfileのLimitsは変更しない。

型付きhost入口 `generate_with_output_budget` は呼出し側のBudgetを借り、全PageSetの
resolve/render/serializeへ同じBudgetを渡す。開始時に停止状態を確認し、既に消費したUsageを
保持する。失敗後に別予算へ切り替えず、部分成果物や成功manifestを返さない。
ファイル出力は全生成成功後に始める。I/O失敗時に残る未完成directoryと、生成操作の成功は別である。
この設定は累積する論理的な資源を制限し、物理的なpeak heapや壁時計の期限は保証しない。
package/Profile準備、hostのJSON・file容器・manifest生成・I/Oを共有出力Usageへ含めたとはしない。

成功manifestには各ページのparse/lowerの全Limits・全Usageと、共有出力の全Limits・
開始Usage・終了Usageを記録する。旧 `output_usage` は互換のため3資源の表示として残す。
PageSetの意味identityと別に `execution_identity` を記録し、出力予算を結び付ける。
計算はUTF-8の `nepl3.local-doc-pages.execution/1` とNUL、32byteの意味identity、
上記field順のLimits 8値、同順の開始Usage 8値を連結したSHA-256である。
各数値は8byte big-endianとし、JSON objectの列挙順へ依存しない。
同じ内容でも設定や開始Usageが異なれば実行identityは異なる。生成物の内容digestは変更しない。
新設定による成功を既定設定の停止記録の訂正や、文書移行・Pages公開の受入合格として扱わない。

