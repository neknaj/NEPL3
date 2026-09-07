//! Conservative logical storage accounting for owned rollback and suspension copies.
use crate::{model::*, plan::CharClass};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::*,
    origin::*,
    source::*,
    syntax::*,
    value::*,
    view::*,
};
#[derive(Clone, Copy)]
pub(crate) enum CopyPurpose {
    Clone,
    Compare,
}
pub(crate) trait CopyCost {
    fn charge_for(&self, b: &mut Budget, _purpose: CopyPurpose) -> Result<(), StopReason>;
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        self.charge_for(b, CopyPurpose::Compare)
    }
    fn charge_copy(&self, b: &mut Budget) -> Result<(), StopReason> {
        self.charge_for(b, CopyPurpose::Clone)
    }
}
pub(crate) fn copy<T: CopyCost + Clone>(value: &T, b: &mut Budget) -> Result<T, StopReason> {
    value.charge_copy(b)?;
    Ok(value.clone())
}
pub(crate) fn slot<T>(b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::Work, 1)?;
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)
}
impl<T: CopyCost> CopyCost for Vec<T> {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        for v in self {
            v.charge_for(b, purpose)?;
        }
        Ok(())
    }
}
impl<T: CopyCost> CopyCost for Option<T> {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        if let Some(v) = self {
            v.charge_for(b, purpose)?;
        }
        Ok(())
    }
}
impl CopyCost for String {
    fn charge_for(&self, b: &mut Budget, _purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        b.charge(Resource::AllocationUnits, self.len() as u64)?;
        b.charge(Resource::Work, self.len() as u64)
    }
}
macro_rules! fixed {($($ty:ty),*)=>{$(impl CopyCost for $ty {fn charge_for(&self,b:&mut Budget,_purpose:CopyPurpose)->Result<(),StopReason>{slot::<Self>(b)}})*};}
fixed!(u64, ViewRef, OriginId, TraceOverflow);
impl CopyCost for SchemaRef {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.package.charge_for(b, purpose)
    }
}
impl CopyCost for OperationRef {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge_for(b, purpose)?;
        self.name.charge_for(b, purpose)
    }
}
impl CopyCost for Span {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.snapshot_ref().source.0.charge_for(b, purpose)
    }
}
impl CopyCost for SourceSnapshot {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        match purpose {
            CopyPurpose::Clone => self.charge_shared_clone(b),
            CopyPurpose::Compare => self.charge_clone(b),
        }
    }
}

