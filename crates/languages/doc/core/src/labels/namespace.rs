//! Explicit composition of document-owned label scopes. Foreign closures are
//! not traversed: the host selects and lowers each participating Doc fragment.
use super::*;
#[cfg(test)]
mod tests;

/// One display occurrence in the caller's ordered member list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemberId(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamespaceSite<'a> {
    pub member: MemberId,
    pub site: LabelSite<'a>,
}
impl index::Named for NamespaceSite<'_> {
    fn name(&self) -> &str {
        self.site.name
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamespaceReference<'a> {
    pub reference: NamespaceSite<'a>,
    /// Index into this checked namespace's declaration sequence.
    pub target: DocLabelId,
}
#[derive(Debug, Eq, PartialEq)]
pub enum Error<'a> {
    Stopped(StopReason),
    Input(LabelError<'a>),
    Root(crate::check::Category),
    Duplicate {
        definition: NamespaceSite<'a>,
        previous: NamespaceSite<'a>,
    },
    Unresolved {
        reference: NamespaceSite<'a>,
    },
}
impl From<StopReason> for Error<'_> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl<'a> From<LabelError<'a>> for Error<'a> {
    fn from(error: LabelError<'a>) -> Self {
        match error {
            LabelError::Stopped(reason) => Self::Stopped(reason),
            error => Self::Input(error),
        }
    }
}
/// Immutable structurally validated input. References are intentionally pending
/// until the complete explicitly selected document namespace is available.
pub struct Member<'a> {
    document: &'a DocumentSyntax,
    definitions: Vec<LabelSite<'a>>,
    references: Vec<LabelSite<'a>>,
}
impl<'a> Member<'a> {
    pub fn document(&self) -> &'a DocumentSyntax {
        self.document
    }
    pub(super) fn has_definition(
        &self,
        site: LabelSite<'_>,
        b: &mut Budget,
    ) -> Result<bool, StopReason> {
        contains_site(&self.definitions, site, b)
    }
    pub(super) fn has_reference(
        &self,
        site: LabelSite<'_>,
        b: &mut Budget,
    ) -> Result<bool, StopReason> {
        contains_site(&self.references, site, b)
    }
}
fn contains_site(
    sites: &[LabelSite<'_>],
    site: LabelSite<'_>,
    b: &mut Budget,
) -> Result<bool, StopReason> {
    b.poll()?;
    for candidate in sites {
        let mut work = (candidate.name.len() as u64)
            .saturating_add(site.name.len() as u64)
            .saturating_add(256);
        for span in [
            candidate.selection,
            candidate.range,
            site.selection,
            site.range,
        ]
        .into_iter()
        .flatten()
        {
            work = work.saturating_add(span.snapshot_ref().source.0.len() as u64);
        }
        b.charge(Resource::Work, work)?;
        if *candidate == site {
            return Ok(true);
        }
    }
    Ok(false)
}
pub fn inspect<'a>(
    document: &'a DocumentSyntax,
    registry: &SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<Member<'a>, Error<'a>> {
    let checked = document
        .validate_structure(registry, b, admission)
        .map_err(LabelError::from)?;
    inspect_structure(&checked, b)
}
pub(crate) fn inspect_structure<'a>(
    checked: &crate::check::ValidatedDocumentSyntax<'a>,
    b: &mut Budget,
) -> Result<Member<'a>, Error<'a>> {
    b.poll()?;
    let document = checked.document();
    let (root, category) = edges::root(document.value.root);
    if !matches!(
        document.value.root,
        DocRoot::Article(_) | DocRoot::Sentence(_) | DocRoot::Inline(_)
    ) {
        return Err(Error::Root(category));
    }
    let (definitions, references) = collect_sites(document, checked.shape().postorder(), root, b)?;
    // Duplicate names within a single member retain the ordinary diagnostic.
    index::build(&definitions, b)?;
    Ok(Member {
        document,
        definitions,
        references,
    })
}
/// Proof for exactly the selected immutable member sequence. This is a name
/// resolution proof, not a foreign-operation or HTML safety proof.
pub struct CheckedNamespace<'m, 'a> {
    members: &'m [&'m Member<'a>],
    definitions: Vec<NamespaceSite<'a>>,
    references: Vec<NamespaceReference<'a>>,
}
impl<'a> CheckedNamespace<'_, 'a> {
    pub fn documents(&self) -> impl ExactSizeIterator<Item = &'a DocumentSyntax> + '_ {
        self.members.iter().map(|member| member.document)
    }
    pub fn document(&self, member: MemberId) -> Option<&'a DocumentSyntax> {
        usize::try_from(member.0)
            .ok()
            .and_then(|index| self.members.get(index))
            .map(|member| member.document)
    }
    pub fn definitions(&self) -> &[NamespaceSite<'a>] {
        &self.definitions
    }
    pub fn references(&self) -> &[NamespaceReference<'a>] {
        &self.references
    }
}
/// The sequence supplies display occurrences, not a set of unique documents.
/// Selecting the same declaration-bearing member twice is a duplicate, while
/// repeated reference-only fragments resolve independently. Declaration IDs
/// preserve member/constructor order; an auxiliary sorted index resolves names.
pub fn resolve<'m, 'a>(
    members: &'m [&'m Member<'a>],
    b: &mut Budget,
) -> Result<CheckedNamespace<'m, 'a>, Error<'a>> {
    b.poll()?;
    let mut definitions = Vec::new();
    for (number, input) in members.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let member = MemberId(number as u64);
        for site in &input.definitions {
            b.charge(Resource::Work, 1)?;
            push(
                &mut definitions,
                NamespaceSite {
                    member,
                    site: *site,
                },
                b,
            )?;
        }
    }
    let names = index::build_for(&definitions, b).map_err(|error| match error {
        index::IndexError::Stopped(reason) => Error::Stopped(reason),
        index::IndexError::Duplicate { previous, current } => Error::Duplicate {
            definition: definitions[current],
            previous: definitions[previous],
        },
    })?;
    let mut references = Vec::new();
    for (number, input) in members.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let member = MemberId(number as u64);
        for site in &input.references {
            b.charge(Resource::Work, 1)?;
            let reference = NamespaceSite {
                member,
                site: *site,
            };
            let target = index::find(&names, &definitions, site.name, b)?
                .ok_or(Error::Unresolved { reference })?;
            push(&mut references, NamespaceReference { reference, target }, b)?;
        }
    }
    Ok(CheckedNamespace {
        members,
        definitions,
        references,
    })
}
