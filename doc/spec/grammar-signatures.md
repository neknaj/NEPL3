# Grammar：構文signatureの全表

この表は `design/forms.json` と同じ規範データから生成した。`List<T>` は `cons T List<T>` / `nil`、`@` は基礎readerの識別である。

## Grammar/Root

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `language` | `Grammar.Language` | name: @Name, revision: @Nat, root: @Name, declarations: List<Grammar/Declaration> | 4 |

## Grammar/Declaration

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `category` | `Grammar.Category` | name: @Name, mode: @Name | 2 |
| `namespace` | `Grammar.Namespace` | name: @Name, policy: Grammar/Policy | 2 |
| `reader` | `Grammar.Reader` | name: @Name, expression: Grammar/ReaderExpr | 2 |
| `mode` | `Grammar.Mode` | name: @Name, rules: List<Grammar/LexRule> | 2 |
| `form` | `Grammar.Form` | kind: @Name, category: @Name, spelling: @Text, fields: List<Grammar/Field>, bindings: Grammar/Binding, styles: List<Grammar/Style> | 6 |
| `leaf` | `Grammar.Leaf` | kind: @Name, category: @Name, token: @Name, bindings: Grammar/Binding, styles: List<Grammar/Style> | 5 |
| `extension` | `Grammar.Extension` | alias: @Name, provider: @Text, signature: @Text | 3 |

## Grammar/Policy

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `lexical` | `Grammar.Lexical` | なし | 0 |
| `global` | `Grammar.Global` | なし | 0 |
| `open` | `Grammar.Open` | なし | 0 |

## Grammar/Field

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `field` | `Grammar.FieldDeclaration` | name: @Name, read: Grammar/ReadSpec | 2 |

## Grammar/ReadSpec

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `local` | `Grammar.Local` | category: @Name | 1 |
| `foreign` | `Grammar.Foreign` | alias: @Name, category: @Name | 2 |
| `listof` | `Grammar.ListOf` | element: Grammar/ReadSpec | 1 |
| `withmode` | `Grammar.WithMode` | mode: @Name, read: Grammar/ReadSpec | 2 |
| `builtin` | `Grammar.Builtin` | reader: @Name | 1 |

## Grammar/LexRule

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `take` | `Grammar.Take` | kind: @Name, reader: @Name | 2 |
| `skip` | `Grammar.Skip` | reader: @Name | 1 |

## Grammar/ReaderExpr

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `literal` | `Grammar.Literal` | text: @Text | 1 |
| `scalar` | `Grammar.Scalar` | class: Grammar/CharClass | 1 |
| `seq` | `Grammar.Seq` | parts: List<Grammar/ReaderExpr> | 1 |
| `choice` | `Grammar.Choice` | alternatives: List<Grammar/ReaderExpr> | 1 |
| `many` | `Grammar.Many` | body: Grammar/ReaderExpr | 1 |
| `some` | `Grammar.Some` | body: Grammar/ReaderExpr | 1 |
| `optional` | `Grammar.Optional` | body: Grammar/ReaderExpr | 1 |
| `repeat` | `Grammar.Repeat` | min: @Nat, max: @Nat, body: Grammar/ReaderExpr | 3 |
| `look` | `Grammar.Look` | body: Grammar/ReaderExpr | 1 |
| `not` | `Grammar.Not` | body: Grammar/ReaderExpr | 1 |
| `commit` | `Grammar.Commit` | body: Grammar/ReaderExpr | 1 |
| `capture` | `Grammar.Capture` | name: @Name, body: Grammar/ReaderExpr | 2 |
| `region` | `Grammar.Region` | role: @Name, body: Grammar/ReaderExpr | 2 |
| `node` | `Grammar.Node` | kind: @Name, body: Grammar/ReaderExpr | 2 |
| `discard` | `Grammar.Discard` | body: Grammar/ReaderExpr | 1 |
| `ref` | `Grammar.Ref` | name: @Name | 1 |
| `decode` | `Grammar.Decode` | decoder: @Text, body: Grammar/ReaderExpr | 2 |
| `map` | `Grammar.Map` | provider: @Text, body: Grammar/ReaderExpr | 2 |
| `then` | `Grammar.Then` | first: Grammar/ReaderExpr, provider: @Text | 2 |
| `call` | `Grammar.Call` | provider: @Text | 1 |
| `eof` | `Grammar.Eof` | なし | 0 |
| `takecount` | `Grammar.Takecount` | count: @Nat | 1 |
| `until` | `Grammar.Until` | delimiter: @Text | 1 |

## Grammar/CharClass

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `any` | `Grammar.Any` | なし | 0 |
| `whitespace` | `Grammar.Whitespace` | なし | 0 |
| `identifierStart` | `Grammar.Identifierstart` | なし | 0 |
| `identifierContinue` | `Grammar.Identifiercontinue` | なし | 0 |
| `digit` | `Grammar.Digit` | なし | 0 |
| `asciiLetter` | `Grammar.Asciiletter` | なし | 0 |
| `chars` | `Grammar.Chars` | text: @Text | 1 |
| `except` | `Grammar.Except` | text: @Text | 1 |
| `range` | `Grammar.Range` | lo: @Text, hi: @Text | 2 |

## Grammar/Binding

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `visit` | `Grammar.Visit` | child: @Name | 1 |
| `import` | `Grammar.Import` | child: @Name | 1 |
| `propagate` | `Grammar.Propagate` | child: @Name | 1 |
| `scope` | `Grammar.Scope` | plans: List<Grammar/Binding> | 1 |
| `group` | `Grammar.Group` | plans: List<Grammar/Binding> | 1 |
| `bind` | `Grammar.Bind` | namespace: @Name, field: @Name | 2 |
| `reference` | `Grammar.Reference` | namespace: @Name, field: @Name | 2 |
| `export` | `Grammar.Export` | namespace: @Name, field: @Name | 2 |
| `sequential` | `Grammar.Sequential` | declarations: @Name, body: @Name | 2 |
| `recursive` | `Grammar.Recursive` | declarations: @Name, body: @Name | 2 |
| `none` | `Grammar.None` | なし | 0 |
| `custom` | `Grammar.Custom` | provider: @Text | 1 |

## Grammar/Style

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `style` | `Grammar.Style` | selector: Grammar/Selector, class: @Text | 2 |

## Grammar/Selector

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `head` | `Grammar.Head` | なし | 0 |
| `self` | `Grammar.Self` | なし | 0 |
| `field` | `Grammar.FieldSelector` | name: @Name | 1 |
| `capture` | `Grammar.CaptureSelector` | name: @Name | 1 |
