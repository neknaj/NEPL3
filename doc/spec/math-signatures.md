# Math：構文signatureの全表

この表は `design/forms.json` と同じ規範データから生成した。`List<T>` は `cons T List<T>` / `nil`、`@` は基礎readerの識別である。

## Math/Expr

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `add` | `Math.Add` | left: Math/Expr, right: Math/Expr | 2 |
| `sub` | `Math.Sub` | left: Math/Expr, right: Math/Expr | 2 |
| `mul` | `Math.Mul` | left: Math/Expr, right: Math/Expr | 2 |
| `frac` | `Math.Frac` | left: Math/Expr, right: Math/Expr | 2 |
| `pow` | `Math.Pow` | left: Math/Expr, right: Math/Expr | 2 |
| `equal` | `Math.Equal` | left: Math/Expr, right: Math/Expr | 2 |
| `lt` | `Math.Lt` | left: Math/Expr, right: Math/Expr | 2 |
| `le` | `Math.Le` | left: Math/Expr, right: Math/Expr | 2 |
| `subscript` | `Math.Subscript` | left: Math/Expr, right: Math/Expr | 2 |
| `superscript` | `Math.Superscript` | left: Math/Expr, right: Math/Expr | 2 |
| `neg` | `Math.Neg` | value: Math/Expr | 1 |
| `sqrt` | `Math.Sqrt` | value: Math/Expr | 1 |
| `transpose` | `Math.Transpose` | value: Math/Expr | 1 |
| `det` | `Math.Det` | value: Math/Expr | 1 |
| `root` | `Math.Root` | degree: Math/Expr, radicand: Math/Expr | 2 |
| `scripts` | `Math.Scripts` | base: Math/Expr, sub: Math/Expr, sup: Math/Expr | 3 |
| `fence` | `Math.Fence` | open: @Text, close: @Text, value: Math/Expr | 3 |
| `sequence` | `Math.Sequence` | values: List<Math/Expr> | 1 |
| `symbol` | `Math.Symbol` | name: @Text | 1 |
| `text` | `Math.Text` | text: @Text | 1 |
| `vector` | `Math.Vector` | values: List<Math/Expr> | 1 |
| `matrix` | `Math.Matrix` | rows: List<Math/Row> | 1 |
| `let` | `Math.Let` | name: @Name, init: Math/Expr, body: Math/Expr | 3 |
| `sum` | `Math.Sum` | index: @Name, lower: Math/Expr, upper: Math/Expr, body: Math/Expr | 4 |
| `integral` | `Math.Integral` | index: @Name, lower: Math/Expr, upper: Math/Expr, body: Math/Expr | 4 |
| `call` | `Math.Call` | function: Math/Expr, arguments: List<Math/Expr> | 2 |
| `label` | `Math.Label` | value: Math/Expr, annotation: Math/DocGuest | 2 |

葉の認識規則：`math-number-or-name`。

## Math/Row

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `row` | `Math.Row` | values: List<Math/Expr> | 1 |

## Math/DocGuest

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `Doc` | `Math.DocGuest` | syntax: Doc/Sentence | 1 |
