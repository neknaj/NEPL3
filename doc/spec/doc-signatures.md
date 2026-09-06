# Doc：構文signatureの全表

この表は `design/forms.json` と同じ規範データから生成した。`List<T>` は `cons T List<T>` / `nil`、`@` は基礎readerの識別である。

## Doc/Article

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `article` | `Doc.Article` | language: @Lang, title: Doc/Sentence, body: Doc/Body | 3 |

## Doc/Body

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `body` | `Doc.Body` | blocks: List<Doc/Block> | 1 |

## Doc/Block

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `paragraph` | `Doc.Paragraph` | items: List<Doc/Flow> | 1 |
| `section` | `Doc.Section` | id: @Name, title: Doc/Sentence, body: Doc/Body | 3 |
| `display` | `Doc.DisplayMath` | syntax: Doc/MathGuest | 1 |
| `circuit` | `Doc.CircuitFigure` | caption: Doc/Sentence, syntax: Doc/CircuitGuest | 2 |
| `code` | `Doc.Code` | syntax: Doc/Guest | 1 |

## Doc/Sentence

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `sentence` | `Doc.Sentence` | inlines: List<Doc/Inline> | 1 |

葉の認識規則：`sentence`。

## Doc/Flow

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `paragraph` | `Doc.Paragraph` | items: List<Doc/Flow> | 1 |
| `section` | `Doc.Section` | id: @Name, title: Doc/Sentence, body: Doc/Body | 3 |
| `display` | `Doc.DisplayMath` | syntax: Doc/MathGuest | 1 |
| `circuit` | `Doc.CircuitFigure` | caption: Doc/Sentence, syntax: Doc/CircuitGuest | 2 |
| `code` | `Doc.Code` | syntax: Doc/Guest | 1 |
| `sentence` | `Doc.Sentence` | inlines: List<Doc/Inline> | 1 |
| `parallel` | `Doc.Parallel` | variants: List<Doc/Variant> | 1 |

葉の認識規則：`sentence`。

## Doc/Variant

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `variant` | `Doc.Variant` | language: @Lang, sentence: Doc/Sentence | 2 |

## Doc/Inline

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `text` | `Doc.Text` | text: @Text | 1 |
| `concat` | `Doc.Concat` | inlines: List<Doc/Inline> | 1 |
| `ruby` | `Doc.Ruby` | base: Doc/Inline, reading: Doc/Inline | 2 |
| `anno` | `Doc.Anno` | base: Doc/Inline, notes: List<Doc/Inline> | 2 |
| `math` | `Doc.InlineMath` | syntax: Doc/MathGuest | 1 |
| `anchor` | `Doc.Anchor` | id: @Name, label: Doc/Inline | 2 |
| `ref` | `Doc.Reference` | target: @Name, label: Doc/Inline | 2 |
| `em` | `Doc.Emphasis` | inline: Doc/Inline | 1 |
| `strong` | `Doc.Strong` | inline: Doc/Inline | 1 |
| `break` | `Doc.Break` | なし | 0 |

## Doc/MathGuest

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `Math` | `Doc.MathGuest` | syntax: Math/Expr | 1 |

## Doc/CircuitGuest

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `Circuit` | `Doc.CircuitGuest` | syntax: Circuit/Design | 1 |

## Doc/Guest

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `Grammar` | `Doc.GrammarGuest` | syntax: Grammar/Root | 1 |
| `Doc` | `Doc.DocGuest` | syntax: Doc/Article | 1 |
| `Math` | `Doc.MathGuest` | syntax: Math/Expr | 1 |
| `Circuit` | `Doc.CircuitGuest` | syntax: Circuit/Design | 1 |
