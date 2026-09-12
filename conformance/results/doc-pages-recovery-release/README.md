# Recovery release storage verification

Implementation bfff8b3. Four tests passed independently normally and optimized;
23 additional cases probe missing metadata, altered bytes, typed pin constraints
and size limits. Source snapshots match implementation Git blobs. The root full
site suite and repository checker passed; raw commands/logs are included.
This tests storage metadata and downloaded-byte matching only. Authenticated
transport, actual immutability, tag resolution, tar/identity/smoke validation
and LKG promotion are not established by these fixtures.
