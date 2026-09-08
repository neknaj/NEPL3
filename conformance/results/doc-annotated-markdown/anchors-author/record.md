# Chapter 13: manual legacy-anchor mapping

This is an author-prepared mapping for separate independent review. It does not change production, schema, either source document, the page registry or canonical status.

Fixed source commit: `2b05110490238656d088c33bbad601784777d657`.

- Canonical `doc/spec/13-reproducibility.md`: SHA-256 `81d032e6f04730fd4c52a15c11464bff0ed879c1030a059be19d10b4d8c9a16d`, 14,922 bytes.
- Authored `doc/migration/authored/13-reproducibility.nepld`: SHA-256 `31486ad75e0776d55cc12b2a9b5af2840780ad7812456d9c222a90fd8ab495a8`, 30,905 bytes.

The original headings and the authored title literals were read directly. Their base wording is identical; Ruby supplies readings without changing the heading text. The section bodies were read to identify the corresponding meaning, not assigned by guessing a slug from the Section ID. `mapping.json` records the manual correspondence and derivation for each candidate. `checked-mapping.json` records exact source lines/excerpts and observed GitHub heading attributes.

| Original heading | Doc target | Legacy fragment (without `#`) |
| --- | --- | --- |
| 13. 再現性・schema識別・契約の判定 | Article title; no Section ID | `13-再現性schema識別契約の判定` |
| 方針 | Section `policy` | `方針` |
| 1. digest | Section `digest` | `1-digest` |
| Package意味正規形 | Section `semantic`, child of `digest` | `package意味正規形` |
| 具体実行identity | Section `execution`, child of `digest` | `具体実行identity` |
| 2. native値とwire値 | Section `wire` | `2-native値とwire値` |
| 3. normal formとartifact | Section `artifacts` | `3-normal-formとartifact` |
| 4. limitsとproviderの互換 | Section `limits` | `4-limitsとproviderの互換` |

## Derivation and observed evidence

GitHub documents lowercase conversion, space-to-hyphen conversion, removal of other whitespace/punctuation and formatting, and numbering repeated heading anchors. These rules were applied by hand to the eight original headings, before comparison to the retrieved rendered page. In particular, the article title loses the period and both Japanese middle dots; `Package` becomes `package`. [Official section-link rules](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#section-links).

The fixed-commit [actual GitHub file page](https://github.com/neknaj/NEPL3/blob/2b05110490238656d088c33bbad601784777d657/doc/spec/13-reproducibility.md) was fetched over HTTPS. Its server-generated HTML is preserved as `github-page.html`, SHA-256 `6f43d6837100eb5a52df4ea0d530b085d79633bc80431870873fe5d73e8e3624`. Each of the eight heading elements is followed by a permalink anchor whose `href` equals `#` plus the manual candidate and whose DOM ID equals `user-content-` plus that candidate. Heading text and levels also match the fixed Markdown. Thus these values are observed current GitHub-rendered hrefs, not unverified algorithm guesses. The `user-content-` prefix is an observed DOM implementation detail, **not** the public fragment to put in the manual mapping.

The scraper selects the actual `heading-element` elements from the returned HTML. An initial overly broad extraction also matched GitHub navigation headings; it was corrected to this explicit selector before the final eight-row assertions. No canonical or authored document was changed. The evidence does not rely on a locally installed slug library or on automatically slugging annotated Doc headings.

No browser click/scroll test was performed. HTTP retrieval does not send the fragment to the server, so a successful page GET is not itself proof of fragment navigation. This records the actual emitted href/ID correspondence at the fixed commit, not a guarantee about future GitHub rendering, the new Markdown projection or independent acceptance. New aliases still need renderer/parser and target-view verification. GitHub documents custom named anchors separately, including their omission from the automatic outline. [Official custom anchors](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#custom-anchors).

## Meaning and collision boundaries

- The Article title is one distinct output target. The Doc Article constructor has no Section ID here. Record `target_kind: article-title, section_id: null`; do not create a fictitious `title` Section or attach the title alias to the first `policy` Section.
- There are seven unique Section IDs and eight unique observed legacy fragments. None of the legacy fragments equals one of these seven Section IDs. Two subsections remain children of `digest`; preserving heading order alone without this hierarchy would be insufficient.
- `policy` describes general reproducibility policy, `digest` the hashing/canonical descriptor contract, `semantic` its package semantic-normal-form subsection, and `execution` concrete execution/provenance identity. `wire`, `artifacts` and `limits` cover native/wire identity, output normalization and resource/provider compatibility respectively. `mapping.json` contains per-row details.
- The collision check is limited to this old page's eight heading fragments and the seven local semantic ID strings. It does not prove absence of collisions in the new annotated output. Ruby/Anno readings added to a visible heading can produce different automatic slugs; a generated old alias may collide with another newly generated or explicit anchor. The new renderer must check the complete emitted anchor set, including its own automatic headings and article title.
- A title edit or movement of repeated headings must not silently regenerate the recorded legacy mapping. The binding needs the reviewed source/draft identity and explicit output page identity. These are local fragments, not globally unique page IDs or route declarations. Percent-encoded URL serialization must preserve their Unicode scalar content rather than invent a normalized spelling.
- The mapping does not authorize resolving an arbitrary old fragment by heading-text guesses. It is a finite, manually reviewed alias table for this page. It leaves code, two RFC external links, all annotations and the documents' full body untouched.

## Validation scope

Successful self-checks: eight original heading texts/levels/order; exact authored title-literal base wording; seven unique Section IDs with manually specified parent relation; eight unique candidate fragments; all eight candidates matched actual GitHub-rendered href/DOM ID pairs. Source excerpts, complete source blobs and payload hashes are retained.

Not executed: production Doc parse/lower, new Markdown renderer, portable request/reply, browser fragment navigation, accessibility, independent review or canonical cutover. The small heading-literal check is not a general Doc parser and does not substitute for those stages.
