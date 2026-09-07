# Independent RP2040 wrapper / CI final review

No additional blocking request in this delta. Production/Git were not changed. Final copied source hashes and absolute copy paths: final-files.json. Review scope is RP2040 adapter/build wrapper and its CI wiring; concurrent Pulley code is excluded except whole-YAML syntax lint. Prior four-case firmware review remains separate.

## Findings fixed

- Stale emulator evidence: originally, a real exit-zero Node child writing nothing reused old emulator.json and returned passed. Final wrapper clears old records and uses a unique TemporaryDirectory output. The same scenario now exits 1, writes failed execution and leaves no old emulator file; real execution still passes.
- Non-object JSON: build.json=[] originally escaped with TypeError and retained old execution.json=passed, although process exit was 1. Final object checks and initial output cleanup fix this. Actual non-object build and fresh non-object child evidence both now write failed execution and exit 1.
- Build identity: latest build() removes old build record and compares input hashes before/after compilation. Static review and a mocked source-input probe confirm stable inputs publish a record, changed inputs raise and leave no old build.json. This final build-path check mocks the compiler/metadata command; it is not a second real firmware build. Input stability is checked at those two observations, not an atomic immutable filesystem snapshot or signed attestation.

## Executed checks

- Final seven Python converter/wrapper tests passed.
- Actual root ELF/UF2/build bundle executed successfully through copied wrapper. Final actual-process negative cases: zero child/no new file, child exit3, malformed build JSON, fresh malformed child JSON all exit1/failed. Actual ELF and UF2 corruption were rejected by hash checks.
- Final wrapper ran a busy-loop Node with the unchanged 30-second timeout: 30.129 seconds elapsed, exit1/failed, PID 38596 absent after return, old emulator file absent. Logs retained. This verifies direct-child kill/reap; it does not claim process-tree termination or interruption of process creation.
- Changed/stable build-input mock probe passed both paths. Final wrapper source at SHA bab29c4ba111642c032436ed494863a2dcc9259e88829da3b339b77e69880d0f preserves the already exercised execute() body and adds the checked build-input behavior.
- Final copied YAML passed actionlint 1.7.12 with shellcheck/pyflakes disabled. Actual remote CI/artifact download was not executed independently.

## CI / scope

Build and execution jobs share github.sha artifact name and same workflow run; execution downloads ELF/UF2/build record and does not rebuild. Official pinned upload default archive=true retains multiple files; download default digest-mismatch=error fails corruption. Node24.14.1, npm ci --ignore-scripts, readonly contents, persist-credentials=false, always-upload evidence and quality requiring both jobs are correctly wired. The new thumb check lists all eight implemented portable core crates with no-default-features. This compilation is distinct from the four core/wire runtime tests; reader/engine complete MCU suites, all-language acceptance, physical boot/timing and Pulley are not claimed. Runtime/build records link hashes but are not authentication of arbitrary externally supplied local provenance.

Selected evidence: final-managed.log, final-results.json, final-positive.log, final-stale-zero.log, final-stale-nonzero.log, final-bad-build.log, final-bad-evidence.log, final-timeout-observation.json, final-timeout.log, build-input-probe.py/log, actionlint.log. Before evidence: stale-zero.log, bad-build-shape.log and matching firmware-before.py. Large binaries/UF2 are unnecessary for archiving this wrapper record.

Primary sources: https://docs.python.org/3/library/subprocess.html#subprocess.run ; https://raw.githubusercontent.com/actions/upload-artifact/043fb46d1a93c77aae656e7c1c64a875d1fc6a0a/action.yml ; https://raw.githubusercontent.com/actions/download-artifact/3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c/action.yml .
