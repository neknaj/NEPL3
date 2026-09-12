# macOS smoke fixture path correction

Base: 8778dbc6b7d31ec2be6cd28552e4d46179d81ecf.
The macOS CI job 103572795911 failed with two errors and two failures because the temporary site path contained a linked ancestor. Only the test fixture root is resolved before construction; production path validation is unchanged.

Root validation on Windows:
- python -m unittest discover -s tools/site -p test_record_smoke.py -v: 5 passed, 134.438 seconds.
- python -m unittest discover -s tools/site -p test_payload.py -v: 7 passed.
- cargo run --locked -p nepl3-tools -- check: exit 0; runtime acceptance not run.
- git diff --check: passed.

Independent review and a real junction/temporary bare Git probe are preserved as byte-exact .fixture files. Manifest paths acquire the .fixture suffix in this archive. The probe uses an injected HTTP report, not live Pages. macOS rerun and actual publication remain unverified at this checkpoint. No task or acceptance status is promoted.
