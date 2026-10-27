//! Explicit native Body name composition, deliberately separate from legacy
//! namespace capabilities consumed by page and rendering preparation.
use super::*;
use nepl3_core::{diagnostic::Diagnostic, value_codec::FoundationValueCodec};

/// Structurally checked Body sites, with references pending explicit selection.
/// This is not a legacy member or a page/rendering admission capability.
///
/// ```compile_fail
/// use nepl3_doc_core::labels::namespace::{BodyMember, Member};
/// fn legacy<'p, 'a>(body: &'p BodyMember<'a>) -> &'p Member<'a> { body }
/// ```
pub struct BodyMember<'a>(Member<'a>);
impl<'a> BodyMember<'a> {
    pub fn document(&self) -> &'a DocumentSyntax {
        self.0.document()
    }
    pub fn definitions(&self) -> &[LabelSite<'a>] {
        self.0.definitions()
    }
    pub fn references(&self) -> &[LabelSite<'a>] {
        self.0.references()
    }
}

/// Inspect only a standalone Body. Existing `inspect` root gates are unchanged.
/// Foreign closures are not traversed, and unresolved names remain pending.
pub fn inspect_body<'a>(
    document: &'a DocumentSyntax,
    registry: &SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<BodyMember<'a>, Error<'a>> {
    let checked = document
        .validate_structure(registry, b, admission)
        .map_err(LabelError::from)?;
    let (_, category) = edges::root(document.value.root);
    if !matches!(document.value.root, DocRoot::Body(_)) {
        return Err(Error::Root(category));
    }
    inspect_structure(&checked, b).map(BodyMember)
}

/// Name proof for one explicit Body and the caller's selected legacy fragments.
/// Member zero is the Body; subsequent member IDs follow the fragment slice.
/// No title/language, displayed traversal, foreign meaning, asset preparation,
/// PageSet routing, or HTML/Markdown export follows from this proof.
///
/// ```compile_fail
/// use nepl3_doc_core::labels::namespace::{CheckedBodyNamespace, CheckedNamespace};
/// fn legacy<'p, 'm, 'a>(body: &'p CheckedBodyNamespace<'m, 'a>)
///     -> &'p CheckedNamespace<'m, 'a> { body }
/// ```
pub struct CheckedBodyNamespace<'m, 'a> {
    body: &'m BodyMember<'a>,
    fragments: &'m [&'m Member<'a>],
    definitions: Vec<NamespaceSite<'a>>,
    references: Vec<NamespaceReference<'a>>,
}
impl<'a> CheckedBodyNamespace<'_, 'a> {
    pub fn document(&self, member: MemberId) -> Option<&'a DocumentSyntax> {
        if member.0 == 0 {
            return Some(self.body.document());
        }
        usize::try_from(member.0 - 1)
            .ok()
            .and_then(|index| self.fragments.get(index))
            .map(|fragment| fragment.document())
    }
    pub fn documents(&self) -> impl Iterator<Item = &'a DocumentSyntax> + '_ {
        core::iter::once(self.body.document())
            .chain(self.fragments.iter().map(|fragment| fragment.document()))
    }
    pub fn definitions(&self) -> &[NamespaceSite<'a>] {
        &self.definitions
    }
    pub fn references(&self) -> &[NamespaceReference<'a>] {
        &self.references
    }
}

// The temporary legacy view never escapes. Charge before reserving and map both
// capacity arithmetic and physical reservation failures to a sticky typed stop.
fn selected_members<'m, 'a>(
    body: &'m BodyMember<'a>,
    fragments: &[&'m Member<'a>],
    b: &mut Budget,
) -> Result<Vec<&'m Member<'a>>, StopReason> {
    b.poll()?;
    let count = fragments
        .len()
        .checked_add(1)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    let work = u64::try_from(count).map_err(|_| b.stop(StopReason::WorkLimit))?;
    let bytes = count
        .checked_mul(core::mem::size_of::<&Member<'a>>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::Work, work)?;
    b.charge(Resource::AllocationUnits, bytes)?;
    let mut members = Vec::new();
    members
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    members.push(&body.0);
    for fragment in fragments {
        b.poll()?;
        members.push(*fragment);
    }
    Ok(members)
}

/// Resolve the Body followed by explicitly selected fragments in member order.
/// The caller remains responsible for using its shared admission across each
/// inspection; this operation proves names, not cross-member source admission.
pub fn resolve_body<'m, 'a>(
    body: &'m BodyMember<'a>,
    fragments: &'m [&'m Member<'a>],
    b: &mut Budget,
) -> Result<CheckedBodyNamespace<'m, 'a>, Error<'a>> {
    let members = selected_members(body, fragments, b)?;
    let CheckedNamespace {
        definitions,
        references,
        ..
    } = resolve(&members, b)?;
    Ok(CheckedBodyNamespace {
        body,
        fragments,
        definitions,
        references,
    })
}

/// Validate explicit site orders without claiming a host's display traversal.
pub fn resolve_body_interleaved<'m, 'a>(
    body: &'m BodyMember<'a>,
    fragments: &'m [&'m Member<'a>],
    definitions: &[DefinitionIndex],
    references: &[ReferenceIndex],
    b: &mut Budget,
) -> Result<CheckedBodyNamespace<'m, 'a>, Error<'a>> {
    let members = selected_members(body, fragments, b)?;
    let CheckedNamespace {
        definitions,
        references,
        ..
    } = resolve_interleaved(&members, definitions, references, b)?;
    Ok(CheckedBodyNamespace {
        body,
        fragments,
        definitions,
        references,
    })
}

impl Error<'_> {
    /// Materialize only semantic Body-composition errors using the same ordered
    /// owners as `resolve_body`. Forged sites are checked by the legacy owner
    /// validator; its member references and capabilities are never returned.
    pub fn diagnostic_body<C: FoundationValueCodec>(
        &self,
        body: &BodyMember<'_>,
        fragments: &[&Member<'_>],
        registry: &SchemaRegistry,
        codec: &mut C,
        b: &mut Budget,
    ) -> Result<Diagnostic, LabelDiagnosticError<C::Error>> {
        b.poll()?;
        match self {
            Self::Stopped(reason) => return Err(LabelDiagnosticError::Stopped(b.stop(*reason))),
            Self::Input(_) | Self::Root(_) | Self::Order(_) => {
                return Err(LabelDiagnosticError::NotSemantic);
            }
            Self::Duplicate { .. } | Self::Unresolved { .. } => {}
        }
        let members = selected_members(body, fragments, b)?;
        self.diagnostic(&members, registry, codec, b)
    }
}
