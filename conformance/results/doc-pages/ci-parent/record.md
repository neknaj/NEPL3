# Independent workflow parent-directory correction review

Reviewed the working-tree diff against commit 64970b2: the only functional change is converting the linked Doc candidate step to a multiline run and adding mkdir -p dist immediately before the unchanged cargo run command.

The workflow defaults to bash on all native matrix OSes. The previous mkdir -p dist existed only in Export the real Doc example, guarded by ubuntu-latest. The candidate step runs on Ubuntu, Windows, and macOS. Therefore it incorrectly relied on a skipped earlier step to create its parent directory.

The supplied Windows job log identifies Git Bash -e -o pipefail, the unchanged pages command, and os error 3 at lines 1432-1448. Production pages::write calls create_dir(output), intentionally requiring an existing parent and a new output directory. The new unconditional parent creation resolves this dependency while retaining the new-child/no-overwrite contract. mkdir -p dist does not create dist/doc-migration, does not overwrite content, and does not turn failed generation into success. No permissions, upload condition, backend, limits, or acceptance conditions change.

No blocking finding in this diff. Root owns the before/after actual CLI reproduction. This review is static source and existing CI-log verification; it does not claim the next remote workflow has passed or independently confirm the macOS failure log.
