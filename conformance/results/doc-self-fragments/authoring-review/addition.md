`Relative` のpathが空でfragmentが非空の場合だけ、現在の登録Docのsourceを参照先とする。
これは元文書の `#fragment` を保持するための自己参照であり、fileや親directoryの検索ではない。
空pathとNone、空pathと空fragmentは `InvalidRelative` のままとする。自己参照でも
明示IDの完全一致、未選択variantを含む意味検査、実際に出力されたanchorの検査を省略しない。
リンク先Docを推測したり、見出し本文からslugを自動生成してfragmentを書き換えたりしない。

