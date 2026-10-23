<!-- Generated from doc/decisions/standalone&#45;sentence&#45;html.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page standalone&#45;sentence&#45;html; source SHA-256 ee498619a9b8d6c48ed32b434a3096f94154965302a6d3ed0edcec04b50bb606; alias input SHA-256 7923b0005ce79672aa3224d73de4ba2f5e5b39c422cb619db0957ba2c6a1e227; page input SHA-256 be63b4cedbe26bd3ff5bcf863f1ed13ff9a75ac6d10b32850193e76cb2ae9e22. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="standalone-sentence-preparation-and-html"></a>

# Standalone Sentence preparation and HTML

[正本（NEPL3d）](<standalone-sentence-html.nepld>)

Math annotations need Doc phrasing output without fabricating an Article or discarding Ruby\/Anno through plain\-text projection\. The existing Article renderer already owns these inline transformations\, so standalone Sentence rendering uses the same builder and stylesheet\.

`CheckedSentenceLabels` and `PreparedLocalSentence` are separate borrowed proofs\; they cannot authorize an Article operation\. Standalone labels are local to that Sentence\, including forward references and duplicate occurrence checks\. Article rendering retains its existing whole\-Article namespace\. No surrounding labels or foreign meanings are implicitly imported\.

The existing neutral DocumentSyntax and DocPreparationPlan representations already carry the root\, full input identity and requirements\. Their field layout does not change\. Plan codecs rederive the plan according to the supplied document root\; decoding a plan is never authority to render\. Links\, assets and foreign closures still require explicit resolution\, and local preparation returns that requirement list instead of emitting placeholders\.

This establishes the Doc\-side phrasing path\. Host selection of a Doc annotation\, Math label composition and final resource delivery remain separate work\; this change does not declare the full Math display or Doc preparation acceptance done\.
