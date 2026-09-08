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
| `table` | `Doc.Table` | columns: List<Doc/Alignment>, header: Doc/OptionalRow, rows: List<Doc/Row> | 3 |
| `list` | `Doc.List` | style: Doc/ListStyle, items: List<Doc/ListItem> | 2 |
| `rawcode` | `Doc.RawCode` | languageHint: Doc/OptionalText, text: @Text | 2 |
| `image` | `Doc.Image` | asset: Doc/Asset, alt: Doc/Sentence, caption: Doc/OptionalSentence | 3 |

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
| `table` | `Doc.Table` | columns: List<Doc/Alignment>, header: Doc/OptionalRow, rows: List<Doc/Row> | 3 |
| `list` | `Doc.List` | style: Doc/ListStyle, items: List<Doc/ListItem> | 2 |
| `rawcode` | `Doc.RawCode` | languageHint: Doc/OptionalText, text: @Text | 2 |
| `image` | `Doc.Image` | asset: Doc/Asset, alt: Doc/Sentence, caption: Doc/OptionalSentence | 3 |

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
| `link` | `Doc.Link` | target: Doc/LinkTarget, label: Doc/Inline | 2 |
| `code` | `Doc.InlineCode` | text: @Text | 1 |
| `image` | `Doc.InlineImage` | asset: Doc/Asset, alt: Doc/Sentence | 2 |

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

## Doc/Alignment

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `default` | `Doc.AlignmentDefault` | なし | 0 |
| `left` | `Doc.AlignmentLeft` | なし | 0 |
| `center` | `Doc.AlignmentCenter` | なし | 0 |
| `right` | `Doc.AlignmentRight` | なし | 0 |

## Doc/ListStyle

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `unordered` | `Doc.Unordered` | なし | 0 |
| `ordered` | `Doc.Ordered` | start: @Nat | 1 |

## Doc/Check

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `none` | `Doc.CheckNone` | なし | 0 |
| `checked` | `Doc.Checked` | なし | 0 |
| `unchecked` | `Doc.Unchecked` | なし | 0 |

## Doc/OptionalRow

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `none` | `Doc.NoRow` | なし | 0 |
| `some` | `Doc.SomeRow` | row: Doc/Row | 1 |

## Doc/OptionalSentence

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `none` | `Doc.NoSentence` | なし | 0 |
| `some` | `Doc.SomeSentence` | sentence: Doc/Sentence | 1 |

## Doc/OptionalText

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `none` | `Doc.NoText` | なし | 0 |
| `some` | `Doc.SomeText` | text: @Text | 1 |

## Doc/LinkTarget

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `page` | `Doc.PageTarget` | page: @Text, fragment: Doc/OptionalText | 2 |
| `relative` | `Doc.RelativeTarget` | path: @Text, fragment: Doc/OptionalText | 2 |
| `external` | `Doc.ExternalTarget` | uri: @Text | 1 |

## Doc/Asset

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `asset` | `Doc.AssetRef` | id: @Text, digest: Doc/OptionalText | 2 |

## Doc/Row

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `row` | `Doc.Row` | cells: List<Doc/Sentence> | 1 |

## Doc/ListItem

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `item` | `Doc.ListItem` | checked: Doc/Check, body: Doc/Body | 2 |
