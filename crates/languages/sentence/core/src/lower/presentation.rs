//! Standard literal/prefix entry preserving the source presentation boundary.
use super::{ForeignInlineForm, literal, prefix_with_foreign};
use crate::{
    model::Root,
    syntax::{NodeLocation, SentenceSyntax, SentenceView},
};
use alloc::{vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::Span,
    syntax::{ForeignClosure, SyntaxError, ValidatedOwnerProvenance, ValidatedSyntaxBundle},
    value::SchemaRef,
    value_codec::FoundationValueCodec,
};

#[derive(Debug, Eq, PartialEq)]
pub enum Error<E> {
    Stopped(StopReason),
    Prefix(super::Error),
    Literal(literal::Error<E>),
    Presentation(crate::syntax::Error),
    Mapping,
    Closure(SyntaxError),
    Selection,
    Category,
}
impl<E> From<StopReason> for Error<E> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl<E> From<super::Error> for Error<E> {
    fn from(e: super::Error) -> Self {
        match e {
            super::Error::Stopped(s) => Self::Stopped(s),
            e => Self::Prefix(e),
        }
    }
}
impl<E> From<literal::Error<E>> for Error<E> {
    fn from(e: literal::Error<E>) -> Self {
        match e {
            literal::Error::Stopped(s) => Self::Stopped(s),
            e => Self::Literal(e),
        }
    }
}
impl<E> From<crate::syntax::Error> for Error<E> {
    fn from(e: crate::syntax::Error) -> Self {
        match e {
            crate::syntax::Error::Stopped(s) => Self::Stopped(s),
            e => Self::Presentation(e),
        }
    }
}
fn span(value: &Span, b: &mut Budget) -> Result<Span, StopReason> {
    value.clone_with_budget(b)
}
fn push<T>(out: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    out.push(value);
    Ok(())
}
/// Standard Sentence and Inline forms use the prefix projection; the literal
/// leaf uses its exact owner-token decoder. No guest execution or legacy Doc
/// codec is selected. Auxiliary token views belong to the root presentation;
/// form-head views belong to their corresponding meaning node.
pub fn sentence<C: FoundationValueCodec>(
    input: &ValidatedSyntaxBundle<'_>,
    surface: &SchemaRef,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<SentenceSyntax, Error<C::Error>> {
    sentence_with_foreign(input, surface, &[], registry, codec, b)
}

/// Preserve the source closure for host-selected foreign-inline forms. Guest
/// syntax remains unevaluated and keeps its own local provenance identifiers.
pub fn sentence_with_foreign<C: FoundationValueCodec>(
    input: &ValidatedSyntaxBundle<'_>,
    surface: &SchemaRef,
    foreign_forms: &[ForeignInlineForm<'_>],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<SentenceSyntax, Error<C::Error>> {
    let bundle = input.bundle();
    let root = bundle.node(bundle.root).map_err(super::Error::from)?;
    b.charge(Resource::Work, root.kind.len() as u64 + 1)?;
    if root.kind == "Leaf:SentenceLiteral" {
        return Ok(literal::sentence(input, surface, registry, codec, b)?);
    }
    let projected = prefix_with_foreign(
        input,
        surface,
        foreign_forms,
        registry,
        b,
        codec.source_admission(),
    )?;
    finish_prefix(projected, bundle, registry, codec, b)
}

/// Lower selected closures against one immutable registry. Owner provenance
/// may be reused; every guest is validated at the caller's current depth and
/// source admission. The fresh guest proof is consumed by the private lowering
/// helpers in this call, with no second graph traversal and no guest execution.
pub struct ClosureLowerer<'a> {
    registry: &'a SchemaRegistry,
    owner: Option<ValidatedOwnerProvenance<'a>>,
}
impl<'a> ClosureLowerer<'a> {
    pub fn new(registry: &'a SchemaRegistry) -> Self {
        Self {
            registry,
            owner: None,
        }
    }

    pub fn lower<C: FoundationValueCodec>(
        &mut self,
        closure: &'a ForeignClosure,
        surface: &SchemaRef,
        forms: &[ForeignInlineForm<'_>],
        codec: &mut C,
        b: &mut Budget,
    ) -> Result<SentenceSyntax, Error<C::Error>> {
        b.poll()?;
        let result = (|| {
            b.charge(Resource::Work, 1)?;
            if !self
                .owner
                .as_ref()
                .is_some_and(|proof| proof.matches_owner(&closure.provenance))
            {
                self.owner = Some(
                    closure
                        .provenance
                        .validate(self.registry, b, codec.source_admission())
                        .map_err(Error::Closure)?,
                );
            }
            let checked = self
                .owner
                .as_ref()
                .ok_or(Error::Mapping)?
                .validate_closure(closure, b, codec.source_admission())
                .map_err(Error::Closure)?;
            b.charge(
                Resource::Work,
                (closure.syntax.schema.package.len()
                    + surface.package.len()
                    + closure.syntax.category.len()) as u64
                    + 40,
            )?;
            if &closure.syntax.schema != surface
                || !matches!(closure.syntax.category.as_str(), "Sentence" | "Inline")
            {
                return Err(Error::Selection);
            }
            let input = checked.syntax();
            let bundle = input.bundle();
            let root = bundle.node(bundle.root).map_err(super::Error::from)?;
            b.charge(Resource::Work, root.kind.len() as u64 + 1)?;
            let output = if root.kind == "Leaf:SentenceLiteral" {
                literal::sentence_checked(input, surface, self.registry, codec, b)?
            } else {
                let projected = super::prefix_checked(
                    input,
                    surface,
                    forms,
                    self.registry,
                    b,
                    codec.source_admission(),
                )?;
                finish_prefix(projected, bundle, self.registry, codec, b)?
            };
            if !matches!(
                (closure.syntax.category.as_str(), output.value.root),
                ("Sentence", Root::Sentence(_)) | ("Inline", Root::Inline(_))
            ) {
                return Err(Error::Category);
            }
            Ok(output)
        })();
        b.poll()?;
        result
    }
}

fn finish_prefix<C: FoundationValueCodec>(
    projected: super::Projection,
    bundle: &nepl3_core::syntax::SyntaxBundle,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<SentenceSyntax, Error<C::Error>> {
    let root = match projected.value.root {
        Root::Sentence(r) => r.0,
        Root::Inline(r) => r.0,
    };
    b.charge(
        Resource::AllocationUnits,
        (projected.value.nodes.len() as u64)
            .saturating_mul(core::mem::size_of::<Option<NodeLocation>>() as u64),
    )?;
    let mut locations = vec![None; projected.value.nodes.len()];
    b.charge(
        Resource::AllocationUnits,
        (bundle.tokens.len() as u64).saturating_mul(core::mem::size_of::<u64>() as u64),
    )?;
    let mut token_owners = vec![root; bundle.tokens.len()];
    for (node, mapped) in bundle.nodes.iter().zip(&projected.syntax_to_meaning) {
        b.charge(Resource::Work, 1)?;
        let Some(index) = mapped else {
            continue;
        };
        let slot = locations.get_mut(*index as usize).ok_or(Error::Mapping)?;
        if slot.is_some() {
            return Err(Error::Mapping);
        }
        *slot = Some(NodeLocation {
            origin: node.origin,
            head: node.head.as_ref().map(|s| span(s, b)).transpose()?,
            cover: node.cover.as_ref().map(|s| span(s, b)).transpose()?,
        });
        if let Some(token) = node.token {
            *token_owners
                .get_mut(token.0 as usize)
                .ok_or(Error::Mapping)? = *index;
        }
    }
    let mut output = SentenceSyntax {
        value: projected.value,
        locations: Vec::new(),
        sources: Vec::new(),
        origins: Vec::new(),
        views: Vec::new(),
        source_maps: Vec::new(),
    };
    for location in locations {
        push(&mut output.locations, location.ok_or(Error::Mapping)?, b)?;
    }
    for source in &bundle.sources {
        push(&mut output.sources, source.clone_with_budget(b)?, b)?;
    }
    for origin in &bundle.origins {
        push(&mut output.origins, origin.clone_with_budget(b)?, b)?;
    }
    for map in &bundle.source_maps {
        push(&mut output.source_maps, map.clone_with_budget(b)?, b)?;
    }
    for (token, owner) in bundle.tokens.iter().zip(token_owners) {
        let view = SentenceView {
            owner,
            head: span(&token.head, b)?,
            view: token.views.clone_with_budget(b)?,
        };
        push(&mut output.views, view, b)?;
    }
    output.validate(registry, b, codec.source_admission())?;
    Ok(output)
}
