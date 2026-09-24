use nepl3_doc_core::model::{DocContent, DocumentSyntax};

pub(super) fn assert_doc_retention(
    before: &DocumentSyntax,
    after: &DocumentSyntax,
) -> Result<(), String> {
    // Wire encoding canonicalizes source tables and guest graph coordinates.
    assert_eq!(before.value.root, after.value.root);
    assert_eq!(before.value.nodes, after.value.nodes);
    assert_eq!(before.origins, after.origins);
    assert_eq!(before.views, after.views);
    assert_eq!(before.source_maps, after.source_maps);
    assert_eq!(before.sources.len(), after.sources.len());
    for source in &before.sources {
        assert!(after.sources.contains(source));
    }
    assert_eq!(before.value.embeds.len(), after.value.embeds.len());
    for (before, after) in before.value.embeds.iter().zip(&after.value.embeds) {
        assert_eq!(before.kind, after.kind);
        match (&before.content, &after.content) {
            (DocContent::Syntax { closure: before }, DocContent::Syntax { closure: after }) => {
                assert_eq!(before.syntax.schema, after.syntax.schema);
                assert_eq!(before.syntax.category, after.syntax.category);
                assert_eq!(before.owner_environment, after.owner_environment);
                assert_eq!(before.provenance.origins(), after.provenance.origins());
                assert_eq!(
                    before.provenance.source_maps(),
                    after.provenance.source_maps()
                );
                assert_eq!(
                    before.provenance.sources().len(),
                    after.provenance.sources().len()
                );
                for source in before.provenance.sources() {
                    assert!(after.provenance.sources().contains(source));
                }
            }
            (DocContent::Value { value: before }, DocContent::Value { value: after }) => {
                assert_eq!(before, after);
            }
            _ => return Err("portable transport changed the content representation".into()),
        }
    }
    Ok(())
}
