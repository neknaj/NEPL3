# Doc site delivery tools

These host tools support the docs-only site defined in `doc/spec/15-site.md`.
They do not yet form a complete publisher; running an individual tool never
establishes permission, workflow eligibility, public health, or LKG promotion.

`deployment.submit` checks the full recorded history for creation intents without
an immediately matching receipt and valid original API response. Changing a
transaction name or switching between deployment and recovery does not bypass
that check. An unresolved intent must be reconciled through the writer protocol;
creating a new transaction is not reconciliation. Having a receipt only proves
that the creation response was recorded. Completion, current publication,
recovery eligibility and LKG persistence remain separate publisher requirements.

## Select the original tar from an Actions download

After authenticating and saving Actions artifact metadata and its ZIP download,
use `python -I tools/site/artifact.py --help` for the input contract. Supply the
metadata path, archive path and a new output path, followed by all of:

- `--owner`, `--repository`, `--repository-id`
- `--artifact-id`, `--run-id`, `--source-commit`
- `--expected-tar`, `--expected-manifest` (lowercase SHA-256 values)

Select those identities from the publisher's trusted verification context, not
from the archive being checked. The command validates metadata, ZIP bytes, the
internal packaging receipt, raw tar and source identity before writing output.
It writes the original raw tar without extracting the ZIP or regenerating HTML.
The output parent must already exist without symlink/reparse ancestors; output
must not already exist. Successful stdout is a JSON packaging receipt with
`publication_verified: false`. Errors return nonzero. An I/O failure during the
write can leave an incomplete output file; it must not be uploaded or treated as
a successful artifact. A subsequent invocation will refuse to overwrite it.

The containing `doc-browser-<commit>` artifact includes diagnostic material and
is **not** the Pages artifact ID. The selected tar must be uploaded unchanged in
the format required by Pages, with the new upload identity separately verified.
The publisher must verify workflow/run attempt and required checks, own the
writer lock, and reconcile journal/API/public state before any mutation.

Validation:

```sh
python -m unittest discover -s tools/site -p 'test_artifact*.py' -v
python -O -m unittest discover -s tools/site -p 'test_artifact*.py' -v
```

The command accepts bounded local inputs; it does not download credentials or
enforce an OS process memory/time limit. Execute it within the bounded publishing
job. Formal acceptance and actual Pages deployment remain separate.

## Responsibility and current limits

The failure-mode/ownership decision is [ADR 0008](../../doc/decisions/0008-evidence-and-publisher-boundaries.md).
Payload validation owns content/byte identity; Actions owns the job graph,
artifact transfer and same-group execution scheduling. Neither an artifact
upload nor a deployment receipt proves public health or LKG. This directory's
`test_*.py` are host regression tests, not publication commands or review evidence.
Their managed real-document fixture is in `tools/site/fixtures/`.

Full protocol tests run once in `site-publication`; native jobs retain the
filesystem, socket and isolated-process portability tests. Browser jobs retain
real rendering and payload integration. `quality` requires every lane. These
are scoped implementation checks, not T20/S06 acceptance or a live deployment.
Use the common command collector for new review/test records. Do not add
per-review runners, source snapshots, or new transport layers to this subsystem.
Publisher expansion is paused while core/language work resumes after the
boundary correction; outstanding publication and recovery requirements remain.
