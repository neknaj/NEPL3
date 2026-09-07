//! Lower the explicit Doc surface schema, without depending on parser execution
//! types. The host checks its selected parse/profile before passing this graph;
//! this boundary checks the graph again with the operation's current resources.
mod forms;
mod literal;
pub use literal::DocumentLowerError;
mod operands;
use crate::{
    check::{Category, ShapeError, StructureError},
    model::*,
};
use alloc::{vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::{SourceAdmission, Span},
    syntax::{FieldValue, ForeignClosure, NodeRef, SyntaxError, SyntaxNode, ValidatedSyntaxBundle},
    value::SchemaRef,
};

#[derive(Debug, Eq, PartialEq)]
pub enum LowerError {
    Stopped(StopReason),
    Syntax(SyntaxError),
    Structure(StructureError),
    Shape(ShapeError),
    /// A retained source node violates the selected Doc constructor contract.
    Operand {
        node: NodeRef,
        field: usize,
    },
    Unsupported {
        node: NodeRef,
    },
    NaturalRange {
        node: NodeRef,
    },
    AssetDigest {
        node: NodeRef,
    },
    LiteralPayload {
        node: NodeRef,
    },
}
impl From<StopReason> for LowerError {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
impl From<SyntaxError> for LowerError {
    fn from(e: SyntaxError) -> Self {
        match e.stop_reason() {
            Some(s) => Self::Stopped(s),
            None => Self::Syntax(e),
        }
    }
}
impl From<ShapeError> for LowerError {
    fn from(e: ShapeError) -> Self {
        match e {
            ShapeError::Stopped(s) => Self::Stopped(s),
            e => Self::Shape(e),
        }
    }
}
impl From<StructureError> for LowerError {
    fn from(e: StructureError) -> Self {
        match e {
            StructureError::Stopped(s) => Self::Stopped(s),
            e => Self::Structure(e),
        }
    }
}

#[derive(Clone, Copy)]
enum Mapped {
    Node(u64),
}
struct Adapter<'a, 'b> {
    checked: &'a ValidatedSyntaxBundle<'a>,
    registry: &'a SchemaRegistry,
    mapping: Vec<Option<Mapped>>,
    nodes: Vec<DocNode>,
    embeds: Vec<DocEmbed>,
    b: &'b mut Budget,
    presentations: Vec<literal::Presentation>,
    next_origin: u64,
}
pub(crate) fn span(value: &Span, b: &mut Budget) -> Result<Span, StopReason> {
    b.charge(Resource::Work, value.snapshot_ref().source.0.len() as u64)?;
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<Span>() as u64 + value.snapshot_ref().source.0.len() as u64,
    )?;
    Ok(value.clone())
}
fn push<T>(target: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    target.push(value);
    Ok(())
}
fn root(category: Category, node: u64) -> DocRoot {
    match category {
        Category::Article => DocRoot::Article(ArticleRef(node)),
        Category::Body => DocRoot::Body(BodyRef(node)),
        Category::Block => DocRoot::Block(BlockRef(node)),
        Category::Flow => DocRoot::Flow(FlowRef(node)),
        Category::Sentence => DocRoot::Sentence(SentenceRef(node)),
        Category::Inline => DocRoot::Inline(InlineRef(node)),
        Category::Variant => DocRoot::Variant(VariantRef(node)),
        Category::Row => DocRoot::Row(RowRef(node)),
        Category::ListItem => DocRoot::ListItem(ListItemRef(node)),
        Category::Alignment => DocRoot::Alignment(AlignmentRef(node)),
        Category::ListStyle => DocRoot::ListStyle(ListStyleRef(node)),
        Category::Check => DocRoot::Check(CheckRef(node)),
        Category::Target => DocRoot::Target(TargetRef(node)),
        Category::Asset => DocRoot::Asset(AssetValueRef(node)),
        Category::OptionalRow => DocRoot::OptionalRow(OptionalRowRef(node)),
        Category::OptionalSentence => DocRoot::OptionalSentence(OptionalSentenceRef(node)),
        Category::OptionalText => DocRoot::OptionalText(OptionalTextRef(node)),
    }
}
/// The explicit surface schema is the compiler-produced Doc syntax schema,
/// distinct from `nepl3.doc`'s semantic arena schema. Source and origin tables
/// are retained in their original ID spaces. No guest is semantically lowered.
/// SentenceLiteral leaves require the separate reader-payload adapter; this
/// prefix entry returns `Unsupported` for them, never a fabricated value.
pub fn prefix(
    input: &ValidatedSyntaxBundle<'_>,
    surface: &SchemaRef,
    category: Category,
    registry: &SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<DocumentSyntax, LowerError> {
    run(
        input,
        surface,
        category,
        registry,
        b,
        &mut literal::PrefixOnly(admission),
    )
}
/// Lower prefix constructors and accepted SentenceLiteral payloads together.
/// The codec's shared admission owns both the host graph and decoded payload
/// declarations. Payload failures retain their typed codec causes.
pub fn document<C: nepl3_core::value_codec::FoundationValueCodec>(
    input: &ValidatedSyntaxBundle<'_>,
    surface: &SchemaRef,
    category: Category,
    registry: &SchemaRegistry,
    b: &mut Budget,
    codec: &mut C,
) -> Result<DocumentSyntax, DocumentLowerError<C::Error>> {
    run(
        input,
        surface,
        category,
        registry,
        b,
        &mut literal::Portable(codec),
    )
}
fn run<D: literal::Decoder>(
    input: &ValidatedSyntaxBundle<'_>,
    surface: &SchemaRef,
    category: Category,
    registry: &SchemaRegistry,
    b: &mut Budget,
    decoder: &mut D,
) -> Result<DocumentSyntax, D::Error> {
    let checked = input
        .bundle()
        .validate_with_sources(registry, b, decoder.admission())?;
    let bundle = checked.bundle();
    let size = bundle.nodes.len();
    b.charge(
        Resource::AllocationUnits,
        (size as u64).saturating_mul((core::mem::size_of::<Option<Mapped>>() + 1) as u64),
    )?;
    let mut done = vec![false; size];
    let mut a = Adapter {
        checked: &checked,
        registry,
        mapping: vec![None; size],
        nodes: Vec::new(),
        embeds: Vec::new(),
        b,
        presentations: Vec::new(),
        next_origin: bundle.origins.len() as u64,
    };
    // Each frame retains the next field and child position, avoiding recursion
    // and repeated flattening of list/foreign source graphs.
    let mut stack = Vec::new();
    let mut order = Vec::new();
    push(&mut stack, (bundle.root, 0usize, 0usize), a.b)?;
    while let Some((id, field, item)) = stack.last_mut() {
        a.b.charge(Resource::Work, 1)?;
        let index = usize::try_from(id.0).map_err(|_| SyntaxError::Reference)?;
        let node = bundle.nodes.get(index).ok_or(SyntaxError::Reference)?;
        if done[index] {
            stack.pop();
            continue;
        }
        let next = match node.fields.get(*field) {
            Some(FieldValue::Child(child)) => {
                *field += 1;
                Some(*child)
            }
            Some(FieldValue::Children(children)) => {
                if let Some(child) = children.get(*item) {
                    *item += 1;
                    Some(*child)
                } else {
                    *field += 1;
                    *item = 0;
                    continue;
                }
            }
            Some(FieldValue::Atom(_) | FieldValue::Foreign(_)) => {
                *field += 1;
                continue;
            }
            None => None,
        };
        if let Some(child) = next {
            a.b.observe_depth(stack.len() as u64 + 1)?;
            push(&mut stack, (child, 0, 0), a.b)?;
            continue;
        }
        let id = *id;
        a.b.charge(
            Resource::Work,
            (node.schema.package.len() + surface.package.len() + node.kind.len()) as u64 + 33,
        )?;
        if &node.schema != surface {
            return Err(LowerError::Unsupported { node: id }.into());
        }
        push(&mut order, index, a.b)?;
        done[index] = true;
        stack.pop();
    }
    a.b.charge(Resource::AllocationUnits, (size as u64).saturating_mul(8))?;
    let mut depths = vec![0u64; size];
    depths[bundle.root.0 as usize] = 1;
    for index in order.iter().rev() {
        let next = depths[*index].saturating_add(1);
        for field in &bundle.nodes[*index].fields {
            let children = match field {
                FieldValue::Child(child) => core::slice::from_ref(child),
                FieldValue::Children(children) => children.as_slice(),
                _ => &[],
            };
            for child in children {
                a.b.charge(Resource::Work, 1)?;
                let d = depths
                    .get_mut(child.0 as usize)
                    .ok_or(SyntaxError::Reference)?;
                *d = (*d).max(next);
            }
        }
    }
    let caller = a.b.current_depth();
    for index in order {
        let depth = caller
            .checked_add(depths[index])
            .ok_or(StopReason::DepthLimit)?;
        let node = &bundle.nodes[index];
        if node.kind == "Leaf:SentenceLiteral" && node.fields.is_empty() {
            a.literal_at_depth(NodeRef(index as u64), node, depth, decoder)?;
        } else {
            a.convert_at_depth(NodeRef(index as u64), node, depth, decoder.admission())?;
        }
    }
    let Some(Mapped::Node(root_id)) = a.mapping.get(bundle.root.0 as usize).copied().flatten()
    else {
        return Err(LowerError::Unsupported { node: bundle.root }.into());
    };
    let mut output = DocumentSyntax {
        value: DocValue {
            root: root(category, root_id),
            nodes: a.nodes,
            embeds: a.embeds,
        },
        sources: Vec::new(),
        origins: Vec::new(),
        views: Vec::new(),
        source_maps: Vec::new(),
    };
    for source in &bundle.sources {
        let source = source.clone_with_budget(a.b)?;
        push(&mut output.sources, source, a.b)?;
    }
    for origin in &bundle.origins {
        let origin = origin.clone_with_budget(a.b)?;
        push(&mut output.origins, origin, a.b)?;
    }
    for token in &bundle.tokens {
        let view = DocView {
            head: span(&token.head, a.b)?,
            view: token.views.clone_with_budget(a.b)?,
        };
        push(&mut output.views, view, a.b)?;
    }
    for map in &bundle.source_maps {
        let map = map.clone_with_budget(a.b)?;
        push(&mut output.source_maps, map, a.b)?;
    }
    literal::append(a.presentations, &mut output, a.b)?;
    // Auxiliary constructor wrappers consumed into typed parent fields are not
    // semantic children. Compact only this freshly built, checked graph.
    output.value = crate::normalize::compact(output.value, a.b)?;
    output.value = crate::normalize::value(output.value, &mut output.origins, a.b)?;
    output.validate_structure(registry, a.b, decoder.admission())?;
    Ok(output)
}
