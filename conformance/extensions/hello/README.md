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
