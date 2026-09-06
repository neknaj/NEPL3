//! Static head selection uses the original lexeme, never a reader's semantic payload as a spelling.
use crate::{
    package::{EntryContext, LanguagePackage, PackageError, ReadSpecId},
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
pub(super) fn category<'a>(
    package: &'a LanguagePackage,
    entry: &EntryContext,
    token: &Token,
    source: &SourceSnapshot,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Option<Head<'a>>, PackageError> {
    let raw = source.slice(&token.head)?;
    for (index, form) in package.forms.iter().enumerate() {
        budget.charge(
            Resource::Work,
            (entry.category.len() + raw.len()) as u64 + 1,
        )?;
        if form.category == entry.category && form.spelling == raw {
            return Ok(Some(Head {
                kind: &form.kind,
                selection: ShapeSelection::Form {
                    index: index as u64,
                },
                arity: form.fields.len() as u64,
            }));
        }
    }
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
