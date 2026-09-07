//! Conservative logical clone accounting for the common owned syntax model.
//! No source admission or diagnostic/event counters are repeated by a storage copy.
use super::*;
use crate::{source::*, value::*, view::*};
trait CopyCost {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason>;
}
fn slot<T>(b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::Work, 1)?;
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)
}
impl<T: CopyCost> CopyCost for Vec<T> {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        for v in self {
            v.charge(b)?;
        }
        Ok(())
    }
}
impl<T: CopyCost> CopyCost for Option<T> {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        if let Some(v) = self {
            v.charge(b)?;
        }
        Ok(())
    }
}
impl CopyCost for String {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        b.charge(Resource::AllocationUnits, self.len() as u64)?;
        b.charge(Resource::Work, self.len() as u64)
    }
}
macro_rules! fixed {($($ty:ty),*)=>{$(impl CopyCost for $ty {fn charge(&self,b:&mut Budget)->Result<(),StopReason>{slot::<Self>(b)}})*};}
fixed!(u64, ViewRef, OriginId, NodeRef);
impl CopyCost for SchemaRef {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.package.charge(b)
    }
}

impl CopyCost for OperationRef {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge(b)?;
        self.name.charge(b)
    }
}

impl CopyCost for Span {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.snapshot_ref().source.0.charge(b)
    }
}

impl CopyCost for SourceSnapshot {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        self.charge_clone(b)
    }
}

impl CopyCost for NdfValue {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        self.charge_clone(b)
    }
}

impl CopyCost for TypedValue {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Record(v) => {
                v.schema.charge(b)?;
                v.kind.charge(b)?;
                v.fields.charge(b)
            }
            Self::Variant(v) => {
                v.schema.charge(b)?;
                v.type_name.charge(b)?;
                v.variant.charge(b)?;
                v.fields.charge(b)
            }
        }
    }
}

impl CopyCost for KindRef {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge(b)
    }
}

impl CopyCost for PresentationClass {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge(b)?;
        self.name.charge(b)
    }
}

impl CopyCost for ViewField {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.name.charge(b)?;
        self.children.charge(b)
    }
}

impl CopyCost for ViewRelation {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge(b)?;
        self.kind.charge(b)
    }
}

impl CopyCost for ViewElement {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.kind.charge(b)?;
        self.span.charge(b)?;
        self.fields.charge(b)?;
        self.roles.charge(b)?;
        self.relations.charge(b)
    }
}

impl CopyCost for ViewBundle {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.elements.charge(b)?;
        self.roots.charge(b)
    }
}

impl CopyCost for Mapping {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.source.charge(b)?;
        self.target.charge(b)
    }
}

impl CopyCost for Origin {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Direct(s) => s.charge(b),
            Self::Composite(ids) => ids.charge(b),
            Self::Generated {
                operation,
                callsite,
                inputs,
            } => {
                operation.charge(b)?;
                callsite.charge(b)?;
                inputs.charge(b)
            }
            Self::Synthetic { reason, anchor } => {
                reason.charge(b)?;
                anchor.charge(b)
            }
        }
    }
}

impl CopyCost for EnvironmentBinding {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.namespace.schema.charge(b)?;
        self.namespace.name.charge(b)?;
        self.name.charge(b)?;
        self.value.charge(b)?;
        self.origin.charge(b)
    }
}

impl CopyCost for ResourceContent {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.id.charge(b)?;
        b.charge(Resource::AllocationUnits, self.bytes.len() as u64)?;
        b.charge(Resource::Work, self.bytes.len() as u64)
    }
}

impl CopyCost for Token {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.kind.charge(b)?;
        self.head.charge(b)?;
        self.payload.charge(b)?;
        self.views.charge(b)?;
        for trivia in &self.leading_trivia {
            slot::<Trivia>(b)?;
            trivia.span.charge(b)?;
        }
        Ok(())
    }
}
impl CopyCost for EnvironmentEntry {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.value.bindings.charge(b)?;
        self.value.resources.charge(b)
    }
}
impl CopyCost for NdfScalar {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        let bytes = match self {
            Self::Text(v) => v.len() as u64,
            Self::Bytes(v) => v.len() as u64,
            Self::Integer(v) => v.as_bigint().bits().div_ceil(8),
            Self::Rational(v) => {
                v.numerator().as_bigint().bits().div_ceil(8) + v.denominator().bits().div_ceil(8)
            }
            _ => 0,
        };
        b.charge(Resource::Work, bytes)?;
        b.charge(Resource::AllocationUnits, bytes)
    }
}

