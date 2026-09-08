# First canonical Doc specification page

This records the implementation and review of chapter 13's source selection at
`152c7d485aeb7339b0bc8448bef09bca40837a03`, based on
`0d80fe6289346a1b8481b03b149d5b35af90d68d`. The authored Doc moves unchanged into
`doc/spec/13-reproducibility.nepld`; the existing Markdown URL becomes its generated
all-notes view. `doc/canonical.json` selects this source for exact projection checks
and production HTML export. Other pages remain Markdown-canonical.

The main agent implemented the registry, bounded input validation, shared projection
generation, CLI and CI stages. Huygens independently reviewed the code and reproduced
two Windows path defects before their correction: a trailing-dot directory alias and
a `#` source accepted by projection checks but rejected by HTML generation. The same
probes reject both after the fix. The reviewed path policy belongs to the repository
host, not to Unicode Doc content or foundation Source URIs.

Carson manually synchronized four affected Doc drafts. Kepler separately reviewed
their complete deltas against the current Markdown and authoring policy. The initial
omission of the path-policy paragraph is retained and its correction verified. These
draft reviews do not claim that those four pages have switched canonical source.

- Windows native: 70 tool unit tests passed, one existing test ignored; 76 Doc
  integration tests passed; workspace Clippy, format and repository checks passed.
  The first repository check preceded staging the moved source and failed; the
  corrected staged-input check is retained separately.
- Independent implementation review: the original path probes, 114 extra boundary
  cases, five focused tests and real chapter export passed after correction. Unix
  symlink execution was left to the actual native CI lanes, not inferred on Windows.
- Markdown and complete HTML/CSS/manifest were each generated twice with identical
  bytes. The root input record and independent review bind the production sources,
  schemas, checked bootstrap inputs, toolchain, executable digest and source bytes.
- GitHub's real final Markdown path retains eight legacy fragments and seven
  semantic Section IDs. The first root viewport-only check was too weak to establish
  completed scrolling. Independent opposite-end navigation with stable-scroll
  checks confirms all 15; the final section correctly stops at the document end.
- Local static HTML in Chromium, Firefox and WebKit, JavaScript disabled, was checked
  at two widths and two nonroot prefixes. The strengthened 84 fragment navigations
  were independently repeated. Display, Ruby, headings, CSS and href checks passed.
  Accessibility tree snapshots are automated evidence, not human screen-reader use.
- Windows WebKit does not Tab to these links. Independent plain-link and form-control
  comparisons reproduce the behavior without Doc markup or CSS; pinned upstream
  platform preferences support an embedder-policy explanation. The failure is
  retained, not reported as keyboard success. Chromium/Firefox traversal passed;
  no script permission or artificial tabindex was added to the document.

This is the first page cutover, not T21, J01-J04, full foundation distribution,
Playground, human assistive-technology acceptance or GitHub Pages deployment.
GitHub CI receipts for subsequent evidence-only commits are separate from the
source and local executions recorded here.

`payloads.json` binds each original byte length, SHA-256 and restoration path.
`.fixture` suffixes prevent historical source fragments, failed probes and old
Markdown links from becoming current repository contracts. Each owner's manifest
also remains intact. The `root/snapshot/` payloads are the actual generated HTML,
stylesheet and manifest; they can be read with no Rust compiler or JavaScript.
The exact restoration snippet below was executed; the restored files matched the
original bytes and opened as file URLs in all three engines with JavaScript off.
The initial probe selected the palette's third background instead of the primary
background; its failed expectation and correction remain separate from production.

From the repository root, restore only that snapshot to a new directory:

```python
from pathlib import Path
import hashlib, json

source = Path("conformance/results/doc-canonical-source")
output = Path("dist/canonical-snapshot")
output.mkdir(parents=True)  # refuse an existing snapshot
for item in json.loads((source / "payloads.json").read_text(encoding="utf-8"))["payloads"]:
    prefix = "root/snapshot/"
    if not item["restore"].startswith(prefix):
        continue
    data = (source / item["path"]).read_bytes()
    assert len(data) == item["bytes"]
    assert hashlib.sha256(data).hexdigest() == item["sha256"]
    relative = Path(item["restore"][len(prefix):])
    assert not relative.is_absolute() and ".." not in relative.parts
    target = output / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    with target.open("xb") as stream:
        stream.write(data)
```

Open `dist/canonical-snapshot/docs/spec/13-reproducibility.html`. The stylesheet is
relative to that page; the two RFC links retain their external destinations.
