# External Hello language consumer

This independent Cargo workspace defines `org.example.hello` with the syntax
`hello <name>`. It imports only public core, reader, engine and wire APIs.
No Grammar, Doc, Math, Circuit, suite, tools or private test source is imported.

## Observe an input

From the repository root, run:

```sh
cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example inspect -- "hello 世界"
```

The example prints the typed `ParseReply`: the outcome, syntax nodes and fields,
tokens with UTF-8 byte spans, source provenance and diagnostics. For this input,
the outcome is `Complete`, the root kind is `Greeting`, and the final token's
payload is `世界` at bytes `6..12`. Replace the input argument with `hello NEPL3`
to observe the recipient and its span change to `6..11`.

`goodbye 世界` produces `Recovered` with an `UnparsedInput` diagnostic. `hello`
examines a missing child at end of input. To examine an unfinished buffer, pass
`--partial "hello "` after the Cargo `--`; the outcome is `NeedMore`.

The command prints recovery and stop outcomes as data and exits successfully
when observation completes. Argument errors and setup/parse API errors produce
a nonzero exit status. The output uses Rust's diagnostic `Debug` representation;
its layout is intended for inspection. The example performs parsing only.

## Run the external workspace checks

From the repository root, run:

```sh
python -m pip install -r tools/extensions/requirements.txt
python -m unittest tools.extensions.test_run
python tools/extensions/run.py --output dist/external-extension
```

The output directory must not already exist. The runner copies this consumer
outside the repository and resolves its four path dependencies to the checked
foundation tree. It pins the repository toolchain and this consumer's lockfile,
checks the independent workspace and dependency identities, and runs format,
Clippy, the Rust tests, the five Hello inputs above and three MiniExpr inputs below. Logs and source/consumer hashes are retained,
including command failures and partial timeout output.

## Define and inspect recursive expressions

[`src/miniexpr.rs`](src/miniexpr.rs) defines a second package,
`org.example.miniexpr`, through the same public Foundation APIs. Its `Expr`
category accepts natural-number leaves, unary `neg`, and binary `add` and `mul`.
Each form field uses `ReadSpec::Local` to read another `Expr`. The ordered
`Nat`/`Name` readers distinguish numbers from form heads. The Nat reader returns
an `Integer` payload; its lexical contract admits nonnegative decimal integers.
Binding visitors cover the recursive fields explicitly.

```sh
cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example miniexpr -- "add 1 mul 2 3"
```

The result is `Complete; cursor=13`, followed by five syntax nodes. `Add` has
children `Natural(1)` and `Mul`; `Mul` has children `Natural(2)` and `Natural(3)`.
The numeric token spans are `4..5`, `10..11`, and `12..13`.
`source print: Complete("add 1 mul 2 3")` reports the separate printing result.
Try `neg 7`, `neg add 1 2`, and `add 1`. The last input produces recovery and
diagnostics, with no successful print result.

`inspect` parses one expression, validates a complete tree against its resolved
package, and passes that proof to the existing source-backed printer. The
printer retains accepted lexemes and leading trivia. The returned cursor marks
the consumed expression; host-owned trailing input remains outside that output.
Each operation uses its own resource budget. Recovery, unfinished input, and
resource stops remain explicit outcomes. Arithmetic evaluation and semantic
normalization are future operations of the example language.

To change the accepted head, edit the `add` spelling in `language()` to `sum`
and run the example with `sum 1 2`. The `Add` schema and its two fields remain
the same. The regression test constructs this variation separately and verifies
its tree and printed spelling. Restore the declaration when the exercise ends;
the tests retain the standard language's independent expectations.

## Compose two recursive languages

[`src/composition.rs`](src/composition.rs) registers an extended MiniExpr and an
independently identified Frame package in one `ParseProfile`. The composition
variant has schema `org.example.miniexpr.framed`; the standalone MiniExpr schema
and its existing forms remain available. Each package names its foreign category
through `ReadSpec::Foreign`. The host supplies the aliases `Expr` and `Frame`.

```sh
cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example composition -- "add framed frame neg 7 2"
```

`framed` enters the Frame category. Its `frame` head reads an Expr, so `neg 7`
returns to MiniExpr. The final `2` is the second child of the outer `add`.
The result reports `Complete; cursor=24` and three bundle contexts, with paths
identifying both language boundaries. `FrameCode` belongs to Frame; `Code`
belongs to Expr. Source printing preserves the complete accepted input.
The `7` token retains the original source span `21..22` across both boundaries.

Try `framed unknown` and `framed frame`. Each produces `Recovered` with a
diagnostic and no successful print. A recovered foreign root has the engine
recovery schema; the expected guest package/category/mode stays in its
`NodeSelection.entry`. `ForeignSyntax.schema` agrees with its actual root.
Validation checks the recovery record and selected guest context together.
NDF exchange preserves these identities and rejects forged selections.

The runner executes these three composition inputs in the external workspace,
in addition to the Hello and standalone MiniExpr examples. The tests also rename
both host aliases, reject a missing package, and distinguish unfinished input.
The composition uses statically registered packages. Source-level import,
Sentence/annotation integration, evaluation, and independent package distribution
remain subsequent stages of the external-language contract.

The `portable_profile_drives_recursive_parsing` test exchanges the complete
`ParseProfile` through `nepl3_engine::portable::profile` and the NDF wire codec.
The receiver constructs its own package catalog, resolves the received package
identities and operation/resource requirements, and parses normal and recovered
recursive inputs. It compares the full parse reply with the native profile path,
including diagnostics and source positions. Package implementations remain host
registrations; portable `LanguagePackage` loading is a subsequent stage.

## What the existing example verifies

Start with `unicode_source_and_external_kind_survive_native_and_ndf` in
[`src/tests.rs`](src/tests.rs). Its `run("hello 世界", true, false)` call parses a
complete input. The expected root is `org.example.hello::Greeting` with one
recipient field; the final token contains `世界`, spans UTF-8 bytes 6..12, and
retains a Direct Origin. No domain-specific evaluator runs.

Run the checks without editing the consumer or its assertions. The `inspect`
example accepts input independently of these regression expectations.

The other tests distinguish an unknown head (`goodbye 世界`, recovery with an
`UnparsedInput` diagnostic), unfinished input (`hello `, NeedMore) and explicit
cancellation (Stopped). Changing a greeting's recipient cannot introduce a new
head: the form and its one-child shape are defined by `language()` in
[`src/lib.rs`](src/lib.rs).

That function constructs a schema and `LanguagePackage`; the shared parser in
[`src/parse.rs`](src/parse.rs) resolves a
`ParseProfile`, prepares explicit source/environment inputs, and calls
`ParseSession`. The exchange uses the typed portable tree adapters and
`FoundationCodec`. This is a small native public-API consumer, not yet a general
package import/composition tutorial or an implementation of annotation.

Expected Unicode byte positions, the external root kind, recovery, streaming and
cancel behavior are asserted independently of roundtrip equality. Parsed trees
cross the production typed NDF adapters and wire codec; malformed values and
truncated bytes are rejected. The Python tests inject orchestration failures;
they are not execution of the Rust language implementation.

This proves consumption from an external directory, using the current
monorepo's public path dependencies. It does **not** prove an independently
published foundation distribution, SemVer compatibility, portable package
loading or a separate-process operation provider. These remain requirements of
[T26 and the external extension contract](../../../doc/spec/22-external-extensions.md).
