//! Reader-owned token views are sidecars, never implicit prefix-parser children.
use crate::{
    budget::{Budget, Resource, StopReason},
    origin::{OriginError, SourceMap, ValidatedSourceMap},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor},
    source::{SourceError, SourceStore, Span},
    value::{KindRef, NdfValue, SchemaRef},
};
use alloc::{string::String, vec::Vec};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewRef(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriviaRef(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FallbackRole {
    Content,
    Marker,
    Delimiter,
    Name,
    Quantity,
    Annotation,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationClass {
    pub schema: SchemaRef,
    pub name: String,
    pub fallback: FallbackRole,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewField {
    pub name: String,
    pub children: Vec<ViewRef>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewRelation {
    pub schema: SchemaRef,
    pub kind: String,
    pub target: ViewRef,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewElement {
    pub kind: KindRef,
    pub span: Span,
    pub fields: Vec<ViewField>,
    pub roles: Vec<PresentationClass>,
    pub relations: Vec<ViewRelation>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewBundle {
    pub elements: Vec<ViewElement>,
    pub roots: Vec<ViewRef>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriviaKind {
    Whitespace,
    Comment,
    Bom,
    /// An explicitly skipped lexeme with no whitespace/comment interpretation.
    Skipped,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trivia {
    pub span: Span,
    pub kind: TriviaKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: KindRef,
    pub head: Span,
    pub payload: NdfValue,
    pub views: ViewBundle,
    pub leading_trivia: Vec<Trivia>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewError {
    Stopped(StopReason),
    Source(SourceError),
    Origin(OriginError),
    Schema(SchemaError),
    Reference,
    Cycle,
    Cover,
    DuplicateField,
    Presentation,
    Trivia,
}
impl From<StopReason> for ViewError {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
impl From<OriginError> for ViewError {
    fn from(e: OriginError) -> Self {
        match e {
            OriginError::Stopped(reason) => Self::Stopped(reason),
            e => Self::Origin(e),
        }
    }
}
impl From<SourceError> for ViewError {
    fn from(e: SourceError) -> Self {
        Self::Source(e)
    }
}
impl From<SchemaError> for ViewError {
    fn from(e: SchemaError) -> Self {
        Self::Schema(e)
    }
}
impl ViewBundle {
    pub fn element(&self, reference: ViewRef) -> Result<&ViewElement, ViewError> {
        usize::try_from(reference.0)
            .ok()
            .and_then(|i| self.elements.get(i))
            .ok_or(ViewError::Reference)
    }
    pub fn validate(
        &self,
        sources: &SourceStore,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<(), ViewError> {
        let maps = SourceMap::validate_mappings(&[], sources, budget)?;
        self.validate_with_maps(sources, registry, &maps, budget)
    }
    pub fn validate_with_maps(
        &self,
        sources: &SourceStore,
        registry: &SchemaRegistry,
        maps: &ValidatedSourceMap<'_>,
        budget: &mut Budget,
    ) -> Result<(), ViewError> {
        maps.validate_sources(sources, budget)?;
        for root in &self.roots {
            self.element(*root)?;
        }
        for element in &self.elements {
            budget.charge(Resource::Nodes, 1)?;
            check_kind(&element.kind, registry)?;
            sources
                .get_ref(element.span.snapshot_ref())
                .ok_or(SourceError::MissingSnapshot)?
                .slice(&element.span)?;
            for (index, field) in element.fields.iter().enumerate() {
                budget.charge(Resource::Work, 1)?;
                if field.name.is_empty()
                    || element.fields[..index].iter().any(|f| f.name == field.name)
                {
                    return Err(ViewError::DuplicateField);
                }
                for child in &field.children {
                    if !maps.contains(&element.span, &self.element(*child)?.span, budget)? {
                        return Err(ViewError::Cover);
                    }
                }
            }
            for role in &element.roles {
                if role.name.is_empty() || registry.descriptor(&role.schema).is_none() {
                    return Err(ViewError::Presentation);
                }
            }
            for relation in &element.relations {
                self.element(relation.target)?;
                if relation.kind.is_empty() || registry.descriptor(&relation.schema).is_none() {
                    return Err(ViewError::Presentation);
                }
            }
        }
        budget.charge(Resource::AllocationUnits, self.elements.len() as u64 * 9)?;
        let mut state = alloc::vec![0u8; self.elements.len()];
        let mut heights = alloc::vec![0u64; self.elements.len()];
        for root in 0..self.elements.len() {
            let mut pending = Vec::new();
            push(&mut pending, root, false, 1, budget)?;
            while let Some((index, exiting, depth)) = pending.pop() {
                budget.charge(Resource::Work, 1)?;
                budget.observe_depth(depth)?;
                if exiting {
                    let mut height = 1;
                    for child in self.elements[index].fields.iter().flat_map(|f| &f.children) {
                        height = height.max(
                            heights[child.0 as usize]
                                .checked_add(1)
                                .ok_or(StopReason::DepthLimit)?,
                        );
                    }
                    heights[index] = height;
                    state[index] = 2;
                    continue;
                }
                match state[index] {
                    1 => return Err(ViewError::Cycle),
                    2 => {
                        budget.observe_depth(
                            depth
                                .checked_add(heights[index] - 1)
                                .ok_or(StopReason::DepthLimit)?,
                        )?;
                        continue;
                    }
                    _ => {}
                }
                state[index] = 1;
                push(&mut pending, index, true, depth, budget)?;
                for child in self.elements[index].fields.iter().flat_map(|f| &f.children) {
                    push(
                        &mut pending,
                        child.0 as usize,
                        false,
                        depth.checked_add(1).ok_or(StopReason::DepthLimit)?,
                        budget,
                    )?;
                }
            }
        }
        Ok(())
    }
}
impl Token {
    /// Requires a validated view table; checks token-owned references and conservative source spans.
    pub fn validate(
        &self,
        sources: &SourceStore,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<(), ViewError> {
        let maps = SourceMap::validate_mappings(&[], sources, budget)?;
        self.validate_with_maps(sources, registry, &maps, budget)
    }
    pub fn validate_with_maps(
        &self,
        sources: &SourceStore,
        registry: &SchemaRegistry,
        maps: &ValidatedSourceMap<'_>,
        budget: &mut Budget,
    ) -> Result<(), ViewError> {
        self.views
            .validate_with_maps(sources, registry, maps, budget)?;
        check_kind(&self.kind, registry)?;
        sources
            .get_ref(self.head.snapshot_ref())
            .ok_or(SourceError::MissingSnapshot)?
            .slice(&self.head)?;
        registry.validate(&TypeDescriptor::NdfValue, &self.payload, budget)?;
        for view in &self.views.elements {
            budget.charge(Resource::Work, 1)?;
            if !maps.contains(&self.head, &view.span, budget)? {
                return Err(ViewError::Cover);
            }
        }
        let mut last = 0;
        for trivia in &self.leading_trivia {
            budget.charge(Resource::Work, 1)?;
            let source = sources
                .get_ref(trivia.span.snapshot_ref())
                .ok_or(SourceError::MissingSnapshot)?;
            let text = source.slice(&trivia.span)?;
            if trivia.span.snapshot_ref() != self.head.snapshot_ref()
                || trivia.span.start() < last
                || trivia.span.end() > self.head.start()
            {
                return Err(ViewError::Trivia);
            }
            match trivia.kind {
                TriviaKind::Skipped if text.is_empty() => return Err(ViewError::Trivia),
                TriviaKind::Whitespace
                    if text.is_empty() || !text.chars().all(char::is_whitespace) =>
                {
                    return Err(ViewError::Trivia);
                }
                TriviaKind::Bom if trivia.span.start() != 0 || text != "\u{feff}" => {
                    return Err(ViewError::Trivia);
                }
                _ => {}
            }
            last = trivia.span.end();
        }
        Ok(())
    }
}
fn check_kind(kind: &KindRef, registry: &SchemaRegistry) -> Result<(), ViewError> {
    if !registry.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    let descriptor = registry
        .descriptor(&kind.schema)
        .ok_or(SchemaError::UnknownSchema)?;
    if kind.local_kind >= descriptor.types.len() as u64 {
        return Err(SchemaError::UnknownType.into());
    }
    Ok(())
}
fn push(
    stack: &mut Vec<(usize, bool, u64)>,
    index: usize,
    exiting: bool,
    depth: u64,
    budget: &mut Budget,
) -> Result<(), ViewError> {
    budget.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<(usize, bool, u64)>() as u64,
    )?;
    stack.push((index, exiting, depth));
    Ok(())
}
