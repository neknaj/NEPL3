# Original Pages tar staging

Base: b678330eb043432d46d18d8e6faa037f3589c1de. CI stages the already checked raw tar as artifact.tar and uploads that single file with the pinned action. The new upload ID/outer digest is saved separately from the raw tar/manifest identity. This is artifact upload, not Pages deployment.

Root stage test passed normal/-O; real isolated CLI returned original 1177600 bytes; repository check passed. Independent review and CLI/junction/failure probes are byte-exact fixtures. Manifest paths gain .fixture in this archive. Actual Actions upload and Pages acceptance require remote execution and are not claimed by local tests. No task or acceptance status is promoted.
