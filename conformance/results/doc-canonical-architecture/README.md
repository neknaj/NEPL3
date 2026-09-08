# Chapter 01 canonical Doc source

Source checkpoint: `9c5883b555ebe5e09380ea4ab20ce275d0687259`.
Integration-test fixture correction: `652347cb5a37ea341e5ee67887f61ccf11692cee`.
PR #91 was integrated at `126ac1f5d1c577aa3bd5b3a6d494afebe203518e`;
the cutover sources and generation implementation are unchanged by that merge.
The architecture chapter is registered as a Doc source with the explicit
page-context Markdown profile. Its actual relative link resolves to the canonical
extension chapter. The three existing legacy Markdown projections keep their
exact bytes; source and projection are not maintained independently.

The reviewed manuscript was moved to `doc/spec/01-architecture.nepld`.
One heading was subsequently clarified to “3. surface descriptorとGrammarのbootstrap”.
The original section explicitly describes Grammar's own seed/bootstrap, so this
does not change its requirements. This avoids a duplicate GitHub automatic
heading anchor while retaining the explicit legacy Markdown alias. The rest of
the moved manuscript is unchanged. All eight original heading destinations and
seven semantic Section destinations remain in generated Markdown.

HTML uses seven emitted semantic Section IDs and the actual page-to-page link.
The canonical HTML host does **not** consume the Markdown alias file. An initial
test incorrectly expected the eight Markdown aliases in HTML; its failure and
the scope correction are retained. The same inaccurate user-facing claim was
explicitly corrected. Legacy Markdown aliases must not be inferred to exist in
HTML, and future URL compatibility work must define the two formats separately.

## Validation

Two fresh Markdown stages (five files including the receipt) and HTML bundles
(six files including CSS and manifest) are byte-identical. Actual source parsing,
lowering, preparation and rendering use the production implementation. The
registry selects shared Markdown output Work 300M / Allocation 750M before entry;
all other named limits keep their default values. This does not raise global
defaults or establish default-budget success for four grouped pages. The HTML
host retains its separate existing default output budget.

Root browser checks passed in Chromium, Firefox and WebKit at widths 375/1280
under two non-root paths with document JavaScript disabled: twelve display cases,
84 semantic fragment navigations and twelve actual links to chapter 22. Code
blocks equal the original fenced source values, including final newlines. Root
inspected the narrow and wide WebKit screenshots. All fifteen GitHub destinations
were individually navigated on the fixed source commit with matching server
rawLines, unique targets and stabilized visible scroll positions.
The verbatim restoration snippet below also passed in all three engines using
file URLs, JavaScript disabled and HTTP(S) blocked. All six restored files match
the generated bundle, and the chapter-22 link remains usable offline.

The earlier GitHub run failed because an explicit legacy alias and GitHub's
automatic permalink occupied the same bootstrap section. Its DOM and partial
results remain a failed run. The corrected heading was independently checked
against the original meaning. A first browser expected-code extraction omitted
the final newline in the original code fence; the expected-value parser was
corrected, with no production or manuscript change for that test mistake.

Full manuscript meaning/authoring review preceded cutover and checked the original
requirements, dependency boundaries, target versus current state, raw code and
relative link. The one heading clarification has a separate review. Code,
cutover and archive integration reviews are recorded separately; their manifests
and records identify exact executed scopes. Human accessibility review, Pages
deployment, actual repository separation and whole T21/T16 acceptance are not
established by this chapter's tests.

Independent fresh generation also passed twice with exact five/six-file equality,
independent context-digest reconstruction and unchanged existing three Markdown
pages, their HTML and CSS. It compared both raw code blocks through the actual
CommonMark parser and HTML text. A moved-source `include_str!` in an integration
test still named the deleted draft path; its independent compile failure is
retained. The fixture now names the canonical Doc source. The same no-run command
and five independent native page-set tests passed after correction. Root's full
86 Doc native tests and five WASI page-set tests also passed. The initial one-line
repair had an indentation error caught by `cargo fmt --check`; that formatting
failure was observed interactively, not preserved as a fabricated raw log.

Independent cutover checks separately passed all fifteen GitHub destinations,
twelve HTML cases, 84 semantic moves, twelve cross-page links and three offline
restorations. Its first offline probe attempted CSSOM `cssRules` from a file URL
and failed with Chromium SecurityError. The retained failed probe was corrected
to test computed styles and actual CSS application; production was unchanged.

## Read the saved HTML without a compiler

Run this code from the repository root to restore six checked files to a new
directory. It does not invoke Rust, Node, network or Doc compilation.

```python
from pathlib import Path
import hashlib, json

archive = Path("conformance/results/doc-canonical-architecture")
output = Path("dist/canonical01-snapshot")
entries = json.loads((archive / "payloads.json").read_text(encoding="utf-8"))["files"]
selected = [e for e in entries if e["owner"] == "root-html-first"]
assert len(selected) == 6
output.mkdir(parents=True, exist_ok=False)
for entry in selected:
    relative = Path(entry["restore"])
    assert not relative.is_absolute() and ".." not in relative.parts
    data = (archive / entry["file"]).read_bytes()
    assert len(data) == entry["bytes"]
    assert hashlib.sha256(data).hexdigest() == entry["sha256"]
    destination = output / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("xb") as stream:
        stream.write(data)
```

Open `dist/canonical01-snapshot/docs/spec/01-architecture.html` with the adjacent
assets and pages. This saved reading snapshot does not replace regeneration.
`payloads.json` binds original restore paths, lengths and hashes. Strict reviewer
manifests exclude caches, binaries and build workspaces. `.fixture` payloads keep
original bytes and reserved archive path components are renamed `saved-*`.
