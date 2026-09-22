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

The separate `org.example.miniexpr.operations` schema defines pure `neg`, `add`
and `mul` operations. `Unary.value` and `Binary.left/right` are Integer fields;
the output is `Value.value: Integer`. The environment record has no fields.
`native::Arithmetic` registers this descriptor and provides terminal callbacks
bound to exact OperationRefs and a host-supplied implementation identity.
The host finalizes the registry and authorizes sources/resources before dispatch.

`syntax::Cursor` borrows a validated ParseTree and classifies one node at a time.
Its typed Expression variants retain local child references, foreign-language
boundaries and borrowed head spans. Complete schema identities supplied by the
host bind the view to the selected packages. Recovered trees and mismatched
identities are rejected. Traversal charges Work; the eventual execution adapter
must additionally account for its retained state and dependency depth.

`program::compile` converts those views into an immutable dependency plan using
an explicit work stack. Natural values and head spans remain borrowed. Each
consumer references its earlier child ValueIds; Frame and Framed remain distinct
language-owned operations. Each syntax occurrence is charged separately, including
repeated references. Work, AllocationUnits, Nodes and logical Depth bound planning.

`program::transfer` represents the plan as a flat, typed `Plan`/`Node` schema.
The decoder validates exact schema identity, nonnegative literal values,
postorder child references, single-parent occurrences and language boundaries.
The root is the last MiniExpr node. Disconnected nodes, shared occurrence IDs
and cycles are rejected before scheduling. Source spans remain in the host's
immutable Program and correspond to the same occurrence indices. This transport
value carries no source authority; the execution adapter must retain that mapping
and supply independently authorized source snapshots.

Current implementation: typed integer arithmetic, operation schemas, borrowed
syntax views, checked plan transfer and native terminal dispatch with input/output validation.
Remaining integration: plan-to-operation scheduling, Await/Resume adapters, grants, and
end-to-end recursive composition. The terminal dispatch test yielding -5 covers
an explicit binary request. Source-level MiniExpr/Frame evaluation remains open.

Run `cargo test --locked --manifest-path conformance/extensions/suite/Cargo.toml`.
WASI uses the same command with `--target wasm32-wasip2` and the Wasmtime runner.
CI runs this independent workspace on all three native hosts and WASI.
