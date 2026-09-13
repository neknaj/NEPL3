# Standalone Sentence preparation and HTML

Math annotations need Doc phrasing output without fabricating an Article or
discarding Ruby/Anno through plain-text projection. The existing Article renderer
already owns these inline transformations, so standalone Sentence rendering uses
the same builder and stylesheet.

`CheckedSentenceLabels` and `PreparedLocalSentence` are separate borrowed proofs;
they cannot authorize an Article operation. Standalone labels are local to that
Sentence, including forward references and duplicate occurrence checks. Article
rendering retains its existing whole-Article namespace. No surrounding labels or
foreign meanings are implicitly imported.

The existing neutral DocumentSyntax and DocPreparationPlan representations already
carry the root, full input identity and requirements. Their field layout does not
change. Plan codecs rederive the plan according to the supplied document root;
decoding a plan is never authority to render. Links, assets and foreign closures
still require explicit resolution, and local preparation returns that requirement
list instead of emitting placeholders.

This establishes the Doc-side phrasing path. Host selection of a Doc annotation,
Math label composition and final resource delivery remain separate work; this
change does not declare the full Math display or Doc preparation acceptance done.
