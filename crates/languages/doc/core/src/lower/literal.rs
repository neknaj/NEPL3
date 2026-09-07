//! Accepted literal payloads join the host arena without changing the original
//! host Origin IDs or the owner-local ID spaces inside ForeignClosure.
use super::*;
use crate::portable::PortableError;
use nepl3_core::{
    origin::{Mapping, Origin, SourceMap},
    source::{SourceError, SourceSnapshot},
    value_codec::FoundationValueCodec,
    view::ViewBundle,
};

#[derive(Debug, Eq, PartialEq)]
pub enum DocumentLowerError<E> {
    Stopped(StopReason),
    Lower(LowerError),
    Payload {
        node: NodeRef,
        error: PortableError<E>,
    },
}
impl<E> From<LowerError> for DocumentLowerError<E> {
    fn from(e: LowerError) -> Self {
        match e {
            LowerError::Stopped(s) => Self::Stopped(s),
            e => Self::Lower(e),
        }
    }
}
impl<E> From<StopReason> for DocumentLowerError<E> {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
impl<E> From<SyntaxError> for DocumentLowerError<E> {
    fn from(e: SyntaxError) -> Self {
        LowerError::from(e).into()
    }
}
impl<E> From<ShapeError> for DocumentLowerError<E> {
    fn from(e: ShapeError) -> Self {
        LowerError::from(e).into()
    }
}
impl<E> From<StructureError> for DocumentLowerError<E> {
    fn from(e: StructureError) -> Self {
        LowerError::from(e).into()
    }
}

pub(super) trait Decoder {
    type Error: From<LowerError>
        + From<StopReason>
        + From<SyntaxError>
        + From<ShapeError>
        + From<StructureError>;
    fn admission(&mut self) -> &mut SourceAdmission;
    fn read(
        &mut self,
        id: NodeRef,
        node: &SyntaxNode,
        bundle: &nepl3_core::syntax::SyntaxBundle,
        registry: &SchemaRegistry,
        b: &mut Budget,
    ) -> Result<DocumentSyntax, Self::Error>;
}
pub(super) struct PrefixOnly<'a>(pub &'a mut SourceAdmission);
impl Decoder for PrefixOnly<'_> {
    type Error = LowerError;
    fn admission(&mut self) -> &mut SourceAdmission {
        self.0
    }
    fn read(
        &mut self,
        id: NodeRef,
        _node: &SyntaxNode,
        _bundle: &nepl3_core::syntax::SyntaxBundle,
        _registry: &SchemaRegistry,
        _b: &mut Budget,
    ) -> Result<DocumentSyntax, LowerError> {
        Err(LowerError::Unsupported { node: id })
    }
}
pub(super) struct Portable<'a, C>(pub &'a mut C);
impl<C: FoundationValueCodec> Decoder for Portable<'_, C> {
    type Error = DocumentLowerError<C::Error>;
    fn admission(&mut self) -> &mut SourceAdmission {
        self.0.source_admission()
    }
    fn read(
        &mut self,
        id: NodeRef,
        node: &SyntaxNode,
        bundle: &nepl3_core::syntax::SyntaxBundle,
        registry: &SchemaRegistry,
        b: &mut Budget,
    ) -> Result<DocumentSyntax, Self::Error> {
        let token = node
            .token
            .and_then(|id| bundle.tokens.get(id.0 as usize))
            .ok_or(LowerError::LiteralPayload { node: id })?;
        crate::portable::from_value(&token.payload, registry, self.0, b).map_err(
            |error| match error {
                PortableError::Stopped(s) => DocumentLowerError::Stopped(s),
                error => DocumentLowerError::Payload { node: id, error },
            },
        )
    }
}
pub(super) struct Presentation {
    sources: Vec<SourceSnapshot>,
    origins: Vec<Origin>,
    maps: Vec<Mapping>,
}
fn view_work(view: &ViewBundle, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::Work, view.roots.len() as u64 + 1)?;
    for element in &view.elements {
        b.charge(
            Resource::Work,
            (element.kind.schema.package.len() + element.span.snapshot_ref().source.0.len()) as u64
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
    Ok(())
}
impl Adapter<'_, '_> {
    pub(super) fn literal_at_depth<D: Decoder>(
        &mut self,
        id: NodeRef,
        node: &SyntaxNode,
        depth: u64,
        decoder: &mut D,
    ) -> Result<(), D::Error> {
        let Self {
            checked,
            registry,
            mapping,
            nodes,
            embeds,
            b,
            presentations,
            next_origin,
        } = self;
        b.with_depth_at_least(depth, |b| {
            let mut adapter = Adapter {
                checked,
                registry,
                mapping: core::mem::take(mapping),
                nodes: core::mem::take(nodes),
                embeds: core::mem::take(embeds),
                b,
                presentations: core::mem::take(presentations),
                next_origin: *next_origin,
            };
            let result = adapter.literal(id, node, decoder);
            *mapping = adapter.mapping;
            *nodes = adapter.nodes;
            *embeds = adapter.embeds;
            *presentations = adapter.presentations;
            *next_origin = adapter.next_origin;
            result
        })
    }
    pub(super) fn literal<D: Decoder>(
        &mut self,
        id: NodeRef,
        node: &SyntaxNode,
        decoder: &mut D,
    ) -> Result<(), D::Error> {
        let mut doc = decoder.read(id, node, self.checked.bundle(), self.registry, self.b)?;
        let DocRoot::Sentence(root) = doc.value.root else {
            return Err(LowerError::LiteralPayload { node: id }.into());
        };
        let token = node
            .token
            .and_then(|id| self.checked.bundle().tokens.get(id.0 as usize))
            .ok_or(LowerError::LiteralPayload { node: id })?;
        let [view] = doc.views.as_slice() else {
            return Err(LowerError::LiteralPayload { node: id }.into());
        };
        self.b.charge(
            Resource::Work,
            (view.head.snapshot_ref().source.0.len() + token.head.snapshot_ref().source.0.len())
                as u64
                + 33,
        )?;
        view_work(&view.view, self.b)?;
        view_work(&token.views, self.b)?;
        if view.head != token.head || view.view != token.views || !doc.value.embeds.is_empty() {
            return Err(LowerError::LiteralPayload { node: id }.into());
        }
        positions(&doc, &token.head, id, self.b)?;
        // These are the actual sentence grammar's constructors. A typed Doc
        // value alone must not smuggle an unrelated block or foreign element
        // into a SentenceLiteral leaf.
        for node in &doc.value.nodes {
            self.b.charge(Resource::Work, 1)?;
            if !matches!(
                node.kind,
                DocKind::Sentence { .. }
                    | DocKind::Text { .. }
                    | DocKind::Concat { .. }
                    | DocKind::Ruby { .. }
                    | DocKind::Anno { .. }
            ) {
                return Err(LowerError::LiteralPayload { node: id }.into());
            }
        }
        let first = self.nodes.len() as u64;
        let origin_base = self.next_origin;
        self.next_origin = self
            .next_origin
            .checked_add(doc.origins.len() as u64)
            .ok_or(SyntaxError::Reference)?;
        for origin in &mut doc.origins {
            let refs = match origin {
                Origin::Composite(refs) | Origin::Generated { inputs: refs, .. } => {
                    refs.as_mut_slice()
                }
                _ => &mut [],
            };
            for origin in refs {
                self.b.charge(Resource::Work, 1)?;
                origin.0 = origin
                    .0
                    .checked_add(origin_base)
                    .ok_or(SyntaxError::Reference)?;
            }
        }
        for mut node in doc.value.nodes {
            self.b.charge(Resource::Work, 1)?;
            crate::check::edges::rewrite(&mut node.kind, |index| {
                self.b.charge(Resource::Work, 1)?;
                index.checked_add(first).ok_or(ShapeError::Reference(index))
            })?;
            if let Some(origin) = &mut node.origin {
                origin.0 = origin
                    .0
                    .checked_add(origin_base)
                    .ok_or(SyntaxError::Reference)?;
            }
            push(&mut self.nodes, node, self.b)?;
        }
        self.mapping[id.0 as usize] = Some(Mapped::Node(
            root.0.checked_add(first).ok_or(SyntaxError::Reference)?,
        ));
        push(
            &mut self.presentations,
            Presentation {
                sources: doc.sources,
                origins: doc.origins,
                maps: doc.source_maps,
            },
            self.b,
        )?;
        Ok(())
    }
}
// General DocumentSyntax permits source-less constructors and independent
// source spans. A literal instead claims provenance within its single token.
// Use the common all-path mapping relation, without reconstructing its text.
fn positions(
    doc: &DocumentSyntax,
    head: &Span,
    id: NodeRef,
    b: &mut Budget,
) -> Result<(), LowerError> {
    let mut store = nepl3_core::source::SourceStore::default();
    for source in &doc.sources {
        store
            .insert(source.clone_with_budget(b).map_err(StructureError::from)?)
            .map_err(StructureError::from)?;
    }
    let maps =
        SourceMap::validate_mappings(&doc.source_maps, &store, b).map_err(StructureError::from)?;
    b.charge(
        Resource::Work,
        (doc.value.nodes.len() as u64)
            .saturating_add(doc.origins.len() as u64)
            .saturating_add(doc.source_maps.len() as u64),
    )?;
    let mut contains = |span: &Span| -> Result<(), LowerError> {
        b.charge(
            Resource::Work,
            (head.snapshot_ref().source.0.len() + span.snapshot_ref().source.0.len()) as u64 + 40,
        )?;
        if maps.contains(head, span, b).map_err(StructureError::from)? {
            Ok(())
        } else {
            Err(LowerError::LiteralPayload { node: id })
        }
    };
    for node in &doc.value.nodes {
        let span = node
            .span
            .as_ref()
            .ok_or(LowerError::LiteralPayload { node: id })?;
        if node.origin.is_none() {
            return Err(LowerError::LiteralPayload { node: id });
        }
        contains(span)?;
    }
    for origin in &doc.origins {
        match origin {
            Origin::Direct(span) => contains(span)?,
            Origin::Composite(inputs) if !inputs.is_empty() => {}
            Origin::Generated {
                callsite, inputs, ..
            } if !inputs.is_empty() => {
                if let Some(span) = callsite {
                    contains(span)?;
                }
            }
            Origin::Synthetic {
                anchor: Some(span), ..
            } => contains(span)?,
            _ => return Err(LowerError::LiteralPayload { node: id }),
        }
    }
    for map in &doc.source_maps {
        contains(&map.source)?;
        contains(&map.target)?;
    }
    Ok(())
}
pub(super) fn append(
    presentations: Vec<Presentation>,
    output: &mut DocumentSyntax,
    b: &mut Budget,
) -> Result<(), LowerError> {
    for presentation in presentations {
        for source in presentation.sources {
            let mut present = false;
            for prior in &output.sources {
                b.charge(
                    Resource::Work,
                    (source.identity().source.0.len() + prior.identity().source.0.len()) as u64
                        + 33,
                )?;
                if source.identity() == prior.identity() {
                    b.charge(
                        Resource::Work,
                        (source.uri().len()
                            + prior.uri().len()
                            + source.text().len()
                            + prior.text().len()) as u64,
                    )?;
                    if &source != prior {
                        return Err(SyntaxError::Source(SourceError::IdentityConflict).into());
                    }
                    present = true;
                    break;
                }
            }
            if !present {
                push(&mut output.sources, source, b)?;
            }
        }
        for origin in presentation.origins {
            push(&mut output.origins, origin, b)?;
        }
        for map in presentation.maps {
            push(&mut output.source_maps, map, b)?;
        }
    }
    Ok(())
}
