//! Native static SVG preparation for an explicitly resolved Doc namespace.
//! This neither selects Sentence owners nor adopts alt text as an HTML member.
use super::*;
use crate::assets::{AssetError, SvgInput, SvgMode};
use crate::namespace::display::PreparedDisplayNamespace;
use nepl3_doc_core::text::ResolvedInlineText;

/// Inputs are bound to an exact selected occurrence, not merely a document hash
/// or asset name. Equal asset names in different members have independent data.
pub struct MemberAssets<'a> {
    pub member: MemberId,
    pub inputs: &'a [SvgInput<'a>],
    pub resolved_alt: &'a [ResolvedInlineText],
}
#[derive(Debug)]
pub enum PreparationError<'a, E> {
    Stopped(StopReason),
    Input(nepl3_doc_core::prepare::NamespacePreparationError<'a, E>),
    InvalidMember(MemberId),
    DuplicateMember(MemberId),
    Member {
        member: MemberId,
        error: AssetError<'a, E>,
    },
}
impl<E> From<StopReason> for PreparationError<'_, E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
/// Reinspect all member sources with one codec, SourceAdmission and Budget.
/// Missing inputs, unsafe SVG, unused SVG inputs and unresolved links are errors.
/// Resolved alt text remains explicit identity-bound data, never portable proof.
/// Hosts must project actual selected guests with the same cumulative Budget
/// and SourceAdmission, retaining those owners through rendering.
/// No partial namespace is returned. Existing asset-free entries are unchanged.
#[allow(clippy::too_many_arguments)]
pub fn prepare<'a, C: FoundationValueCodec>(
    namespace: &CheckedNamespace<'_, 'a>,
    options: &'a RenderOptions,
    inputs: &[MemberAssets<'_>],
    mode: SvgMode,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedDisplayNamespace<'a>, PreparationError<'a, C::Error>> {
    budget.poll()?;
    let count = namespace.documents().len();
    for (index, input) in inputs.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        if usize::try_from(input.member.0)
            .ok()
            .is_none_or(|id| id >= count)
        {
            return Err(PreparationError::InvalidMember(input.member));
        }
        for prior in &inputs[..index] {
            budget.charge(Resource::Work, 1)?;
            if prior.member == input.member {
                return Err(PreparationError::DuplicateMember(input.member));
            }
        }
    }
    let plans =
        nepl3_doc_core::prepare::inspect_namespace_located(namespace, registry, codec, budget)
            .map_err(PreparationError::Input)?;
    let mut members = Vec::new();
    for (index, (document, plan)) in namespace.documents().zip(plans).enumerate() {
        let member = MemberId(index as u64);
        let mut selected = None;
        for input in inputs {
            budget.charge(Resource::Work, 1)?;
            if input.member == member {
                selected = Some(input);
                break;
            }
        }
        let (svg, alt) = selected.map_or((&[][..], &[][..]), |input| {
            (input.inputs, input.resolved_alt)
        });
        let prepared = crate::assets::prepare_svg_plan(
            document,
            options,
            svg,
            mode,
            registry,
            codec,
            budget,
            Some(true),
            alt,
            plan,
        )
        .map_err(|error| PreparationError::Member { member, error })?;
        build::push(&mut members, prepared, budget)?;
    }
    budget.poll()?;
    Ok(PreparedDisplayNamespace(PreparedNamespace { members }))
}
