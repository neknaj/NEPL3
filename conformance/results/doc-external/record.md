# Doc page-set external links

Base: bf7db0b56008c6da7f9743746d3cd8644d09c8be. Root implementation and separate
fixed-snapshot review connect Doc External links to the existing typed Markup
http/https/mailto profile. Hidden variants are validated; other unresolved
requirements retain the original plan. No fetch or availability check occurs.

Root native Doc integration: 52 passed. Markup: 19 passed; local backend: 2 passed.
The two new production-source/NDF tests also passed on WASI. Independent complete
Doc integration: 55 native / 54 WASI; Markup: 19 on each. The three independent
Doc tests are included in these totals. Raw logs, fixed input hashes, scratch
negative probes and their corrections are under review/ and root/.

Four JavaScript-disabled Chromium/Firefox checks cover HTTP subpath and file
navigation, external href/text preservation without an automatic HTTPS request,
and the internal contract link with nested CSS. Results and runner are under
browser/. Screenshots are retained locally. WebKit is untested.

The host manifest scope was corrected and renderer identification advanced to
pages/2; this two-line change was independently reviewed. Actual export into
.tmp/migration-external-v2 succeeded. This is not full PreparedArticle, asset or
guest resolution, remote URI validation, Pages deployment or T21 completion.
Markdown remains canonical and required acceptance states are unchanged.
