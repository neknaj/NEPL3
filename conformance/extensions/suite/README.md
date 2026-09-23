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
syntax views, checked plan transfer and native recursive operation execution.
`execution::Runtime` registers separate MiniExpr and Frame callbacks, admits the
immutable plan and source grants, and runs the existing suite scheduler. Each
occurrence has request ID `index + 1`. MiniExpr evaluates its arithmetic after
dependency completion; Frame returns its guest result. The source-level test
`add framed frame neg 7 2` evaluates to -5 through four Await/Resume generations.
Missing source grants are rejected before dispatch. Every run owns a fresh
lifetime table; continuation reuse across runs is unsupported.
The host computes each provider's context digest with the existing wire context
recipe after grant admission. It binds the plan, source closure and configured
implementation identity. Borrowed scheduler closures reuse these immutable
digests within the run; every child retains the exact same environment and sources.

Scheduler failures expose the active request ID. `Program::request_node` maps
that ID back to its language and original head Span without allocation or a
Budget poll. A child admission/depth failure belongs to the active parent frame;
accepted child reports retain the child's own ID through `accepted_results`.
The example prints this host failure context and accepted child outcomes.
Before `neg`, `add` or `mul` arithmetic, the adapter prepares one owned
`evaluation-stopped` diagnostic with the current selection and head Span. If
arithmetic or output construction stops, it returns that Report without further
allocation. The parent remains suspended and the scheduler retains the child's
validated Report. Preparation consumes Work, AllocationUnits and one diagnostic
slot even when evaluation succeeds; all charges remain cumulative. A stop during
preparation uses the host failure context. Frame forwarding and request preparation
also retain that host context rather than constructing a Report after exhaustion.

`Runtime::prepare` creates an immutable `Session` for one schema-checked plan,
source closure, registry and executable identity pair. `Session::with_registrations`
borrows the same Invoke/Resume implementations for native scheduling or checked
provider dispatch. The host retains request lifetimes and cumulative execution
budgets. Grants require the bound plan identity; callbacks require the complete
ordered source closure before returning a continuation with its cached context.
Native Invoke and Resume callbacks borrow this immutable plan.
Every returned Complete, Invalid, Stopped and Await records the cumulative Usage
of its execution Budget, including charges made before entering the callback.
Native scheduling shares one Budget across the plan. In the process fixture,
each host retains its own execution Budget; remote reports preserve that host's
local cumulative Usage. Receiving a report does not charge the parent's execution
Budget. These per-host reports do not establish a combined process-wide limit.
The request environment carries a `PlanIdentity` containing the digest of the
canonical NDF plan. Child requests copy this fixed-size identity and select one
occurrence; they share the plan through the callback's Rust lifetime. Plan encoding,
validation and hashing occur once per run. The portable plan representation remains
available for a future process provider's admission path.
`transfer::Checked::program` converts a received, schema-checked plan into the
same typed execution model. The host supplies its admitted source-head mapping
in occurrence order; snapshot identity and bounds are checked before use.
The conversion borrows numeric payloads and spans. `transfer::envelope` carries
the plan and occurrence-ordered Span payloads in one schema-checked NDF packet,
using the Foundation Span codec. Decode checks the plan graph, mapping length
and receiving host's source permissions. `Received` owns the immutable packet;
its typed Program borrows admitted payloads. Source correspondence is a host
responsibility. Tests exercise
the actual NDF codec, native evaluation and rejection
of missing source permissions, mismatched mapping length and allocation stops.
Remaining integration: additional failure and large-input cost cases, general
distributed scheduling and independent review. Regression tests cover successful
nesting depths 1, 8 and 24, Depth=1 rejection, and sampled Work/Allocation stops.
They check unique cancellation and preserve inspectable accepted child outcomes.

Run the existing MiniExpr/Frame syntax through the evaluator:

```sh
cargo run --locked --manifest-path conformance/extensions/suite/Cargo.toml --example evaluate -- "add framed frame neg 7 2"
```

Standard output is `-5`. Standard error shows Await request IDs `6`, `4`, `3`,
`2`, corresponding to Add, Framed, Frame and Neg. Try `mul neg 3 add 4 2` to
obtain `-18`. An incomplete input such as `add 1` exits unsuccessfully before
evaluation. The example requires a complete parse and has no source-level import.

Run `cargo test --locked --manifest-path conformance/extensions/suite/Cargo.toml`.
WASI uses the same command with `--target wasm32-wasip2` and the Wasmtime runner.
CI runs this independent workspace on all three native hosts and WASI.

The native `process` test transfers the envelope in an Invoke frame through the
existing provider's stdin/stdout transport. A separate process validates the
operation, host-selected source grants and envelope, then evaluates the admitted
plan through its native scheduler. The parent validates the reply and compares
its Value with both native execution and independent arithmetic expectations
(`-5` and `-18`). Corrupt packets, missing source grants and an unexpected
operation must close the child without a result; the test checks the specific
failure diagnostic and unsuccessful exit. The parent enforces a 30-second
deadline and terminates/reaps the direct child on failure.

Run this stage independently with:

```sh
cargo test --locked --manifest-path conformance/extensions/suite/Cargo.toml --test process
```

The aggregate cases use a test-only operation with a fixed, host-authorized
fixture source. Additional cases independently install the same fixture plan in
both hosts and dispatch the actual MiniExpr root in the child process. Its Await
dependencies execute through the parent's native scheduler; their checked results
return in Resume. Both arithmetic examples agree with the native reference.
Explicit cancellation and a Work-stopped guest multiplication cancel the remote
parent without Resume. The latter retains the host's accepted child Report with
its own operation, request ID and `mul` source range `17..20`. Close produces no
duplicate cancellation notification in the child.

A further case resumes a remote multiplication with completed literal operands.
Its execution Work limit stops the multiplication in the child process. The
parent receives the validated Stopped result, checks the root operation and input,
the `mul` source range `0..3`, and the absence of a partial value. Both hosts mark
the root Finished; Close produces no cancellation notification. Remote Usage
remains a claim carried by the Report, and the parent's local Usage is preserved.

The process harness has ten cases. It covers a remote root and native dependency
subtrees. General routing across multiple providers, transferring stopped child
outcomes between processes, dynamic package loading, process-tree containment and
cross-process cumulative resource accounting remain unimplemented. Host protocol
and validation failures terminate the fixture process. WASI explicitly skips this
OS process harness while executing the portable consumer tests.
