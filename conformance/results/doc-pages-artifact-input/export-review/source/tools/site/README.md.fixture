# Doc site delivery tools

These host tools support the docs-only site defined in `doc/spec/15-site.md`.
They do not yet form a complete publisher; running an individual tool never
establishes permission, workflow eligibility, public health, or LKG promotion.

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