enum Pending<'a> {
    Bundle(&'a SyntaxBundle),
    Field(&'a FieldValue),
}
fn push<'a>(
    pending: &mut Vec<(Pending<'a>, u64)>,
    item: Pending<'a>,
    depth: u64,
    b: &mut Budget,
) -> Result<(), StopReason> {
    // Account both this borrowed audit worklist and the later iterative Clone worklist.
    b.charge(
        Resource::AllocationUnits,
        2 * core::mem::size_of::<(Pending<'a>, u64)>() as u64,
    )?;
    pending.push((item, depth));
    Ok(())
}
fn charge(root: Pending<'_>, b: &mut Budget) -> Result<(), StopReason> {
    let base = b.current_depth();
    let mut pending = Vec::new();
    push(&mut pending, root, 1, b)?;
    while let Some((item, depth)) = pending.pop() {
        b.observe_depth(depth)?;
        b.with_depth_at_least(
            base.checked_add(depth - 1).ok_or(StopReason::DepthLimit)?,
            |b| {
                match item {
                    Pending::Field(field) => {
                        slot::<FieldValue>(b)?;
                        match field {
                            FieldValue::Atom(v) => v.charge(b)?,
                            FieldValue::Child(_) => {}
                            FieldValue::Children(v) => v.charge(b)?,
                            FieldValue::Foreign(v) => {
                                slot::<ForeignSyntax>(b)?;
                                v.schema.charge(b)?;
                                v.category.charge(b)?;
                                push(
                                    &mut pending,
                                    Pending::Bundle(&v.bundle),
                                    depth.checked_add(1).ok_or(StopReason::DepthLimit)?,
                                    b,
                                )?;
                            }
                        }
                    }
                    Pending::Bundle(bundle) => {
                        slot::<SyntaxBundle>(b)?;
                        bundle.sources.charge(b)?;
                        bundle.origins.charge(b)?;
                        bundle.environments.charge(b)?;
                        bundle.tokens.charge(b)?;
                        bundle.source_maps.charge(b)?;
                        for node in &bundle.nodes {
                            slot::<SyntaxNode>(b)?;
                            node.schema.charge(b)?;
                            node.kind.charge(b)?;
                            node.head.charge(b)?;
                            node.cover.charge(b)?;
                            for field in &node.fields {
                                push(&mut pending, Pending::Field(field), depth, b)?;
                            }
                        }
                    }
                }
                Ok::<(), StopReason>(())
            },
        )?;
    }
    Ok(())
}
impl SyntaxBundle {
    /// Precharge storage/work for `Clone`, including foreign bundles and all owned
    /// source, token, environment and value payloads. This is not semantic validation.
    /// Foreign ownership depth is observed iteratively; source admission is unchanged.
    pub fn charge_clone(&self, budget: &mut Budget) -> Result<(), StopReason> {
        charge(Pending::Bundle(self), budget)
    }
    pub fn clone_with_budget(&self, budget: &mut Budget) -> Result<Self, StopReason> {
        self.charge_clone(budget)?;
        Ok(self.clone())
    }
}
impl FieldValue {
    pub fn charge_clone(&self, budget: &mut Budget) -> Result<(), StopReason> {
        charge(Pending::Field(self), budget)
    }
    pub fn clone_with_budget(&self, budget: &mut Budget) -> Result<Self, StopReason> {
        self.charge_clone(budget)?;
        Ok(self.clone())
    }
}

// Partial parse arenas use these parts before they have a root. The same storage
// accounting applies without constructing an invalid temporary SyntaxBundle.
macro_rules! owned_part {
    ($($ty:ty),* $(,)?) => {$(
        impl $ty {
            pub fn charge_clone(&self, budget: &mut Budget) -> Result<(), StopReason> {
                self.charge(budget)
            }
            pub fn clone_with_budget(&self, budget: &mut Budget) -> Result<Self, StopReason> {
                self.charge_clone(budget)?;
                Ok(self.clone())
            }
        }
    )*};
}
impl CopyCost for ForeignClosure {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.syntax.schema.charge(b)?;
        self.syntax.category.charge(b)?;
        self.syntax.bundle.charge_clone(b)?;
        self.owner_environment.charge(b)?;
        self.owner_origins.charge(b)?;
        self.owner_sources.charge(b)?;
        self.owner_source_maps.charge(b)
    }
}
owned_part!(
    Token,
    Origin,
    EnvironmentEntry,
    Mapping,
    ViewBundle,
    ForeignClosure
);
