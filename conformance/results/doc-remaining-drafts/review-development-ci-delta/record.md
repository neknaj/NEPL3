# Independent development CI scope correction and draft follow-up

Reviewed root source change 337057d71838c85a6fb9a304e3ec4164436bb310 and author B draft follow-up 3887fffc9fc08ae449a374bb97ff301db7786df1, against their actual parents. No production or manuscript edits were made.

No additional blocking finding. The Markdown changes exactly one paragraph and no other path. The authored follow-up changes exactly the matching Paragraph. Its complete base text equals the new source paragraph; substituting the previous paragraph restores the complete earlier manuscript structure, including all other Ruby, code, links and examples. This resolves the stale portable-status hold recorded for the earlier fixed draft 446000e, without changing that historical review record.

Direct reading of the fixed CI workflow confirms actual configured WASI tests, browser-target compile checks, ARMv6-M compile checks, separately built/transferred RP2040 firmware and emulator execution, and the quality job requiring native/WASI/build/emulator results. The corrected paragraph distinguishes those foundation scopes from completed WASI CLI, LSP, operation-provider and Web Playground product entrypoints. It also explicitly distinguishes browser compilation from browser execution and retains the release requirement for relevant conformance evidence. It does not broaden the test scope or declare those product entrypoints complete.

The changed draft prose and Ruby were read directly. The new Kanji-only annotations preserve words and okurigana, use separate natural Sentences, and leave the release condition intact. Source/draft hashes, fixed workflow and executable static assertions are retained in this archive.

This is a source/draft content and CI-configuration review. No runner, runtime, deployment or canonical migration was executed, and the workflow's presence is not counted as a new successful CI run. Runtime tests need not be repeated for this paragraph-only correction; root remains responsible for current implementation evidence and canonical status.
