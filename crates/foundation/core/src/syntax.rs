//! Shared syntax graph contracts. Validation proves references and source geometry,
//! not language meaning or correspondence between form fields and domain schemas.
use crate::view::{Token, ViewError};
use crate::{
    budget::{Budget, Resource, StopReason},
    origin::{Mapping, Origin, OriginError, OriginGraph, OriginId, SourceMap},
    schema::{SchemaError, SchemaRegistry},
    source::{Digest, SourceAdmission, SourceError, SourceSnapshot, SourceStore, Span},
    value::{NdfScalar, SchemaRef, TypedValue},
};
use alloc::{boxed::Box, string::String, vec::Vec};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeRef(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenRef(pub u64);
pub type OriginRef = OriginId;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceRef {
    pub schema: SchemaRef,
    pub name: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceContent {
    pub id: String,
    pub digest: Digest,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentBinding {
    pub namespace: NamespaceRef,
    pub name: String,
    pub value: TypedValue,
    pub origin: Option<OriginRef>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Environment {
    pub bindings: Vec<EnvironmentBinding>,
    pub resources: Vec<ResourceContent>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentEntry {
    pub id: u64,
    pub digest: Digest,
    pub value: Environment,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentRef {
    pub id: u64,
    pub digest: Digest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignSyntax {
    pub schema: SchemaRef,
    pub category: String,
    pub root: NodeRef,
    pub bundle: SyntaxBundle,
    pub environment: EnvironmentRef,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldValue {
    Atom(NdfScalar),
    Child(NodeRef),
    Children(Vec<NodeRef>),
    Foreign(Box<ForeignSyntax>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxNode {
    pub schema: SchemaRef,
    pub kind: String,
    pub fields: Vec<FieldValue>,
    pub head: Option<Span>,
    pub cover: Option<Span>,
    pub origin: OriginRef,
    pub token: Option<TokenRef>,
}
pub struct SyntaxBundle {
    pub sources: Vec<SourceSnapshot>,
    pub nodes: Vec<SyntaxNode>,
    pub origins: Vec<Origin>,
    pub root: NodeRef,
    pub environments: Vec<EnvironmentEntry>,
    pub tokens: Vec<Token>,
    pub source_maps: Vec<Mapping>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxError {
    Stopped(StopReason),
    Source(SourceError),
    Origin(OriginError),
    Schema(SchemaError),
    Reference,
    Cycle,
    Cover,
    ChildOrder,
    DuplicateSource,
    DuplicateEnvironment,
    Environment,
    ForeignRoot,
    ResourceDigest,
    View(ViewError),
}
impl From<ViewError> for SyntaxError {
    fn from(e: ViewError) -> Self {
        Self::View(e)
    }
}
impl From<StopReason> for SyntaxError {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
impl From<SourceError> for SyntaxError {
    fn from(e: SourceError) -> Self {
        Self::Source(e)
    }
}
impl From<OriginError> for SyntaxError {
    fn from(e: OriginError) -> Self {
        Self::Origin(e)
    }
}
impl From<SchemaError> for SyntaxError {
    fn from(e: SchemaError) -> Self {
        Self::Schema(e)
    }
}
#[derive(Debug)]
pub struct ValidatedSyntaxBundle<'a> {
    bundle: &'a SyntaxBundle,
}
impl<'a> ValidatedSyntaxBundle<'a> {
    pub fn bundle(&self) -> &'a SyntaxBundle {
        self.bundle
    }
}

impl SyntaxBundle {
    /// Environment digests identify the selected entry here. Wire recomputes their canonical
    /// NDF digest; this graph proof does not certify environment hashes or domain validity.
    pub fn validate<'a>(
        &'a self,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<ValidatedSyntaxBundle<'a>, SyntaxError> {
        self.validate_with_sources(registry, budget, &mut SourceAdmission::default())
    }
    pub fn validate_with_sources<'a>(
        &'a self,
        registry: &SchemaRegistry,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ValidatedSyntaxBundle<'a>, SyntaxError> {
        if !registry.is_finalized() {
            return Err(SchemaError::Unfinalized.into());
        }
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<(&Self, u64)>() as u64,
        )?;
        let mut bundles = alloc::vec![(self, 1u64)];
        while let Some((bundle, depth)) = bundles.pop() {
            budget.observe_depth(depth)?;
            bundle.node(bundle.root)?;
            let mut sources = SourceStore::default();
            for (index, source) in bundle.sources.iter().enumerate() {
                admission.admit_existing(source, budget)?;
                budget.charge(Resource::Work, index as u64 + 1)?;
                if bundle.sources[..index]
                    .iter()
                    .any(|prior| prior.identity() == source.identity())
                {
                    return Err(SyntaxError::DuplicateSource);
                }
                budget.charge(
                    Resource::AllocationUnits,
                    (source.text().len() + source.identity().source.0.len() + source.uri().len())
                        as u64
                        + core::mem::size_of::<SourceSnapshot>() as u64,
                )?;
                sources.insert(source.clone())?;
            }
            budget.charge(
                Resource::AllocationUnits,
                (bundle.origins.len() as u64).saturating_mul(core::mem::size_of::<Origin>() as u64),
            )?;
            OriginGraph::validate_origins(&bundle.origins, &sources, budget)?;
            for (index, environment) in bundle.environments.iter().enumerate() {
                if bundle.environments[..index]
                    .iter()
                    .any(|e| e.id == environment.id)
                {
                    return Err(SyntaxError::DuplicateEnvironment);
                }
                for (index, binding) in environment.value.bindings.iter().enumerate() {
                    budget.charge(Resource::Work, 1)?;
                    require_schema(registry, &binding.namespace.schema)?;
                    if binding.name.is_empty()
                        || binding.namespace.name.is_empty()
                        || environment.value.bindings[..index]
                            .iter()
                            .any(|b| b.namespace == binding.namespace && b.name == binding.name)
                    {
                        return Err(SyntaxError::Environment);
                    }
                    if let Some(origin) = binding.origin {
                        check_origin(bundle, origin)?;
                    }
                    registry.validate_typed(&binding.value, budget)?;
                }
                for (index, resource) in environment.value.resources.iter().enumerate() {
                    budget.charge(Resource::Work, resource.bytes.len() as u64)?;
                    if resource.id.is_empty()
                        || environment.value.resources[..index]
                            .iter()
                            .any(|r| r.id == resource.id)
                        || resource.digest != Digest::of(&resource.bytes)
                    {
                        return Err(SyntaxError::ResourceDigest);
                    }
                }
            }
            let maps = SourceMap::validate_mappings(&bundle.source_maps, &sources, budget)?;
            for token in &bundle.tokens {
                token.validate_with_maps(&sources, registry, &maps, budget)?;
            }
            let node_depths = bundle.check_cycles(depth, budget)?;
            for (node_index, node) in bundle.nodes.iter().enumerate() {
                budget.charge(Resource::Nodes, 1)?;
                let descriptor = registry
                    .descriptor(&node.schema)
                    .ok_or(SchemaError::UnknownSchema)?;
                if !descriptor.types.iter().any(|ty| ty.name == node.kind) {
                    return Err(SchemaError::UnknownType.into());
                }
                check_origin(bundle, node.origin)?;
                if let Some(reference) = node.token {
                    let token = usize::try_from(reference.0)
                        .ok()
                        .and_then(|i| bundle.tokens.get(i))
                        .ok_or(SyntaxError::Reference)?;
                    if node.head.as_ref().is_some_and(|head| head != &token.head) {
                        return Err(SyntaxError::Cover);
                    }
                }
                for span in [node.head.as_ref(), node.cover.as_ref()]
                    .into_iter()
                    .flatten()
                {
                    sources
                        .get_ref(span.snapshot_ref())
                        .ok_or(SourceError::MissingSnapshot)?
                        .slice(span)?;
                }
                if let Some(head) = &node.head
                    && !node
                        .cover
                        .as_ref()
                        .is_some_and(|cover| cover.contains(head))
                {
                    return Err(SyntaxError::Cover);
                }
                let mut last: Option<&Span> = None;
                for field in &node.fields {
                    budget.charge(Resource::Work, 1)?;
                    match field {
                        FieldValue::Child(child) => check_child(bundle, node, *child, &mut last)?,
                        FieldValue::Children(children) => {
                            for child in children {
                                budget.charge(Resource::Work, 1)?;
                                check_child(bundle, node, *child, &mut last)?;
                            }
                        }
                        FieldValue::Foreign(foreign) => {
                            if foreign.root != foreign.bundle.root
                                || foreign.bundle.node(foreign.root)?.schema != foreign.schema
                                || foreign.category.is_empty()
                            {
                                return Err(SyntaxError::ForeignRoot);
                            }
                            if !bundle.environments.iter().any(|e| {
                                e.id == foreign.environment.id
                                    && e.digest == foreign.environment.digest
                            }) {
                                return Err(SyntaxError::Environment);
                            }
                            budget.charge(
                                Resource::AllocationUnits,
                                core::mem::size_of::<(&Self, u64)>() as u64,
                            )?;
                            bundles.push((
                                &foreign.bundle,
                                node_depths[node_index]
                                    .checked_add(1)
                                    .ok_or(StopReason::DepthLimit)?,
                            ));
                        }
                        FieldValue::Atom(_) => {}
                    }
                }
            }
        }
        Ok(ValidatedSyntaxBundle { bundle: self })
    }
    pub fn node(&self, reference: NodeRef) -> Result<&SyntaxNode, SyntaxError> {
        usize::try_from(reference.0)
            .ok()
            .and_then(|index| self.nodes.get(index))
            .ok_or(SyntaxError::Reference)
    }
    fn check_cycles(&self, base: u64, budget: &mut Budget) -> Result<Vec<u64>, SyntaxError> {
        budget.charge(
            Resource::AllocationUnits,
            (self.nodes.len() as u64).saturating_mul(9),
        )?;
        let mut state = alloc::vec![0u8; self.nodes.len()];
        let mut heights = alloc::vec![0u64; self.nodes.len()];
        let mut order = Vec::new();
        for root in 0..self.nodes.len() {
            let mut stack = Vec::new();
            push_node(&mut stack, root, false, base, budget)?;
            while let Some((index, exiting, depth)) = stack.pop() {
                budget.charge(Resource::Work, 1)?;
                budget.observe_depth(depth)?;
                if exiting {
                    let mut height = 1;
                    for child in children(&self.nodes[index]) {
                        height = height.max(
                            heights[child.0 as usize]
                                .checked_add(1)
                                .ok_or(StopReason::DepthLimit)?,
                        );
                    }
                    heights[index] = height;
                    state[index] = 2;
                    budget.charge(
                        Resource::AllocationUnits,
                        core::mem::size_of::<usize>() as u64,
                    )?;
                    order.push(index);
                    continue;
                }
                match state[index] {
                    1 => return Err(SyntaxError::Cycle),
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
                push_node(&mut stack, index, true, depth, budget)?;
                for child in children(&self.nodes[index]) {
                    self.node(child)?;
                    push_node(
                        &mut stack,
                        child.0 as usize,
                        false,
                        depth.checked_add(1).ok_or(StopReason::DepthLimit)?,
                        budget,
                    )?;
                }
            }
        }
        budget.charge(
            Resource::AllocationUnits,
            (self.nodes.len() as u64).saturating_mul(8),
        )?;
        let mut depths = alloc::vec![base; self.nodes.len()];
        for index in order.into_iter().rev() {
            budget.observe_depth(depths[index])?;
            for child in children(&self.nodes[index]) {
                let child = child.0 as usize;
                depths[child] =
                    depths[child].max(depths[index].checked_add(1).ok_or(StopReason::DepthLimit)?);
            }
        }
        Ok(depths)
    }
}
fn children(node: &SyntaxNode) -> impl Iterator<Item = NodeRef> + '_ {
    node.fields
        .iter()
        .flat_map(|field| match field {
            FieldValue::Child(child) => core::slice::from_ref(child).iter(),
            FieldValue::Children(children) => children.iter(),
            _ => [].iter(),
        })
        .copied()
}
fn push_node(
    stack: &mut Vec<(usize, bool, u64)>,
    index: usize,
    exiting: bool,
    depth: u64,
    budget: &mut Budget,
) -> Result<(), SyntaxError> {
    budget.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<(usize, bool, u64)>() as u64,
    )?;
    stack.push((index, exiting, depth));
    Ok(())
}
fn check_origin(bundle: &SyntaxBundle, origin: OriginRef) -> Result<(), SyntaxError> {
    if origin.0 >= bundle.origins.len() as u64 {
        Err(SyntaxError::Reference)
    } else {
        Ok(())
    }
}
fn require_schema(registry: &SchemaRegistry, schema: &SchemaRef) -> Result<(), SyntaxError> {
    if registry.descriptor(schema).is_some() {
        Ok(())
    } else {
        Err(SchemaError::UnknownSchema.into())
    }
}
fn check_child<'a>(
    bundle: &'a SyntaxBundle,
    parent: &SyntaxNode,
    child: NodeRef,
    last: &mut Option<&'a Span>,
) -> Result<(), SyntaxError> {
    let child = bundle.node(child)?;
    if let Some(cover) = &parent.cover {
        let child_cover = child.cover.as_ref().ok_or(SyntaxError::Cover)?;
        if !cover.contains(child_cover) {
            return Err(SyntaxError::Cover);
        }
        if parent
            .head
            .as_ref()
            .is_some_and(|head| head.end() > child_cover.start())
        {
            return Err(SyntaxError::ChildOrder);
        }
        if last.is_some_and(|last| last.end() > child_cover.start()) {
            return Err(SyntaxError::ChildOrder);
        }
        *last = Some(child_cover);
    }
    Ok(())
}

impl SyntaxBundle {
    fn detach_guests(&mut self, pending: &mut Vec<Self>) {
        for node in &mut self.nodes {
            for field in &mut node.fields {
                if let FieldValue::Foreign(foreign) = field {
                    let empty = Self {
                        sources: Vec::new(),
                        nodes: Vec::new(),
                        origins: Vec::new(),
                        root: NodeRef(0),
                        environments: Vec::new(),
                        tokens: Vec::new(),
                        source_maps: Vec::new(),
                    };
                    pending.push(core::mem::replace(&mut foreign.bundle, empty));
                }
            }
        }
    }
}
impl Drop for SyntaxBundle {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        self.detach_guests(&mut pending);
        while let Some(mut bundle) = pending.pop() {
            bundle.detach_guests(&mut pending);
        }
    }
}

impl Clone for SyntaxBundle {
    fn clone(&self) -> Self {
        fn empty() -> SyntaxBundle {
            SyntaxBundle {
                sources: Vec::new(),
                nodes: Vec::new(),
                origins: Vec::new(),
                root: NodeRef(0),
                environments: Vec::new(),
                tokens: Vec::new(),
                source_maps: Vec::new(),
            }
        }
        fn shallow(source: &SyntaxBundle) -> SyntaxBundle {
            SyntaxBundle {
                sources: source.sources.clone(),
                origins: source.origins.clone(),
                root: source.root,
                environments: source.environments.clone(),
                tokens: source.tokens.clone(),
                source_maps: source.source_maps.clone(),
                nodes: source
                    .nodes
                    .iter()
                    .map(|node| SyntaxNode {
                        schema: node.schema.clone(),
                        kind: node.kind.clone(),
                        head: node.head.clone(),
                        cover: node.cover.clone(),
                        origin: node.origin,
                        token: node.token,
                        fields: node
                            .fields
                            .iter()
                            .map(|field| match field {
                                FieldValue::Atom(v) => FieldValue::Atom(v.clone()),
                                FieldValue::Child(v) => FieldValue::Child(*v),
                                FieldValue::Children(v) => FieldValue::Children(v.clone()),
                                FieldValue::Foreign(v) => {
                                    FieldValue::Foreign(Box::new(ForeignSyntax {
                                        schema: v.schema.clone(),
                                        category: v.category.clone(),
                                        root: v.root,
                                        environment: v.environment.clone(),
                                        bundle: empty(),
                                    }))
                                }
                            })
                            .collect(),
                    })
                    .collect(),
            }
        }
        let mut result = empty();
        let mut pending = alloc::vec![(self, &mut result)];
        while let Some((source, target)) = pending.pop() {
            *target = shallow(source);
            for (source, target) in source.nodes.iter().zip(&mut target.nodes) {
                for (source, target) in source.fields.iter().zip(&mut target.fields) {
                    if let (FieldValue::Foreign(source), FieldValue::Foreign(target)) =
                        (source, target)
                    {
                        pending.push((&source.bundle, &mut target.bundle));
                    }
                }
            }
        }
        result
    }
}
impl PartialEq for SyntaxBundle {
    fn eq(&self, other: &Self) -> bool {
        let mut pending = alloc::vec![(self, other)];
        while let Some((a, b)) = pending.pop() {
            if a.sources != b.sources
                || a.origins != b.origins
                || a.root != b.root
                || a.environments != b.environments
                || a.tokens != b.tokens
                || a.source_maps != b.source_maps
                || a.nodes.len() != b.nodes.len()
            {
                return false;
            }
            for (a, b) in a.nodes.iter().zip(&b.nodes) {
                if a.schema != b.schema
                    || a.kind != b.kind
                    || a.head != b.head
                    || a.cover != b.cover
                    || a.origin != b.origin
                    || a.token != b.token
                    || a.fields.len() != b.fields.len()
                {
                    return false;
                }
                for (a, b) in a.fields.iter().zip(&b.fields) {
                    match (a, b) {
                        (FieldValue::Atom(a), FieldValue::Atom(b)) if a == b => {}
                        (FieldValue::Child(a), FieldValue::Child(b)) if a == b => {}
                        (FieldValue::Children(a), FieldValue::Children(b)) if a == b => {}
                        (FieldValue::Foreign(a), FieldValue::Foreign(b))
                            if a.schema == b.schema
                                && a.category == b.category
                                && a.root == b.root
                                && a.environment == b.environment =>
                        {
                            pending.push((&a.bundle, &b.bundle))
                        }
                        _ => return false,
                    }
                }
            }
        }
        true
    }
}
impl Eq for SyntaxBundle {}
impl core::fmt::Debug for SyntaxBundle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SyntaxBundle")
            .field("root", &self.root)
            .field("nodes", &self.nodes.len())
            .field("sources", &self.sources.len())
            .field("origins", &self.origins.len())
            .field("source_maps", &self.source_maps.len())
            .field("environments", &self.environments.len())
            .finish()
    }
}
