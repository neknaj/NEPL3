//! Consume the registered literal payload against its actual owner token.
use super::Error as PrefixError;
use crate::{portable, syntax::SentenceSyntax};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    syntax::{NodeRef, ValidatedSyntaxBundle},
    value::SchemaRef,
    value_codec::FoundationValueCodec,
};

#[derive(Debug, Eq, PartialEq)]
pub enum Error<E> {
    Stopped(StopReason),
    Syntax(PrefixError),
    Payload(portable::Error<E>),
    TokenMismatch(NodeRef),
}
impl<E> From<StopReason> for Error<E> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl<E> From<PrefixError> for Error<E> {
    fn from(e: PrefixError) -> Self {
        match e {
            PrefixError::Stopped(s) => Self::Stopped(s),
            e => Self::Syntax(e),
        }
    }
}
impl<E> From<portable::Error<E>> for Error<E> {
    fn from(e: portable::Error<E>) -> Self {
        match e {
            portable::Error::Stopped(s) => Self::Stopped(s),
            e => Self::Payload(e),
        }
    }
}
/// The standard Sentence literal is a root leaf. Decode its own schema against
/// the exact token owner snapshot and require identical head/View association.
/// This never reparses the source or accepts an old Doc payload as a fallback.
pub fn sentence<C: FoundationValueCodec>(
    input: &ValidatedSyntaxBundle<'_>,
    surface: &SchemaRef,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<SentenceSyntax, Error<C::Error>> {
    let checked = input
        .bundle()
        .validate_with_sources(registry, b, codec.source_admission())
        .map_err(PrefixError::from)?;
    let bundle = checked.bundle();
    let id = bundle.root;
    let root = bundle.node(id).map_err(PrefixError::from)?;
    b.charge(
        Resource::Work,
        (root.schema.package.len() + surface.package.len() + root.kind.len()) as u64 + 33,
    )?;
    if &root.schema != surface || root.kind != "Leaf:SentenceLiteral" || !root.fields.is_empty() {
        return Err(PrefixError::Unsupported(id).into());
    }
    let token = root
        .token
        .and_then(|t| bundle.tokens.get(t.0 as usize))
        .ok_or(Error::TokenMismatch(id))?;
    b.charge(
        Resource::Work,
        (token.head.snapshot_ref().source.0.len() as u64 + 40).saturating_mul(3),
    )?;
    if root.head.as_ref() != Some(&token.head) || root.cover.as_ref() != Some(&token.head) {
        return Err(Error::TokenMismatch(id));
    }
    let mut owner = None;
    for source in &bundle.sources {
        b.charge(
            Resource::Work,
            (source.identity().source.0.len() + token.head.snapshot_ref().source.0.len()) as u64
                + 40,
        )?;
        if source.identity() == token.head.snapshot_ref() {
            owner = Some(source);
            break;
        }
    }
    let owner = owner.ok_or(Error::TokenMismatch(id))?;
    // The reader's semantic arena is nested inside the syntax leaf. Include
    // that ownership depth while decoding, then restore the caller's base.
    b.with_depth_at_least(b.current_depth().saturating_add(1), |b| {
        let result = portable::literal::from_value(&token.payload, owner, registry, codec, b)?;
        let [view] = result.views.as_slice() else {
            return Err(Error::TokenMismatch(id));
        };
        // Budget the comparison without cloning either graph. Its bounded
        // strings and edges are also checked by the payload and token codecs.
        for value in [&view.view, &token.views] {
            b.charge(Resource::Work, value.roots.len() as u64 + 1)?;
            for element in &value.elements {
                b.charge(
                    Resource::Work,
                    (element.kind.schema.package.len() + element.span.snapshot_ref().source.0.len())
                        as u64
                        + 68,
                )?;
                for field in &element.fields {
                    b.charge(
                        Resource::Work,
                        (field.name.len() + field.children.len()) as u64 + 1,
                    )?;
                }
                for role in &element.roles {
                    b.charge(
                        Resource::Work,
                        (role.schema.package.len() + role.name.len()) as u64 + 34,
                    )?;
                }
                for relation in &element.relations {
                    b.charge(
                        Resource::Work,
                        (relation.schema.package.len() + relation.kind.len()) as u64 + 34,
                    )?;
                }
            }
        }
        b.charge(
            Resource::Work,
            (view.head.snapshot_ref().source.0.len() + token.head.snapshot_ref().source.0.len())
                as u64
                + 40,
        )?;
        if view.head != token.head || view.view != token.views {
            return Err(Error::TokenMismatch(id));
        }
        b.poll()?;
        Ok(result)
    })
}