impl CopyCost for NdfValue {
    fn charge_for(&self, b: &mut Budget, _purpose: CopyPurpose) -> Result<(), StopReason> {
        self.charge_clone(b)
    }
}
impl CopyCost for TypedValue {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Record(v) => {
                v.schema.charge_for(b, purpose)?;
                v.kind.charge_for(b, purpose)?;
                v.fields.charge_for(b, purpose)
            }
            Self::Variant(v) => {
                v.schema.charge_for(b, purpose)?;
                v.type_name.charge_for(b, purpose)?;
                v.variant.charge_for(b, purpose)?;
                v.fields.charge_for(b, purpose)
            }
        }
    }
}
impl CopyCost for KindRef {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge_for(b, purpose)
    }
}
impl CopyCost for PresentationClass {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge_for(b, purpose)?;
        self.name.charge_for(b, purpose)
    }
}
impl CopyCost for ViewField {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.name.charge_for(b, purpose)?;
        self.children.charge_for(b, purpose)
    }
}
impl CopyCost for ViewRelation {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge_for(b, purpose)?;
        self.kind.charge_for(b, purpose)
    }
}
impl CopyCost for ViewElement {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.kind.charge_for(b, purpose)?;
        self.span.charge_for(b, purpose)?;
        self.fields.charge_for(b, purpose)?;
        self.roles.charge_for(b, purpose)?;
        self.relations.charge_for(b, purpose)
    }
}
impl CopyCost for ViewBundle {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.elements.charge_for(b, purpose)?;
        self.roots.charge_for(b, purpose)
    }
}
impl CopyCost for CharClass {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Chars(s) | Self::Except(s) => s.charge_for(b, purpose),
            _ => Ok(()),
        }
    }
}
impl CopyCost for Expectation {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Literal(s) => s.charge_for(b, purpose),
            Self::ScalarClass(c) => c.charge_for(b, purpose),
            Self::Provider {
                operation,
                arguments,
            } => {
                operation.charge_for(b, purpose)?;
                arguments.charge_for(b, purpose)
            }
            _ => Ok(()),
        }
    }
}
impl CopyCost for ReaderFact {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Capture { name, span } => {
                name.charge_for(b, purpose)?;
                span.charge_for(b, purpose)
            }
            Self::Presentation { class, span } => {
                class.charge_for(b, purpose)?;
                span.charge_for(b, purpose)
            }
            Self::Relation {
                schema,
                kind,
                from,
                to,
            } => {
                schema.charge_for(b, purpose)?;
                kind.charge_for(b, purpose)?;
                from.charge_for(b, purpose)?;
                to.charge_for(b, purpose)
            }
        }
    }
}
impl CopyCost for TextEdit {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.span.charge_for(b, purpose)?;
        self.replacement.charge_for(b, purpose)
    }
}
impl CopyCost for Fix {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.id.charge_for(b, purpose)?;
        self.edits.charge_for(b, purpose)
    }
}
impl CopyCost for Related {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.span.charge_for(b, purpose)?;
        self.code.charge_for(b, purpose)?;
        self.arguments.charge_for(b, purpose)
    }
}
impl CopyCost for Diagnostic {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge_for(b, purpose)?;
        self.code.charge_for(b, purpose)?;
        self.stage.charge_for(b, purpose)?;
        self.arguments.charge_for(b, purpose)?;
        self.primary.charge_for(b, purpose)?;
        self.related.charge_for(b, purpose)?;
        self.fixes.charge_for(b, purpose)
    }
}
impl CopyCost for Event {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge_for(b, purpose)?;
        self.kind.charge_for(b, purpose)?;
        self.operation_path.charge_for(b, purpose)?;
        self.span.charge_for(b, purpose)?;
        self.payload.charge_for(b, purpose)
    }
}
impl CopyCost for Report {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.diagnostics.charge_for(b, purpose)?;
        self.events.charge_for(b, purpose)?;
        self.trace_overflow.charge_for(b, purpose)
    }
}
impl CopyCost for Mapping {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.source.charge_for(b, purpose)?;
        self.target.charge_for(b, purpose)
    }
}
impl CopyCost for Origin {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Direct(s) => s.charge_for(b, purpose),
            Self::Composite(ids) => ids.charge_for(b, purpose),
            Self::Generated {
                operation,
                callsite,
                inputs,
            } => {
                operation.charge_for(b, purpose)?;
                callsite.charge_for(b, purpose)?;
                inputs.charge_for(b, purpose)
            }
            Self::Synthetic { reason, anchor } => {
                reason.charge_for(b, purpose)?;
                anchor.charge_for(b, purpose)
            }
        }
    }
}
impl CopyCost for EnvironmentBinding {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.namespace.schema.charge_for(b, purpose)?;
        self.namespace.name.charge_for(b, purpose)?;
        self.name.charge_for(b, purpose)?;
        self.value.charge_for(b, purpose)?;
        self.origin.charge_for(b, purpose)
    }
}
impl CopyCost for ResourceContent {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.id.charge_for(b, purpose)?;
        b.charge(Resource::AllocationUnits, self.bytes.len() as u64)?;
        b.charge(Resource::Work, self.bytes.len() as u64)
    }
}
impl CopyCost for ReaderContext {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge_for(b, purpose)?;
        self.category.charge_for(b, purpose)?;
        self.mode.charge_for(b, purpose)?;
        self.environment.value.bindings.charge_for(b, purpose)?;
        self.environment.value.resources.charge_for(b, purpose)?;
        self.origins.charge_for(b, purpose)
    }
}
impl CopyCost for OwnedReadRequest {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.snapshot.source_id.0.charge_for(b, purpose)?;
        self.sources.charge_for(b, purpose)?;
        self.context.charge_for(b, purpose)?;
        self.state.charge_for(b, purpose)
    }
}
impl CopyCost for TransformRequest {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.value.charge_for(b, purpose)?;
        self.span.charge_for(b, purpose)?;
        self.view.charge_for(b, purpose)?;
        self.context.charge_for(b, purpose)
    }
}
impl CopyCost for DependentRequest {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.first.charge_for(b, purpose)?;
        self.request.charge_for(b, purpose)
    }
}
impl CopyCost for ProviderCall {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Read {
                session_id,
                operation,
                request,
                ..
            } => {
                session_id.charge_for(b, purpose)?;
                operation.charge_for(b, purpose)?;
                request.charge_for(b, purpose)
            }
            Self::Transform {
                session_id,
                operation,
                request,
                ..
            } => {
                session_id.charge_for(b, purpose)?;
                operation.charge_for(b, purpose)?;
                request.charge_for(b, purpose)
            }
            Self::Dependent {
                session_id,
                operation,
                request,
                ..
            } => {
                session_id.charge_for(b, purpose)?;
                operation.charge_for(b, purpose)?;
                request.charge_for(b, purpose)
            }
        }
    }
}
impl CopyCost for ReaderCheckpoint {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.state.charge_for(b, purpose)?;
        self.view.charge_for(b, purpose)?;
        self.facts.charge_for(b, purpose)?;
        self.diagnostics.charge_for(b, purpose)?;
        self.events.charge_for(b, purpose)?;
        self.trace_overflow.charge_for(b, purpose)?;
        self.sources.charge_for(b, purpose)?;
        self.source_maps.charge_for(b, purpose)
    }
}
impl CopyCost for FramePhase {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Seq { values, .. } | Self::Repeat { values, .. } => values.charge_for(b, purpose),
            Self::Choice { expected, .. } => expected.charge_for(b, purpose),
            Self::Then { first, .. } => first.charge_for(b, purpose),
            _ => Ok(()),
        }
    }
}
impl CopyCost for ReaderFrame {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.checkpoint.charge_for(b, purpose)?;
        self.phase.charge_for(b, purpose)
    }
}
impl CopyCost for ReaderContinuation {
    fn charge_for(&self, b: &mut Budget, purpose: CopyPurpose) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.session_id.charge_for(b, purpose)?;
        self.plan_schema.charge_for(b, purpose)?;
        self.request.charge_for(b, purpose)?;
        self.frames.charge_for(b, purpose)?;
        self.current.charge_for(b, purpose)?;
        self.pending.charge_for(b, purpose)?;
        self.report.charge_for(b, purpose)
    }
}

impl ReaderContinuation {
    /// Charge the logical traversal/storage required to clone this owned value.
    pub fn charge_clone(&self, budget: &mut Budget) -> Result<(), StopReason> {
        self.charge(budget)
    }
    pub fn clone_with_budget(&self, budget: &mut Budget) -> Result<Self, StopReason> {
        self.charge_copy(budget)?;
        Ok(self.clone())
    }
}
