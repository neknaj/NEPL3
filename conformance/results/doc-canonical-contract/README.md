# Chapter zero canonical Doc cutover

Implementation checkpoint: `4895c1a6a94660a43e617ed86a21844838a7324d`.
The manually authored chapter moves unchanged to `doc/spec/00-contract.nepld`;
`doc/canonical.json` registers it as the source of truth. Its old `.md` path is
the generated annotated `/2` view. This is one page's cutover evidence, not T21,
T16, all-document migration, public Pages deployment or human assistive-technology
acceptance.

The Doc source SHA-256 is
`48c2b1696c7308ee43f26dbf06a6038b38f0199233c1da3ef15a04453f810e93`,
identical to the previously reviewed `cd451bf` manuscript. It retains both
sentence literals and explicit Sentence construction, Kanji Ruby and explanatory
Anno. The former Markdown source is retained unchanged only as the explicitly
historical `tools/migration/fixtures/00-contract.md` converter-test input, from
`a163764e93e2809f8013d7bbe16721082f979143`. It is not another maintained spec.

The restricted converter, its generated Doc fixture and the independent Rust
Markdown-event comparison remain tested. Their historical expected content was
not regenerated from the annotated output. Changed historical input fails before
`--write` can modify the expected output or audit. The current specification is
checked separately by `doc-canonical`.

The first final repository check rejected four archived payload paths containing
the reserved directory name `dist`. Their archive directory is now `saved-dist`;
the manifest retains the original restore paths, byte lengths and digests. No
payload was altered and the repository exclusion rule remains unchanged.

## Local evidence

- Python converter check and four tests passed. The accurately qualified
  historical projection Rust test ran one test and passed. An earlier unqualified
  `--exact` filter selected zero tests and is not counted as a pass.
- Canonical projection checks passed for chapters zero and thirteen. Two HTML
  builds produced identical four-file sets. Chapter thirteen's HTML/CSS remain
  unchanged when chapter zero is added.
- Tools unit tests: 70 passed, one existing ignored. Format, staged diff and
  repository checks passed; broader runtime acceptance was not run by them.
- Root: JavaScript-disabled Chromium, Firefox and WebKit at 375/1280 pixels under
  two non-root paths passed 12 display checks and 72 real semantic-anchor
  navigations. All three list sizes (4/6/14), ten literal code values, seven
  headings, stylesheet loading, unique IDs and no horizontal page overflow were
  checked. Root visually inspected the WebKit narrow/wide screenshots.
- Root: actual GitHub HTML at the fixed checkpoint matches the generated
  Markdown's raw lines. All seven old anchors and six Doc semantic anchors are
  unique and reachable from outside the viewport, with five stable scroll
  samples after each real hash navigation. Root inspected the rendered page.
- The README restoration script below was executed verbatim. Its four restored
  files equal the checked output bytes. All three browsers opened the restored
  chapter through file URLs with JavaScript disabled and loaded the stylesheet;
  no compiler or regeneration command was used for this reading check.
- Independent content review read the entire original, Doc and output, preserving
  all four language/product scopes, exclusions, six boundaries, INV01–INV14,
  ten code values and annotations. Independently reconstructed heading/paragraph
  text agrees across Doc, Markdown and actual HTML. Separate independent review
  covers the two manually synchronized guide manuscripts.
- Independent code review retained the converter algorithms and expected fixture,
  exercised the accurate Rust test, Python tests and both canonical checks, and
  rejected altered Doc/projection/aliases and historical-input changes. Its
  initial missing scratch dependency was a failed build, not a runtime result.
- Independent cutover review rebuilt the fixed production generator and matched
  all four HTML/CSS/manifest files. It reran all 13 GitHub and 72 local anchor
  navigations, checking stable scroll, sticky offset/clamping and actual movement.
  All 12 local display cases passed. It also restored the exact README script's
  four files into isolation and opened them with HTTP blocked and JavaScript
  disabled on all three engines. Public raw Markdown bytes, including final LF,
  matched the fixed Git blob. The later main merge added only PR #87's evidence.

The author-side preflight is explicitly not an independent meaning review.
The content review's first helper escaped Markdown HTML blocks as text; the
helper was corrected without changing production or expected meaning. Raw logs,
source hashes, scripts and those corrections remain separate payload owners.
New public-site deployment and manual screen-reader operation are not implied
by DOM/ARIA inspection. The generated HTML has no interactive links in this
chapter, so no keyboard-link traversal success is claimed for it.

## Read the saved HTML without a compiler

Run this from the repository root with Python. It restores the exact checked
HTML, CSS and output manifest into a new directory, verifying every payload.
No Rust compiler, Doc parser, Node or network is invoked by this restoration.

```python
from pathlib import Path
import hashlib, json

archive = Path("conformance/results/doc-canonical-contract")
output = Path("dist/canonical00-snapshot")
entries = json.loads((archive / "payloads.json").read_text(encoding="utf-8"))["files"]
selected = [e for e in entries if e["owner"] == "root-verification" and e["restore"].startswith("first/")]
assert len(selected) == 4
output.mkdir(parents=True, exist_ok=False)
for entry in selected:
    relative = Path(entry["restore"]).relative_to("first")
    assert not relative.is_absolute() and ".." not in relative.parts
    data = (archive / entry["file"]).read_bytes()
    assert len(data) == entry["bytes"]
    assert hashlib.sha256(data).hexdigest() == entry["sha256"]
    destination = output / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("xb") as stream:
        stream.write(data)
```

Open `dist/canonical00-snapshot/docs/spec/00-contract.html`. Keep the adjacent
`assets/doc.css`. The saved bundle also contains chapter thirteen; it is a
compiler-free reading snapshot, not a substitute for reproducible source checks.

## Archive format

`payloads.json` binds the implementation sources and every `.fixture` to its
owner, original relative name, byte length and SHA-256. Restore those names when
inspecting/rerunning scripts. The archive excludes build caches and binaries;
raw CRLF, patch context whitespace and log endings are preserved through local
Git attributes. Original reviewer manifests remain inside their own groups.
CI and any final cutover review are additional exact-head merge gates.
