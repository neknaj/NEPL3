# Actions artifact input validation

Base: e180e684af3d1e21b7523d2b9ea9f651633338c3.

`tools/site/deployment/artifact.py` binds supplied Actions metadata and downloaded
ZIP bytes to selected artifact/run/repository/source identities and separately
provided tar/manifest pins. It returns the original validated tar bytes without
filesystem extraction. The containing diagnostics artifact is not a Pages upload.

The caller must authenticate the metadata/download, establish the eligible
workflow and run attempt, verify required checks, and supply trusted pins. This
module does not implement those gates or perform a deployment. ZIP parsing has
bounded input bytes and declared expanded sizes; its central directory is parsed
before the entry-count check. An execution host must also bound time/memory.

Root validation:
- `python -m unittest discover -s tools/site -p test_artifact.py -v`: 6 passed.
- The same command with `python -O`: 6 passed.
- `cargo run --locked -p nepl3-tools -- check`: exit 0; runtime acceptance not run.
- `git diff --check`: passed.

The positive vector uses the previously archived real 14-chapter Doc tar and
pre-existing expected hashes. Failures cover metadata substitution, wrong source,
archive corruption, receipt mismatch, links, conflicting paths and member kinds.
Independent review found two ZIP path/kind cases initially accepted; both are
now rejected and retained as regression tests.

Read-only interoperability: Actions run 34700607169, artifact 10300062947,
source a4eaaa013888ec388d48311ac26ed7dda813989a. The downloaded ZIP size of
455286 bytes and SHA-256 matched API metadata, and yielded the original
1177600-byte tar with 22 files. Pins in this interoperability check came from the
downloaded receipt, so this is not an independent eligibility or publication proof.

API reference: https://docs.github.com/en/rest/actions/artifacts
No task or acceptance state is promoted. Live publishing remains unimplemented.
