# Native composition runtime consumer

This independent workspace will connect the existing MiniExpr and Frame package
definitions to suite operations. The Foundation-only Hello workspace retains
its four Foundation dependencies; this host additionally depends on suite.

## MiniExpr arithmetic contract

The semantic domain is exact signed integers. Natural literals denote their
nonnegative integer payload. `neg x` denotes the additive inverse of x;
`add x y` denotes their sum; `mul x y` denotes their product. No fixed-width
overflow or truncation is permitted. Frame's intended operation returns its
guest expression's value through an explicit operation dependency.

The arithmetic kernel charges Work and AllocationUnits before computation.
For s equal to one plus the sum of operand bit lengths divided by eight, the
logical allowance is 64*s*s Work and 64*s AllocationUnits. Negation uses its
single operand's bit length. These are conservative logical allowances, not
measurements of allocator behavior or a guarantee against physical OOM.
Cancellation and resource exhaustion return a typed StopReason.

Current implementation: typed integer arithmetic and resource-stop tests.
Remaining integration: validated syntax lowering with source attribution,
operation input/output schemas, native Invoke/Await/Resume adapters, grants,
and end-to-end recursive composition. The arithmetic unit test yielding -5
does not establish execution of the source expression through the scheduler.

Run `cargo test --locked --manifest-path conformance/extensions/suite/Cargo.toml`.
WASI uses the same command with `--target wasm32-wasip2` and the Wasmtime runner.
