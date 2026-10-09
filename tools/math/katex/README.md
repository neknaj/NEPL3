# Generation-side KaTeX call

`render.mjs` is the synchronous invocation layer shared by future Node and
browser Worker hosts. A caller injects fixed KaTeX 0.18.7; its absence does not
prevent independent Rust MathML startup. Real-NEPL TeX tests use this module.

`node/render.mjs` runs that call in a fresh Node Worker per expression, using a
trusted host-configured local module URL. It checks input before structured
cloning, captures console records in the isolated realm, and bounds diagnostic
UTF-8 bytes (including a byte per record). Deadline and AbortSignal stop the
realm; the promise resolves only after exit and stdio drainage. Unexpected
direct stdio or premature exit is a provider violation. A termination failure
is recorded, and is not reported as a confirmed exit while the realm is alive.
The deadline includes module loading, not only rendering. Cancellation before
final resolution wins over output, including termination and stream drainage.
The owning document operation must still check its current
source/revision and stop state before using any result.

This Worker is a CPU cancellation mechanism for trusted renderer code, not an
OS capability sandbox or an exact memory bound. Node's timer is not a hard
real-time guarantee. Terminated calls omit unobserved diagnostics/usage rather
than inventing zero usage. A local deadline outcome alone never authorizes a
document fallback. The one-shot lifetime also prevents module-global state from
leaking between requests; amortized batch/pool performance is not claimed.

This is an internal host API, not a complete portable operation envelope.
It returns **unchecked renderer output**, never an artifact or fidelity proof.
`html` is the visual profile; `htmlAndMathml` is the explicit comparison profile.
Neither detects broken CSS/fonts. Input/output limits count UTF-8 bytes and
reject isolated surrogates. Options use finite maxExpand/maxSize, strict errors,
trust=false and a fresh empty macro object. The pinned ParseError rawMessage
distinguishes expansion limits from ordinary parse errors without returning
unescaped exception text. Internal exceptions/version mismatches are violations.

The outer host still owns complete request/resource identity, reservation and
settlement, isolated execution, console capture, timeout/cancel and transport.
The Node helper now supplies the invocation-local isolation/capture/stop portion;
the browser Worker and complete operation protocol remain separate work.
This call cannot interrupt synchronous KaTeX or measure its internal allocation.
KaTeX can emit metric warnings through its own console; the outer isolated realm
must capture those diagnostics. No exception does not mean faithful output.
maxSize can clamp dimensions, so producer/profile fidelity must be established
before admission. Arbitrary caller macros are not supported.

Markup/CSS/SVG validation, stylesheet extraction, CSS/fonts, accessibility,
Node/real-Worker comparison and Doc export remain under
[spec17](../../../doc/spec/17-math-html.md). Never insert this unchecked string
directly into a document. These outcomes do not authorize a stopped parent
operation to fall back.

