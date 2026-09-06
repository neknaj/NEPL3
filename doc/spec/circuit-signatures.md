# Circuit：構文signatureの全表

この表は `design/forms.json` と同じ規範データから生成した。`List<T>` は `cons T List<T>` / `nil`、`@` は基礎readerの識別である。

## Circuit/Design

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `design` | `Circuit.Design` | modules: List<Circuit/Module>, entry: @Name, tests: List<Circuit/Test> | 3 |

## Circuit/Module

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `module` | `Circuit.Module` | name: @Name, inputs: List<Circuit/Input>, declarations: List<Circuit/Declaration>, outputs: List<Circuit/Output> | 4 |

## Circuit/Input

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `input` | `Circuit.Input` | name: @Name, width: @Nat | 2 |

## Circuit/Output

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `output` | `Circuit.Output` | name: @Name, width: @Nat, value: Circuit/Expr | 3 |

## Circuit/Declaration

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `wire` | `Circuit.Wire` | name: @Name, value: Circuit/Expr | 2 |
| `state` | `Circuit.State` | name: @Name, width: @Nat, initial: Circuit/Bits | 3 |
| `next` | `Circuit.Next` | name: @Name, value: Circuit/Expr | 2 |
| `inst` | `Circuit.Instance` | name: @Name, module: @Name, inputs: List<Circuit/Expr> | 3 |

## Circuit/Expr

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `and` | `Circuit.And` | left: Circuit/Expr, right: Circuit/Expr | 2 |
| `or` | `Circuit.Or` | left: Circuit/Expr, right: Circuit/Expr | 2 |
| `xor` | `Circuit.Xor` | left: Circuit/Expr, right: Circuit/Expr | 2 |
| `nor` | `Circuit.Nor` | left: Circuit/Expr, right: Circuit/Expr | 2 |
| `add` | `Circuit.Add` | left: Circuit/Expr, right: Circuit/Expr | 2 |
| `concat` | `Circuit.Concat` | left: Circuit/Expr, right: Circuit/Expr | 2 |
| `not` | `Circuit.Not` | value: Circuit/Expr | 1 |
| `mux` | `Circuit.Mux` | select: Circuit/Expr, yes: Circuit/Expr, no: Circuit/Expr | 3 |
| `slice` | `Circuit.Slice` | value: Circuit/Expr, lo: @Nat, width: @Nat | 3 |
| `at` | `Circuit.At` | instance: @Name, output: @Name | 2 |
| `bits` | `Circuit.Bits` | width: @Nat, value: @Nat | 2 |
| `true` | `Circuit.True` | なし | 0 |
| `false` | `Circuit.False` | なし | 0 |

葉の認識規則：`name`。

## Circuit/Bits

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `bits` | `Circuit.Bits` | width: @Nat, value: @Nat | 2 |
| `true` | `Circuit.True` | なし | 0 |
| `false` | `Circuit.False` | なし | 0 |

## Circuit/Test

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `test` | `Circuit.Test` | name: @Name, module: @Name, ticks: List<Circuit/Tick> | 3 |

## Circuit/Tick

| 綴り | kind | 子（順序固定） | arity |
|---|---|---|---:|
| `tick` | `Circuit.Tick` | inputs: List<Circuit/Bits>, outputs: List<Circuit/Bits> | 2 |
