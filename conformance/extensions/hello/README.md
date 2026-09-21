# External Hello language consumer

This independent Cargo workspace defines `org.example.hello` with the syntax
`hello <name>`. It imports only public core, reader, engine and wire APIs.
No Grammar, Doc, Math, Circuit, suite, tools or private test source is imported.

From the repository root, run:

```sh
python -m unittest tools.extensions.test_run
python tools/extensions/run.py --output dist/external-extension
```

The output directory must not already exist. The runner copies this consumer
outside the repository and resolves its four path dependencies to the checked
foundation tree. It pins the repository toolchain and this consumer's lockfile,
checks the independent workspace and dependency identities, and runs format,
Clippy and the three Rust tests. Logs and source/consumer hashes are retained,
including command failures and partial timeout output.

## What the existing example verifies

Start with `unicode_source_and_external_kind_survive_native_and_ndf` in
[`src/tests.rs`](src/tests.rs). Its `run("hello 世界", true, false)` call parses a
complete input. The expected root is `org.example.hello::Greeting` with one
recipient field; the final token contains `世界`, spans UTF-8 bytes 6..12, and
retains a Direct Origin. No domain-specific evaluator runs.

Run the commands above without editing the consumer or its assertions.
This example currently exposes its behavior through regression tests. An
interactive example that accepts arbitrary input and displays the resulting
tree and diagnostics is not provided yet. Editing test expectations is not
the usage interface.

The other tests distinguish an unknown head (`goodbye 世界`, recovery with an
`UnparsedInput` diagnostic), unfinished input (`hello `, NeedMore) and explicit
cancellation (Stopped). Changing a greeting's recipient cannot introduce a new
head: the form and its one-child shape are defined by `language()` in
[`src/lib.rs`](src/lib.rs).

That function constructs a schema and `LanguagePackage`; `run` resolves a
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