Run `node --test tools/audit/math/render.test.mjs` after
`npm ci --prefix tools/audit/math --ignore-scripts`. The Rust integration test
separately feeds real NEPL expressions through production TeX and this call.
Run `node --test tools/audit/math/node.test.mjs` for actual Node realm, warning,
missing-module, stdio/exit, cancellation and synchronous-loop termination tests.
The test-only renderer fixture exercises failures and is never a math oracle.
Worker lifecycle follows the [Node Worker API](https://nodejs.org/api/worker_threads.html).

## Fixed distribution resources

`node/assets.mjs` loads the CSS, all referenced fonts and original LICENSE from
a trusted installed KaTeX package into owned bytes. `assets.json` pins paths,
MIME, sizes and SHA-256; no arbitrary package directory traversal is used.
The resource byte cap is checked before I/O, and reads remain bounded even if a
file grows. This cap covers retained asset bytes, not total JS heap or I/O time.
Any missing, truncated or changed resource rejects the whole load. Returned
buffers belong to the caller, which must revalidate them at an external boundary
and must not replace them by later path reads. This is neither document admission
nor proof that the executing renderer binary matches the asset package.

The original `katex.min.css` and sibling `fonts/` paths are retained. See the
[upstream font layout](https://katex.org/docs/font) and
[fixed package license](https://github.com/KaTeX/KaTeX/blob/v0.18.7/LICENSE).
No runtime CDN, font pruning, CSS rewriting, inline-style admission or automatic
document insertion is added. The artifact owner places the entire set under
one directory and binds its stylesheet and licensing records.

After a reviewed package upgrade and `npm ci --prefix tools/audit/math`, run
`node tools/generate/katex.mjs --write`. Without `--write` it verifies the
inventory and its generated native Rust pin array. The URL scan only inventories that reviewed fixed CSS; it does not
sanitize unknown CSS. `node --test tools/audit/math/assets.test.mjs` checks actual
package closure, bytes and tamper/missing/limit cases. Binary resources remain
in the package/install or generated artifact, never copied into source history.

## Visual HTML conversion

`parse.mjs` accepts the injected parse5 8.0.0 fragment parser. It rejects parse
errors, unknown elements/attributes, repaired containers and namespace changes;
it never strips unknown content and labels the remainder successful. The output
is a finite postorder **unchecked** tree of span/text and SVG/path/line nodes.
Run parsing inside the caller's cancellable realm. Its byte/node/depth caps are
not an exact parser allocation or elapsed-time meter. The outer JSON transport
must also cap bytes before decoding. See [parseFragment](https://parse5.js.org/functions/parse5.parseFragment.html)
and [parser options](https://parse5.js.org/interfaces/parse5.ParserOptions.html).

`tools/src/doc/math/katex.rs` consumes this internal host JSON with exact field
sets, revalidates through `nepl3_markup::katex::fragment`, and writes a visual
HTML/stylesheet pair. The core proof borrows the exact tree; arbitrary JSON or
HTML never acquires that proof merely because a parser returned successfully.
This host JSON is not the portable NDF provider contract.

Computed styles become occurrence-specific classes under a host-assigned unique
`nepl-math-` scope. Validated declarations retain order/values and gain
`!important`; the host must exclude competing important declarations (including
shorthands/`all`) in the fixed CSS and competing author rules in that scope.
This preserves priority over the fixed renderer's normal declarations, not over
arbitrary CSS. The browser comparison checks CSSOM, computed properties and
geometry, and verifies `style-src-attr 'none'` blocks an injected style attribute.
The production constructor test now selects the fixed resource/producer catalog
through verified assets. Output-derived classes remain audit observations only
and do not grant admission authority.

The pair is visual-only (`aria-hidden=true`). Independent accessible MathML,
complete source/resource identity, full Doc insertion and browser Worker
transport remain unfinished. Do not publish this pair alone as an accessible
document, or claim whole-artifact/fidelity acceptance from these scoped tests.

Run `node --test tools/audit/math/parse.test.mjs` and the regular Rust
`katex_fragment` tests. The ignored production TeX integration test emits
`MATH_VISUAL_CASE` records with `--nocapture`; pass its UTF-8 stdout to
`python tools/audit/math/browser.py --corpus <log>`. It uses the existing pinned
Playwright requirements and all three engines by default; generated logs remain
local/CI artifacts rather than source history.


## Native resource-content binding

`tools/src/doc/math/assets.rs` accepts already bounded borrowed host inputs and
requires the exact complete inventory order, path, MIME, length and SHA-256.
Metadata checks precede all payload hashes and copies. Private retained assets
copy only verified lengths; caller capacities and later mutation cannot expand
retained storage. The fixed metadata is generated beside assets.json by the same
inventory tool and checked in the fixed-assets CI job.

Preparation charges copied storage and all hash/copy work, emits no OutputBytes,
and applies an explicit retained binary byte cap. Binary assets are not text
SourceSnapshots and are not charged through SourceAdmission. Materialization
charges every output/copy byte on each call. Borrowed inspection alone is not a
budgeted file export. Errors return no partially prepared set.

The versioned framed identity covers version, count, ordered paths, MIME, lengths
and verified digests. It identifies resource content, not package origin, executing
renderer code, CSS class coverage, source revision, fidelity, accessibility or Doc
admission. Future composition must reserve the mounted asset namespace and bind
these exact owned bytes; it must not substitute later path reads.


## Supervised visual parsing

`node/visual.mjs` offers a separate `renderVisual` call for generated TeX and
trusted local renderer/parser module URLs. The legacy `node/render.mjs` API is
unchanged. Both use the same one-shot realm lifecycle; the visual Worker includes
renderer import, KaTeX generation, parser import, HTML5 parsing, finite-tree
conversion and reply JSON serialization in the same unchanged deadline.

TeX bytes, rendered HTML bytes, parser input/nodes/depth, diagnostics and complete
serialized reply bytes are independently bounded. The reply cap covers raw HTML,
finite visual tree, metadata and diagnostics together. Only fixed-size overflow
or serialization-failure controls are reserved outside that cap; they omit unknown
diagnostic observations. The parent bounds the exact UTF-8 string before JSON
decoding, then rechecks cancellation/deadline. These are content/byte controls,
not exact JS heap/work measurements or hard real-time guarantees.

Missing imports remain unavailable; invalid parser exports, malformed markup,
parser exceptions and serialization defects are provider failures. Diagnostic
or reply overflow, cancellation and deadline expiry remain stops. Resolution still
requires Worker exit plus stdio drainage; a failed terminate request alone is not
proof of exit. Diagnostics are captured before dynamic imports in that fresh
realm. No returned `visual-parsed-unchecked` value is a checked Math artifact,
renderer identity proof, class inventory or Doc insertion capability.

The trusted-URL visual route imports parse5 only inside its Worker. The audit
adapter now selects the stronger fixed-source route described below, while
retaining its separate htmlAndMathml comparison. Tests cover entered loops
in render/import/parse/serialization, cancellation, termination refusal, byte and
structural boundaries, malformed content, diagnostics and fresh module state.
Run `node --test tools/audit/math/visual.test.mjs`; CI runs it on Linux and in the
Windows/macOS lifecycle job. Those CI executions remain separate from local tests.

## Fixed production class catalog

`classes.json` and the private native catalog combine 138 CSS class names from the
exact reviewed CSS with 17 explicitly reviewed inert producer tokens. Atom,
tight/text grouping and all seven script fallback markers are listed with pinned
source rationale. The generator requires the fixed CSS digest/version before its
narrow selector-prelude inventory runs; this is not arbitrary CSS sanitization.
Declarations and URLs do not add class authority. Unsupported selector/block
syntax requires review. The fixed unrelated high-contrast important declaration
is recorded; no competing lowered-property important declaration is accepted by
this reviewed catalog. Class coverage does not prove glyph availability or style
confinement; the original CSS includes document-level rules.

`PreparedAssets::prepare_visual` chooses this catalog itself, rather than accepting
renderer-reported class names. The private bound result retains a borrow of the
exact verified asset set through visual serialization. It exposes content and
asset association, not renderer execution identity, same-Math request pairing,
CSP, accessibility, namespace reservation or Doc insertion. The low-level visual
validator remains available for explicit tests; it cannot forge the private bound
result. The real constructor corpus now uses this fixed production path and tests
all script groups; parser-derived `classes` remains test observation only.


## Fixed owned package-source execution

`node/fixed.mjs` adds a separate fixed-package visual route. A trusted installed
node_modules directory locates 20 pinned ESM sources (953,744 raw bytes): KaTeX
0.18.7, parse5 8.0.0 and the required entities 6.0.1 closure. The public parse5
entry includes its serializer, so both entities/decode and entities/escape are
part of the fixed graph. No arbitrary module resolver or package root is taken
from a document.

`node/modules.mjs` checks the total byte cap before reads, then regular-file size,
bounded reads, extra-byte probe and SHA-256 for every source before package
execution. Fatal UTF-8 decoding retains BOM characters and does not rewrite
newlines. The decoded strings remain in a private issued-bundle map. Linking and
execution never reopen package paths, so replacing those files after loading
cannot replace executing code. Copied bundle metadata cannot mint this local
capability; it is intentionally not a portable receipt.

Every invocation constructs all SourceTextModules in a fresh explicit context,
checks their static request list and links exclusively to preconstructed catalog
modules. Unknown static mappings/import attributes/phases and dynamic imports
are denied without filesystem/network fallback. Only the bounded console sink
is supplied to the context; string/Wasm code generation is disabled. vm is not a
security mechanism, and these restrictions do not make it an OS sandbox. The
code remains reviewed trusted package code; only renderToString is qualified by
this local test slice.

The source identity is SHA-256 of the UTF-8 domain
`nepl3.katex.source-catalog/1` followed by NUL and the exact generated modules.json
bytes, including its final newline. That serialization is identity-bearing. It
covers package/version metadata, ordered module IDs/formats/lengths/digests,
renderer/parser entry roles, import edges and loader-policy version. Package
origin, trusted bootstrap/host render and parse helpers, Node/V8/native code,
remote authenticity, same-Math issuance and Doc admission are not proved by it.
Runtime version/platform are reported separately, not folded into source content.

The dedicated Worker explicitly uses --experimental-vm-modules and selective
ExperimentalWarning stderr suppression. A warning listener remains active: one
exact expected VM warning is observed, while an unexpected warning rejects the
result. It is flushed before success. Package console is explicitly injected
into the VM context, so a separate default VM console cannot lose diagnostics.
Console overflow aborts rendering and remains a diagnostic stop. The same
parent deadline covers acquisition, linking/evaluation, invocation, parsing and
serialization, with exit/stdio-drain confirmation and cancellation precedence.
These are bounded content and lifecycle controls, not exact JS heap/work or hard
real-time/OS-I/O cancellation guarantees.

The audit adapter uses this route and retains the fixed sourceIdentity alongside
the visual result. Received JSON identity fields alone do not grant a private
native artifact capability. Local tests ran on Linux x64 with Node 24.19.0 and
actual CI-version Node 24.14.1; Windows/macOS execution remains a separate CI gate.
The Node 24 API profile uses moduleRequests and the older module.link callback;
no Node 20/22 compatibility is claimed.

```
node --experimental-vm-modules --disable-warning=ExperimentalWarning tools/generate/katex/modules.mjs
node --experimental-vm-modules --disable-warning=ExperimentalWarning --test tools/audit/math/modules.test.mjs
```

The generator parses static requests without evaluating package code. Its
--write mode is for an explicitly reviewed fixed-package update. Tests cover all
20 source mutations, size/absence/cap failures, no-reopen behavior, forged handles,
fresh state, real render parity, diagnostic and reply limits, expected/unexpected
warnings, dynamic-import/code-generation denial and entered VM-loop termination.
The source-pin suite does not complete full Math artifact or browser acceptance.

## Internal native stdio bridge

`node/stdio.mjs` is a one-shot subprocess entry for a future native owner. Its
four host arguments are the local `file:` module-directory URL, stdin byte cap,
stdout byte cap (at least 256), and input/execution deadline in milliseconds.
Network hosts, URL queries and fragments are rejected. The host selects the
Node executable/script and must remove inherited preload options; none of those
choices comes from a document request.

The input is exactly one version-1 object with `version`, `request`, `limits`,
`parseLimits` and `options`, followed by EOF. Their fields are closed by the
bridge. The internal wire spelling must equal `JSON.stringify(JSON.parse(text))`:
no whitespace, duplicate keys, alternate escapes/number spellings or rounded
unsafe numbers. Object key order follows input insertion order, not sorted keys.
A future Rust encoder must obey this spelling contract. Fatal UTF-8 decoding
happens only after raw-byte admission. TeX is passed as data to the fixed route.

The response adds `version: 1` and is bounded again after Worker metadata was
added. Small failure controls reserve at most 256 bytes. No newline is emitted.
This is unchecked internal data, not an authenticated remote Report, a complete
external resource measurement, same-Math admission or a Doc artifact. Native
structural validation and the owning pending request remain separate.

The bridge deadline covers stdin wait, decode, execution and serialization,
including synchronous failure paths. The future parent must independently bound
stdout/stderr, cancellation, blocked writes and process exit, kill/reap as
needed, and require EOF plus exit status before accepting anything. The bridge
clears its execution timer before stdout flush; it does not claim a complete
process deadline. A partial/failed write sets a failing exit status and never
attempts a second JSON message. Parent cancellation always invalidates adoption.
Run `node --test tools/audit/math/stdio.test.mjs`; it includes actual pinned
execution, bad wire/configuration, byte boundaries, stalled input and pipe error.

The native `doc::math::display::request` assembler now encodes that wire from
one immutable `PreparedDisplay`: callers cannot supply a replacement TeX or
mode. `PreparedRequest` borrows its owner and exposes only read-only bytes. It
is input assembly, not a unique invocation, source-revision check, execution
receipt or result admission. Repeated assembly is allowed and charged anew.
The supplied Budget pays the bounded buffer capacity, TeX scan and emitted
Work/OutputBytes; SourceBytes are not charged for this generated transport.
Budget continuity with earlier preparation is the owning operation's duty.
MathML-only and unsupported TeX produce no request; the reason stays available
in the original prepared owner's typed `TexPreparation`.

`doc::math::display::process` provides cooperative direct-child supervision for
that prepared request. The host supplies a trusted Node executable, bridge path
and module URL. Node option parsing is terminated with `--`; NODE_OPTIONS and
NODE_PATH are removed, while the remaining host environment is trusted. Native
input/output byte capacity and maximum retained-byte processing are prepaid in
the supplied Budget; process/thread/OS overhead and external renderer work or
allocation are not thereby measured.

The caller must keep polling the owned Process. Cancellation, deadline and
budget stops remain selected while cleanup continues. Poll never joins live
I/O workers, and completion requires reaped zero exit, completed stdin, clean
stdout/stderr EOF, bounded bytes and a final stop check. The returned bytes are
unchecked and retain the prepared owner; there is no JSON/visual/Doc admission.
A consumed process cannot return another completed result.

This is not a process-tree sandbox or a hard autonomous deadline. Descendants
holding a pipe keep cleanup Pending; its selected failure and termination error
remain visible. Dropping the owner attempts direct-child termination but does
not certify reaping or reclaim blocked pipe threads. Callers needing confirmed
cleanup must retain it. The tests explicitly release a descendant-held pipe
before terminal cleanup; OS thread-creation/wait failures and arbitrary stuck
process trees are not claimed verified.

A completed transport now retains the exact immutable PreparedRequest, its
copied Controls and the host Config selected at launch. Decoder-side replacement
limits cannot stand in for those values. These are invocation selections, not
qualified executable/source identities; path equality does not prove file bytes.

`process::reply::decode` consumes the completed transport and checks its closed
version-1 envelope against the retained invocation. Observations are all-or-none;
absence stays unknown. It validates finite result families, diagnostics, safe
integer counters, generated source-catalog metadata and the exact inner Worker
JSON byte count. The latter excludes outer version/replyBytes/terminationFailure.
An observed termination-request failure does not undo subsequently confirmed exit.

Visual replies retain raw producer HTML and a shape-checked, connected finite
tree with requested node/depth caps. This does not establish HTML/CSS/SVG safety,
qualified bootstrap execution, HTML/tree fidelity, mathematical fidelity or Doc
admission. Never insert the retained raw HTML directly into a document. Fixed
asset/markup validation and accessible composition remain separate.

Before JSON validation/decoding, the native host conservatively admits 128
allocation units and four Work units per input byte. Subsequent typed vectors,
checks and inner encoding traversal are charged separately. These are logical
admission conventions, not measured allocator, CPU or external renderer Usage;
OutputBytes and SourceBytes are not charged for verification. Decoder Budget
stops are errors and do not become successful renderer fallback outcomes.

`Reply::prepare_visual` now consumes a decoded visual reply into fixed-resource
associated visual parts. It moves typed nodes directly through the existing
markup validator, checks the asset/renderer version, and discards raw producer
HTML. Request/configuration, observations, termination flag and exact resources
remain associated through repeated validated serialization. Preparation emits
no OutputBytes; each serialization is charged again. Nonvisual replies return
unchanged, including stops and violations; they are not automatic fallbacks.

This native type does not establish qualified bootstrap/portable identity,
mathematical fidelity, HTML/tree equivalence, document-global scope uniqueness or
Doc admission. Typed HtmlRequest now admits the narrow svg/path/line arena nodes and boolean
aria-hidden, with registered uppercase/underscore classes and full shared
validation. This is substrate only: accessible dual representation and full artifact admission remain unimplemented;
the separate native visual projection described below does not establish them. Serialized
strings must not bypass the validator.


`nepl3_markup::katex::fragment::PreparedVisual::into_html` now provides a consuming native visual projection
into the typed HtmlRequest arena plus generated CSS. It preserves original node
indices and child buffers, adds an aria-hidden scope wrapper, omits empty Class
attributes and removes repeated class tokens in first-occurrence order. CSS
rules retain the previous serializer's root-first traversal and declaration
bytes/order, with the same original-index selector suffixes and !important.
Ordinary HTML serialization sorts attributes, so its HTML bytes intentionally
need not match the old serializer's attribute order.

The original visual policy is checked before adding scope/generated classes;
the complete typed HTML is revalidated with the same sticky caller Budget.
Conversion meters new storage and work (including potentially quadratic token
and policy comparisons) and charges only emitted CSS OutputBytes. Subsequent
HTML serialization charges its own bytes. A styleless conversion emits zero
OutputBytes. The immutable result remains visual-only: fixed resource binding,
document-global scopes, independent accessible MathML, fidelity, and complete
artifact admission remain host obligations. No Doc CLI route is enabled by this
component alone.


Host projection preserves the existing native associations. Consuming
`assets::BoundVisual::into_html` returns `BoundProjectedVisual`, retaining the
exact fixed-resource reference. Consuming
`reply::visual::AssociatedVisual::into_html` additionally retains the actual
PreparedRequest, Config, observations and termination flag in
AssociatedProjectedVisual. Request and asset lifetimes remain independent.
Neither wrapper exposes owner replacement, mutable content, Clone or public
part extraction. Core projection stops remain top-level Stopped errors; failure
returns no partial association and does not roll back already consumed budget.
CSS is charged during projection, while subsequent HTML and resource output
must be charged when performed. A visual reply missing its observations is
still rejected by the decoder. This is not qualified bootstrap identity,
semantic fidelity, document-global scope admission or accessible MathML/Doc
composition, and no CLI default is changed.

`PreparedDisplay::generate_owned` is a native completion scope, not a renderer
loop or fallback selector. It temporarily borrows its owned preparation to issue
one PreparedRequest, passes that request and selected Config to a higher-ranked
driver, then verifies the actual returned request pointer and byte-exact Config
before releasing the borrow. The scope itself chooses the fixed resources and
scope class, so the driver cannot substitute these projection inputs.

The opaque OwnedGeneration retains the original PreparedDisplay, selected
controls/configuration and an explicit attempt outcome. Request rejection,
driver failure, association mismatch, unsafe visual validation and nonvisual
replies remain distinct data; none is successful artifact admission or automatic
fallback. Valid decoded observations remain attached even when visual validation
fails. When no driver ran or no bound reply was obtained, observations and termination
information remain unknown rather than inventing zero counters. Parent budget stops always return
Err, including after a driver returns Ok or Err; a genuine non-TeX request calls
no driver and preserves its MathML-only/unsupported reason. The original request
preparer still rejects invalid controls before detecting that non-TeX case, and
the owning scope retains that rejection with its original prepared input.

Temporary request borrows cannot escape the higher-ranked callback or fixed
owned error type. Private detachment is limited to the display subtree, and the
owned result has no public arbitrary-pair constructor, mutable access or part
extraction. Passing &mut Budget does not enforce honest native-driver accounting;
using the supplied cumulative ledger and handling pending process cleanup remain
trusted driver obligations. This does not provide async/Web orchestration, hard
deadlines, qualified execution identity, document-global scopes, accessible
MathML/visual composition or a complete artifact publication gate.


`OwnedGeneration::into_composite` consumes an eligible local generation result.
Visual success becomes typed dual markup; genuine NotRequested becomes the
original visible MathML with no added KaTeX CSS or assets. Every other attempt
remains Deferred with its complete original owner and failure metadata. A stopped
budget takes precedence over Deferred. This stage is not a fallback selector.

The original projected MathML arena stays at offset zero, preserving node roots,
annotation roots and Sentence/foreign origin mappings. Visual nodes are appended
with all HTML, MathML and SVG edges rebased. Inline output uses span wrappers;
block output uses div wrappers. Only the visual subtree is aria-hidden. The
accessible MathML wrapper gets a fixed scoped clipping rule, including the pinned
webkit declaration. Existing visual classes and stylesheet order are unchanged;
reserved MathML classes and accessibility-class collisions are rejected.
Generated visual nodes and wrappers carry the Doc occurrence owner, without
inventing per-Math-node provenance.

The immutable composite retains its selected scope/configuration/controls,
original TeX preference/unsupported reason, decoded observations, termination
flag and exact fixed asset reference. Ready describes local composition only:
termination_failure=true is preserved, and does not establish process cleanup or
artifact success. Complete resource/CSP admission, global scope uniqueness,
mathematical fidelity and browser/accessibility qualification remain separate.
Callers must supply their cumulative budget; appended CSS is charged as new
output, while already-generated CSS is not emitted again at this stage.


`ComposedMath::serialize_html` validates and emits the local typed HTML fragment
at the caller's current depth. The returned opaque SerializedComposite borrows
that exact ComposedMath owner, keeping its source, mappings, selected resources
and failure observations associated with the bytes. Each invocation charges the
full HTML emission again; repeated calls are not an output-budget bypass. No
stylesheet or asset is emitted or materialized by this operation. The borrowed
owner's existing CSS and resources still require separately metered artifact
assembly, global namespace/resource/CSP checks and browser/accessibility review.


The native Setup separates the transient scope-name lifetime from selected
configuration and fixed resources. generate_owned copies the scope name with
prepaid Work/Allocation and retains that owned name through composition and HTML
serialization; callers may release per-occurrence naming buffers immediately.
This removes a lifetime obstacle to later multi-occurrence document assembly,
but does not itself establish document-global scope uniqueness or source-owner
identity. Invalid scope names are still rejected by visual preparation; no-request
metadata remains visible without being promoted to visual output.


For native plain-Article integration, doc-html guests::render_with_context adds
an immutable Context from the actual prepared traversal. It supplies the exact
document/options/embed references, preparation document digest, actual Doc node,
EmbedRef and render-local foreign ordinal. Shared embeds may belong to distinct
nodes, and a shared node may be displayed repeatedly; these cases keep distinct
ordinals. The ordinal is the zero-based index in that successful render's foreign
placement vector, includes Code and both Math modes, and resets per render.
Legacy render callbacks use the same loop, depth, stop and output checks.
This context is not a portable request identity, one-shot authorization, global
scope allocation, or admission of a complete Math/document artifact. Other host
routes retain their existing interfaces.


`math::occurrence::generate` consumes that opaque Doc context and prepares Math
from its exact document/node before entering the owned generation scope. Its
private Generated/Composed wrappers retain the same Context through ordinary
attempt failures, Deferred and Ready outcomes, eliminating a public route for
pairing independently prepared Math with a different Doc occurrence. Parent
stops outrank preparation/driver errors; non-Math contexts retain the host's typed
node rejection and call no driver. All access is immutable.
This selected-fixed-resource native route still requires real prepared assets;
it is not the independent MathML-only host API, a portable identity, successful
Doc import, cleanup proof or complete artifact adapter. The actual Doc import
integration fixture deliberately clones markup outside a production accounting
claim. Its package-dependent runtime cases are run explicitly with --include-ignored.


`math::document::render` is the closed native plain-Article import stage. It calls
Math and Code adapters in the existing prepared Doc traversal, checking each
bound Math result against the exact document/options/embed references, digest,
node, EmbedRef and guest ordinal. It moves markup into the core import and keeps
original source/metadata allocations; after successful core rendering it remaps
Math roots, annotation roots, recursive Sentence/Math/Doc mappings and generated
visual ranges into the resulting document arena. Foreign placement is indexed
by the guest ordinal, including Code, not by the Math-only record index.

Record growth, comparisons and remapping use the supplied cumulative logical
Budget. Adapter accounting remains trusted. Any stop, adapter error or mapping
failure returns no LocalDocument; partial private records are dropped. The
max_math bound counts selected Doc Math callbacks, not nested annotations. Code
and nonvisual Math cannot introduce reserved nepl-math-* policy classes. Dual
occurrences must have matching fixed-resource identities and distinct scopes;
inert scope metadata on MathML-only results does not cause collisions.

The opaque LocalDocument retains the exact preparation and immutable remapped
records. Contexts from equal-content different native owners/options are rejected.
A repeated render with the same owners and ordinal has no epoch/freshness proof.
This stage does not emit HTML/CSS/fonts, allocate document-global scopes, handle
page bundles, establish portable identity, prove process cleanup or qualify
browser/accessibility behavior. Closed import alone is not a CLI artifact
adapter; the explicit trusted-host route below supplies host selection, supervision
and publication. The integration tests retain a deliberately unmetered cloned reference
path only for expected-output comparison; the closed import itself moves inputs.


`LocalDocument::materialize_bundle` emits a local, immutable external-CSS bundle
borrowing the exact imported owner. Its fixed paths are index.html, assets/doc.css,
assets/math.css and the unchanged KaTeX inventory below assets/katex. KaTeX CSS,
fonts and LICENSE stay together; occurrence CSS is concatenated in traversal
order. MathML-only output omits both KaTeX and occurrence stylesheet links/files.
No viewer JavaScript or Google Fonts request is added. Doc's optional Klee One
family falls back to installed/system fonts.

The fixed shell uses self-only stylesheet/font CSP and style-src-attr 'none'.
Host-selected image resources and cross-artifact hyperlinks are rejected rather
than silently omitted. Core HTML validation retains local-fragment target checks;
ordinary external navigation links may remain. All serialization, retained copies,
CSS and repeated asset emission use the caller's Budget. Browser-relative output
depth includes html/body and is capped at 256 independently of caller budget
depth. Shared-child traversal grows its frontier with metered allocation/copy.
Failures return no partially assembled bundle.

This is local byte assembly, not portable artifact admission or qualified browser
execution. Original diagnostics and termination observations remain reachable
through the source owner; assembly does not turn a cleanup failure into renderer
success. Browser loading/CSP/accessibility, visual fidelity, standalone file://
behavior, complete page/SVG-resource support and public publication remain
separate checks. Commands without `--native-host` retain their existing adapter selection.

## Explicit trusted host for standalone CLI export

`doc-html export --native-host host.json [--css external] [--math-renderer katex-preferred|mathml-only] input.nepld new-output-directory`

This opt-in route uses the existing native Node transport and closed Doc/Math
import. The existing commands without `--native-host` keep their current
capability behavior. The native route supports standalone Article/Code/Math with
external CSS; SVG/page bundles and native inline CSS are not connected here.
The CSS and renderer flags may be supplied separately or together in either
order after `--native-host`. `mathml-only` delegates to the independent existing
exporter before reading the host file, including when `--css inline` is selected.

The host JSON has exactly these fields (replace the example paths):

```json
{
  "node": "/absolute/path/to/node",
  "bridge": "/absolute/path/to/NEPL3/tools/math/katex/node/stdio.mjs",
  "modules_url": "file:///absolute/path/to/NEPL3/tools/audit/math/node_modules/",
  "katex_installation": "/absolute/path/to/NEPL3/tools/audit/math/node_modules/katex"
}
```

All filesystem selections must be absolute. No PATH search or relative-path
resolution is performed. `modules_url` must be a local `file:///` directory URL
with URL-encoded path characters and a trailing slash; the selected bridge
performs URL parsing. Unknown/duplicate fields and configurations above 16 KiB
are rejected. The integration does not install packages or discover another
runtime. Selected host code is trusted and can itself perform arbitrary I/O.
Configuration hashes identify selection bytes, not executable/bridge identity.

Native publication emits `document.html`, Doc/occurrence CSS, unchanged pinned
KaTeX CSS/fonts/LICENSE, and a `nepl3.trusted-native-doc-export/1` manifest.
Unsupported TeX and ordinary renderer-unavailable/render-error fallback retain
distinct reasons. Stops, provider/transport violations, missing required
termination observations, and reported failed termination prevent publication.
The supervisor survives returned generation errors and waits for direct-child
and pipe reclamation. Inherited pipes can delay completion; there is no hard
process-tree termination or OS-time bound.

Generation completes before output creation. The writer exclusively creates a
new directory, writes the manifest last, and attempts to remove that owned
directory on I/O failure; failed rollback is reported. This is not atomic
visibility. The independent MathML-only bypass retains its existing writer
semantics. The manifest size cap is an emitted-size bound after serialization,
not a peak allocation guarantee. Native generation accounting excludes parser
setup, report serialization and filesystem work. Browser rendering, font/CSP
loading, accessibility and portable execution identity remain unqualified.
