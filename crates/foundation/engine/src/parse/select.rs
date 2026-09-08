//! Static head selection uses the original lexeme, never a reader's semantic payload as a spelling.
use crate::{
    package::{CheckedLanguagePackage, EntryContext, LanguagePackage, PackageError, ReadSpecId},
    profile::{ProfileError, ResolvedParseProfile},
    selection::ShapeSelection,
};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::SchemaRegistry,
    source::SourceSnapshot,
    value::KindRef,
    view::Token,
};

pub(super) struct Head<'a> {
    pub kind: &'a KindRef,
    pub selection: ShapeSelection,
    pub arity: u64,
}
pub(super) fn form<'a>(
    package: &CheckedLanguagePackage<'a>,
    entry: &EntryContext,
    token: &Token,
    source: &SourceSnapshot,
    budget: &mut Budget,
) -> Result<Option<Head<'a>>, PackageError> {
    let raw = source.slice(&token.head)?;
    if let Some((index, form)) = package.form(&entry.category, raw, budget)? {
        return Ok(Some(Head {
            kind: &form.kind,
            selection: ShapeSelection::Form {
                index: index as u64,
            },
            arity: form.fields.len() as u64,
        }));
    }
    Ok(None)
}
pub(super) fn leaf<'a>(
    package: &'a LanguagePackage,
    entry: &EntryContext,
    token: &Token,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Option<Head<'a>>, PackageError> {
    for (index, leaf) in package.leaves.iter().enumerate() {
        budget.charge(
            Resource::Work,
            (entry.category.len() + token.kind.schema.package.len()) as u64 + 34,
        )?;
        if leaf.category == entry.category && leaf.token_kind == token.kind {
            registry.validate(&leaf.payload, &token.payload, budget)?;
            return Ok(Some(Head {
                kind: &leaf.kind,
                selection: ShapeSelection::Leaf {
                    index: index as u64,
                },
                arity: 0,
            }));
        }
    }
    Ok(None)
}

pub(super) fn read(
    profile: &ResolvedParseProfile<'_>,
    parent: &EntryContext,
    id: ReadSpecId,
    budget: &mut Budget,
) -> Result<crate::profile::ResolvedRead, ProfileError> {
    profile.read_entry(parent, id, budget)
}
