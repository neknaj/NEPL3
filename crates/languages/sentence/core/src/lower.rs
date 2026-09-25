//! Standard prefix-to-meaning projection. The caller retains the syntax bundle
//! with its source, origins and views; this operation does not erase that graph.
use crate::{check, model::*};
pub mod literal;
pub mod presentation;
use alloc::{string::String, vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::SourceAdmission,
    syntax::{
        FieldValue, ForeignCapture, NodeRef, SyntaxBundle, SyntaxError, SyntaxNode,
        ValidatedSyntaxBundle,
    },
    value::{NdfValue, SchemaRef},
};

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    Syntax(SyntaxError),
    Shape(check::Error),
    Unsupported(NodeRef),
    Operand { node: NodeRef, field: usize },
}
impl From<StopReason> for Error {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
impl From<SyntaxError> for Error {
    fn from(e: SyntaxError) -> Self {
        match e.stop_reason() {
            Some(s) => Self::Stopped(s),
            None => Self::Syntax(e),
        }
    }
}
impl From<check::Error> for Error {
    fn from(e: check::Error) -> Self {
        match e {
            check::Error::Stopped(s) => Self::Stopped(s),
            e => Self::Shape(e),
        }
    }
}
/// Mapping is tied to the exact input bundle, not a reusable node identity.
/// Builtin operands and list constructors have no separate semantic node.
pub struct Projection {
    pub value: SentenceValue,
    pub syntax_to_meaning: Vec<Option<u64>>,
}
/// Host-selected one-field Inline form in the selected Sentence surface.
/// The foreign field must match the complete guest schema identity and category.
/// This selection captures syntax; it grants no permission to execute a guest.
pub struct ForeignInlineForm<'a> {
    pub kind: &'a str,
    pub guest_schema: &'a SchemaRef,
    pub guest_category: &'a str,
}
fn push<T>(v: &mut Vec<T>, x: T, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    v.push(x);
    Ok(())
}
fn child(n: &SyntaxNode, id: NodeRef, field: usize) -> Result<NodeRef, Error> {
    match n.fields.get(field) {
        Some(FieldValue::Child(c)) => Ok(*c),
        _ => Err(Error::Operand { node: id, field }),
    }
}
struct Adapter<'a> {
    bundle: &'a SyntaxBundle,
    mapping: Vec<Option<u64>>,
    nodes: Vec<Kind>,
}
impl Adapter<'_> {
    fn node(&self, id: NodeRef) -> Result<&SyntaxNode, Error> {
        self.bundle
            .nodes
            .get(id.0 as usize)
            .ok_or(SyntaxError::Reference.into())
    }
    fn inline(&self, id: NodeRef, n: &SyntaxNode, field: usize) -> Result<InlineRef, Error> {
        let c = child(n, id, field)?;
        let index = self
            .mapping
            .get(c.0 as usize)
            .copied()
            .flatten()
            .ok_or(Error::Operand { node: id, field })?;
        if matches!(
            self.nodes.get(index as usize),
            None | Some(Kind::Sentence { .. })
        ) {
            return Err(Error::Operand { node: id, field });
        }
        Ok(InlineRef(index))
    }
    fn list(
        &self,
        id: NodeRef,
        n: &SyntaxNode,
        field: usize,
        b: &mut Budget,
    ) -> Result<Vec<InlineRef>, Error> {
        let mut cursor = child(n, id, field)?;
        let mut out = Vec::new();
        loop {
            b.charge(Resource::Work, 1)?;
            let n = self.node(cursor)?;
            b.charge(Resource::Work, n.kind.len() as u64)?;
            if !n.kind.starts_with("List:") {
                return Err(Error::Operand { node: id, field });
            }
            match n.fields.len() {
                0 => break,
                2 => {
                    push(&mut out, self.inline(cursor, n, 0)?, b)?;
                    cursor = child(n, cursor, 1)?;
                }
                _ => return Err(Error::Operand { node: id, field }),
            }
        }
        Ok(out)
    }
    fn text(
        &self,
        id: NodeRef,
        n: &SyntaxNode,
        field: usize,
        b: &mut Budget,
    ) -> Result<String, Error> {
        let n = self.node(child(n, id, field)?)?;
        b.charge(Resource::Work, n.kind.len() as u64)?;
        if n.kind != "Builtin:Text" || !n.fields.is_empty() {
            return Err(Error::Operand { node: id, field });
        }
        let token = n
            .token
            .and_then(|t| self.bundle.tokens.get(t.0 as usize))
            .ok_or(Error::Operand { node: id, field })?;
        let NdfValue::Text(text) = &token.payload else {
            return Err(Error::Operand { node: id, field });
        };
        b.charge(Resource::Work, text.len() as u64)?;
        b.charge(Resource::AllocationUnits, text.len() as u64)?;
        Ok(text.clone())
    }
    fn convert(&mut self, id: NodeRef, b: &mut Budget) -> Result<(), Error> {
        let n = self.node(id)?;
        b.charge(Resource::Work, n.kind.len() as u64 + 1)?;
        let kind = match (n.kind.as_str(), n.fields.len()) {
            ("Builtin:Text", 0) => return Ok(()),
            (name, 0 | 2) if name.starts_with("List:") => return Ok(()),
            ("Form:Sentence", 1) => Kind::Sentence {
                inlines: self.list(id, n, 0, b)?,
            },
            ("Form:Text", 1) => Kind::Text {
                text: self.text(id, n, 0, b)?,
            },
            ("Form:Concat", 1) => Kind::Concat {
                inlines: self.list(id, n, 0, b)?,
            },
            ("Form:Ruby", 2) => Kind::Ruby {
                base: self.inline(id, n, 0)?,
                reading: self.inline(id, n, 1)?,
            },
            ("Form:InlineAnno", 2) => Kind::InlineAnno {
                base: self.inline(id, n, 0)?,
                notes: self.list(id, n, 1, b)?,
            },
            ("Form:Code", 1) => Kind::Code {
                text: self.text(id, n, 0, b)?,
            },
            ("Form:Emphasis", 1) => Kind::Emphasis {
                inline: self.inline(id, n, 0)?,
            },
            ("Form:Strong", 1) => Kind::Strong {
                inline: self.inline(id, n, 0)?,
            },
            ("Form:Break", 0) => Kind::Break,
            ("Form:ExternalLink", 2) => Kind::ExternalLink {
                uri: self.text(id, n, 0, b)?,
                label: self.inline(id, n, 1)?,
            },
            _ => return Err(Error::Unsupported(id)),
        };
        let index = self.nodes.len() as u64;
        b.charge(Resource::Nodes, 1)?;
        push(&mut self.nodes, kind, b)?;
        self.mapping[id.0 as usize] = Some(index);
        Ok(())
    }
}
/// The host supplies its selected compiler-produced surface identity. Revalidate
/// with current resources, then lower every standard prefix constructor without
/// running readers or guests. Literal payloads require the separate decoder
/// integration and are explicitly rejected here, never reparsed or fabricated.
pub fn prefix(
    input: &ValidatedSyntaxBundle<'_>,
    surface: &SchemaRef,
    registry: &SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<Projection, Error> {
    prefix_with_foreign(input, surface, &[], registry, b, admission)
}

/// Lower explicitly selected foreign-inline forms together with the standard
/// constructors. Each selected form has exactly one foreign field. Unselected
/// forms, ambiguous selections and different guest identities are rejected.
pub fn prefix_with_foreign(
    input: &ValidatedSyntaxBundle<'_>,
    surface: &SchemaRef,
    foreign_forms: &[ForeignInlineForm<'_>],
    registry: &SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<Projection, Error> {
    let checked = input
        .bundle()
        .validate_with_sources(registry, b, admission)?;
    let bundle = checked.bundle();
    let mut captures = ForeignCapture::new(&checked);
    let count = bundle.nodes.len();
    b.charge(
        Resource::AllocationUnits,
        (count as u64).saturating_mul(core::mem::size_of::<Option<u64>>() as u64 + 1),
    )?;
    let mut done = vec![false; count];
    let mut a = Adapter {
        bundle,
        mapping: vec![None; count],
        nodes: Vec::new(),
    };
    let mut stack = Vec::new();
    let mut embeds = Vec::new();
    push(&mut stack, (bundle.root, 0usize), b)?;
    while let Some((id, field)) = stack.last_mut() {
        b.charge(Resource::Work, 1)?;
        let n = a.node(*id)?;
        if done[id.0 as usize] {
            stack.pop();
            continue;
        }
        b.charge(
            Resource::Work,
            (n.schema.package.len() + surface.package.len()) as u64 + 33,
        )?;
        if &n.schema != surface {
            return Err(Error::Unsupported(*id));
        }
        if let [FieldValue::Foreign(foreign)] = n.fields.as_slice() {
            b.charge(Resource::Work, n.kind.len() as u64 + 1)?;
            if !n.kind.starts_with("Form:")
                || matches!(
                    n.kind.as_str(),
                    "Form:Sentence"
                        | "Form:Text"
                        | "Form:Concat"
                        | "Form:Ruby"
                        | "Form:InlineAnno"
                        | "Form:Code"
                        | "Form:Emphasis"
                        | "Form:Strong"
                        | "Form:Break"
                        | "Form:ExternalLink"
                )
            {
                return Err(Error::Unsupported(*id));
            }
            let mut selected = None;
            for form in foreign_forms {
                b.charge(Resource::Work, (form.kind.len() + n.kind.len()) as u64 + 1)?;
                if form.kind == n.kind {
                    if selected.is_some() {
                        return Err(Error::Unsupported(*id));
                    }
                    selected = Some(form);
                }
            }
            let form = selected.ok_or(Error::Unsupported(*id))?;
            b.charge(
                Resource::Work,
                (form.guest_schema.package.len()
                    + foreign.schema.package.len()
                    + form.guest_category.len()
                    + foreign.category.len()) as u64
                    + 64,
            )?;
            if form.guest_schema != &foreign.schema || form.guest_category != foreign.category {
                return Err(Error::Operand {
                    node: *id,
                    field: 0,
                });
            }
            let closure = captures.capture_at(*id, 0, registry, b, admission)?;
            let embed = EmbedRef(embeds.len() as u64);
            push(&mut embeds, closure, b)?;
            let index = a.nodes.len() as u64;
            b.charge(Resource::Nodes, 1)?;
            push(&mut a.nodes, Kind::ForeignInline { syntax: embed }, b)?;
            a.mapping[id.0 as usize] = Some(index);
            done[id.0 as usize] = true;
            stack.pop();
            continue;
        }
        if let Some(value) = n.fields.get(*field) {
            let FieldValue::Child(c) = value else {
                return Err(Error::Operand {
                    node: *id,
                    field: *field,
                });
            };
            *field += 1;
            b.observe_depth(stack.len() as u64 + 1)?;
            push(&mut stack, (*c, 0), b)?;
        } else {
            let id = *id;
            a.convert(id, b)?;
            done[id.0 as usize] = true;
            stack.pop();
        }
    }
    let root = a
        .mapping
        .get(bundle.root.0 as usize)
        .copied()
        .flatten()
        .ok_or(Error::Unsupported(bundle.root))?;
    let root = if matches!(a.nodes[root as usize], Kind::Sentence { .. }) {
        Root::Sentence(SentenceRef(root))
    } else {
        Root::Inline(InlineRef(root))
    };
    let value = SentenceValue {
        root,
        nodes: a.nodes,
        embeds,
    };
    value.validate_shape(b)?;
    Ok(Projection {
        value,
        syntax_to_meaning: a.mapping,
    })
}
