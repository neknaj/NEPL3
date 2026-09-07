# Independent RP2040 adapter review (four-case firmware snapshot)

Production/Git edits: none. Fixed firmware source and original adapter are under original/; updated adapter and WFI halt delta are indexed by reviewed-files.json. Foundation dependencies were read from the root worktree and hashed in foundation-inputs.json. Compiler was the repository Rust 1.97 toolchain; Node v24.14.1 / installed rp2040js 1.3.3. Emulator installed files are hashed separately.

## Findings and disposition

The initially read allocator formed &mut State and &mut bytes across live allocations. This was reported as a static unsafe-aliasing defect; root had already replaced it by raw addr_of_mut field accesses when the first file copy was made. No original-byte or Miri reproduction is claimed. Fixed code never forms references to live heap storage, uses checked alignment and bounds, updates only a disjoint cursor, and returns buffer-derived pointers. Single-core/interrupt-disabled, no-reset/no-deallocation constraints remain required. Native adapted allocator probe retained a live allocation across 52 aligned/nonoverlapping allocations and rejected exhaustion without advancing cursor. This runtime check is not a proof of all unsafe aliasing rules. Miri is unavailable in installed toolchains.

Original runner stopped immediately at END and wrote emulator version as a constant. Updated runner checks installed version and Node, records permitted startup peripheral warnings, rejects warning/error after BEGIN, and requires terminal CPU wait after END. Actual firmware faults now reject trailing PANIC, missing END, and allocator exhaustion. These follow-up points are resolved in the reviewed updated snapshot. Wrong installed version 1.3.4 in a scratch-only package JSON was rejected with exit 1 and no evidence; original file restored.

## Actual positive evidence

Initial root ELF copied independently, converted and executed: four real core/wire cases PASS, 92183 instructions, UF2 c19bc19fe4c8df129953db7289c543b70dfe685d21b8c0785891883703ae7f8c. This initial ELF is distinct from independently rebuilt source.

Fixed allocator source independently built with cargo build --locked --release --target thumbv6m-none-eabi, converted by frozen ELF32 converter, and executed: 92144 instructions, UF2 e6548125bd168ce92ec7fbb380aedf7b7733b75d0a8b0fecb0b611bb1c44593c. Updated WFI halt + version/warning/terminal-aware runner independently rebuilt/executed: four PASS, 92154 instructions, UF2 36d042d45ebec8299d25c39d0067f986c421043f858e2fd7342d59e3fbf2cc11. Each evidence records the actual file hash used.

## Boundaries

25 author-managed Node tests and 30 independent UF2/UART checks passed. Checked magic/flags/family/payload size/block ordinal/count, overlap, flash range/alignment, missing image/vector, SP range/alignment, mapped Thumb PC. UART cases cover BEGIN/END/newline absence, failed/duplicate/unknown IDs, PANIC, trailing bytes, NUL/non-ASCII and line limits.

Three independently rebuilt faulty firmware variants all exit 1: END followed by PANIC -> UART protocol bytes; END omitted then WFI -> unexpected CPU wait; a real 128KiB allocation in 64KiB heap -> panic rejected (not a logical NEPL3 Budget stop). Per-variant sources, UF2 files, build logs and run evidence are retained. The normal source was restored afterward; the last target binary is the fault variant and is not the positive UF2 artifact.

Two scratch-only copies of original runner lowered limits: 8192 instruction cap -> failed at 8193 with instruction limit; zero wall cap -> failed at check 4096 with wall timeout. These verify runner failure branches, not external watchdog behavior or a hard wall bound inside a hung host instruction. Normal caps were not increased.

Boot2 page checksum in the rebuilt real UF2 matches Pico SDK pad_checksum with the actual CMake seed 0xffffffff, checksum 0x5d4c22ea. The first reviewer CRC calculation used the standalone script default seed 0, failed, then was corrected after reading the SDK CMake invocation; this was a reviewer parameter error, not firmware defect. No boot-ROM or boot2 execution was tested. Vector entry SP 0x20042000 / Thumb PC were loaded from actual UF2. UF2 strict transport profile is narrower than general UF2 ordering/flags; no general converter compatibility claim.

## Remaining scope

This review covers the initial four real core/wire runtime cases. Reader/engine are dependencies compiled for thumbv6m, not executed by this CASES list. Later added cases, CLI/evidence verifier, CI wiring, dependency/task/acceptance changes are not yet reviewed. Emulator emits PLL/UART unimplemented-peripheral startup warnings now recorded; no physical timing, baud/electrical behavior, board boot, multicore or interrupt concurrency acceptance is claimed. No additional blocker in the reviewed four-case code; later final snapshot still requires final-scope review.

## Primary sources

https://doc.rust-lang.org/std/cell/struct.UnsafeCell.html#aliasing-rules
https://github.com/microsoft/uf2
https://raw.githubusercontent.com/wokwi/rp2040js/v1.3.3/src/simulator.ts
https://raw.githubusercontent.com/raspberrypi/pico-sdk/master/src/rp2040/boot_stage2/pad_checksum
https://raw.githubusercontent.com/raspberrypi/pico-sdk/master/src/rp2040/boot_stage2/CMakeLists.txt

Prior memory only guided checking vector initialization; current pinned code/actual UF2 and official sources were checked independently. No old project success is reused as NEPL3 evidence.
