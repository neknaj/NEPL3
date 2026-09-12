# Prepared upload binding

Base: 2a6629b24f97a6683afa4edc4de30e25228611b7. candidate.prepare_upload validates CI and diagnostic ZIP before reading its producer receipt, then verifies the distinct single-tar Pages upload against that receipt and original tar/manifest pins. All network inputs still require host authentication; final freshness, writer state, permissions and live deployment remain separate.

Root related artifact/candidate/upload tests passed and repository check exited 0. Independent 20 tests per mode (normal/-O) and additional receipt/gate probes passed; exact evidence is stored as fixtures. Manifest paths gain .fixture in this archive. No acceptance status or publication success is inferred.
