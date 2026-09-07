# Portable CI implementation checkpoint

Implementation: `d84c678443f52eca629a7746cf47f0e137ddd095`, based on
`4ad4d428c09247c3bfeef3925d83a08b97e24ebe`. Local tests used that production
source and the recorded runner revisions during development; the build record
contains the actual source hashes, including uncommitted harness files.

- All eight implemented portable crates compiled for `thumbv6m-none-eabi`.
- Actual RP2040 UF2 executed four production core/wire checks under rp2040js
  1.3.3 and Node 24.14.1. The ELF/UF2 and build record were hash checked.
- Same-artifact Wasmtime 44.0.1 native/Pulley64 comparison: 31 invocations,
  209 production tests per backend. Empty library binaries are recorded empty.
  Raw logs and artifact/runner hashes are in `pulley-execution/`.
- Node runner tests: 25 passed. Python converter/wrapper tests: 7 passed.
  Python comparison regression tests: 5 passed. Repository contracts passed.
- Independent reviews and fault reproduction records are retained separately
  from root execution. The review records define the precise frozen scope;
  an earlier buggy runner's success is a reproduction, not accepted evidence.

The Wasmtime runner was refined during this root Cargo run: final aggregate
validation was rerun across all 31 records. Independent final-runner evidence
is separate. No claim is made that every earlier invocation used the final
runner source. The RP2040 host build emits an upstream `proc-macro-error2`
future incompatibility warning; the pinned Rust build currently succeeds.

Binary artifacts are omitted from Git; hashes are retained and CI uploads the
actual firmware payload. These local records do not represent remote CI,
signed attestations, full acceptance groups, RP2040 boot/peripheral/hardware
validation, browser rendering, RISC-V or big-endian execution. No task or
required acceptance status is promoted by this checkpoint.

Next product work remains Doc HTML generation, page-level nepld migration and
the tested docs site/Pages deployment. Existing Markdown remains the canonical
source until each migration satisfies the formal semantic/link/anchor checks.
