// This negative compile probe must not build: a live syntax proof borrows its tree.
fn mutate_while_borrowed(
    tree: &mut nepl3_engine::recovery::ParseTree,
    profile: &nepl3_engine::profile::ResolvedParseProfile<'_>,
    budget: &mut nepl3_core::budget::Budget,
    admission: &mut nepl3_core::source::SourceAdmission,
) -> Result<(), nepl3_engine::tree::TreeError> {
    let checked = tree.validate(profile, budget, admission)?;
    tree.bundle.nodes.clear();
    let _ = checked.syntax().bundle();
    Ok(())
}
