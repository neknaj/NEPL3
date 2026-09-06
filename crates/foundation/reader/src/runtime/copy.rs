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
pub(crate) trait CopyCost {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason>;
}
pub(crate) fn copy<T: CopyCost + Clone>(value: &T, b: &mut Budget) -> Result<T, StopReason> {
    value.charge(b)?;
    Ok(value.clone())
}
pub(crate) fn slot<T>(b: &mut Budget) -> Result<(), StopReason> {
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
fixed!(u64, ViewRef, OriginId, TraceOverflow);
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
        slot::<Self>(b)?;
        self.identity().source.0.charge(b)?;
        b.charge(
            Resource::AllocationUnits,
            (self.text().len() + self.uri().len()) as u64,
        )?;
        b.charge(
            Resource::Work,
            (self.text().len() + self.uri().len()) as u64,
        )
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
impl CopyCost for CharClass {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Chars(s) | Self::Except(s) => s.charge(b),
            _ => Ok(()),
        }
    }
}
impl CopyCost for Expectation {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Literal(s) => s.charge(b),
            Self::ScalarClass(c) => c.charge(b),
            Self::Provider {
                operation,
                arguments,
            } => {
                operation.charge(b)?;
                arguments.charge(b)
            }
            _ => Ok(()),
        }
    }
}
impl CopyCost for ReaderFact {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Capture { name, span } => {
                name.charge(b)?;
                span.charge(b)
            }
            Self::Presentation { class, span } => {
                class.charge(b)?;
                span.charge(b)
            }
            Self::Relation {
                schema,
                kind,
                from,
                to,
            } => {
                schema.charge(b)?;
                kind.charge(b)?;
                from.charge(b)?;
                to.charge(b)
            }
        }
    }
}
impl CopyCost for TextEdit {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.span.charge(b)?;
        self.replacement.charge(b)
    }
}
impl CopyCost for Fix {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.id.charge(b)?;
        self.edits.charge(b)
    }
}
impl CopyCost for Related {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.span.charge(b)?;
        self.code.charge(b)?;
        self.arguments.charge(b)
    }
}
impl CopyCost for Diagnostic {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge(b)?;
        self.code.charge(b)?;
        self.stage.charge(b)?;
        self.arguments.charge(b)?;
        self.primary.charge(b)?;
        self.related.charge(b)?;
        self.fixes.charge(b)
    }
}
impl CopyCost for Event {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge(b)?;
        self.kind.charge(b)?;
        self.operation_path.charge(b)?;
        self.span.charge(b)?;
        self.payload.charge(b)
    }
}
impl CopyCost for Report {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.diagnostics.charge(b)?;
        self.events.charge(b)?;
        self.trace_overflow.charge(b)
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
impl CopyCost for ReaderContext {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.schema.charge(b)?;
        self.category.charge(b)?;
        self.mode.charge(b)?;
        self.environment.value.bindings.charge(b)?;
        self.environment.value.resources.charge(b)?;
        self.origins.charge(b)
    }
}
impl CopyCost for OwnedReadRequest {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.snapshot.source_id.0.charge(b)?;
        self.sources.charge(b)?;
        self.context.charge(b)?;
        self.state.charge(b)
    }
}
impl CopyCost for TransformRequest {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.value.charge(b)?;
        self.span.charge(b)?;
        self.view.charge(b)?;
        self.context.charge(b)
    }
}
impl CopyCost for DependentRequest {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.first.charge(b)?;
        self.request.charge(b)
    }
}
impl CopyCost for ProviderCall {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Read {
                session_id,
                operation,
                request,
                ..
            } => {
                session_id.charge(b)?;
                operation.charge(b)?;
                request.charge(b)
            }
            Self::Transform {
                session_id,
                operation,
                request,
                ..
            } => {
                session_id.charge(b)?;
                operation.charge(b)?;
                request.charge(b)
            }
            Self::Dependent {
                session_id,
                operation,
                request,
                ..
            } => {
                session_id.charge(b)?;
                operation.charge(b)?;
                request.charge(b)
            }
        }
    }
}
impl CopyCost for ReaderCheckpoint {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.state.charge(b)?;
        self.view.charge(b)?;
        self.facts.charge(b)?;
        self.diagnostics.charge(b)?;
        self.events.charge(b)?;
        self.trace_overflow.charge(b)?;
        self.sources.charge(b)?;
        self.source_maps.charge(b)
    }
}
impl CopyCost for FramePhase {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        match self {
            Self::Seq { values, .. } | Self::Repeat { values, .. } => values.charge(b),
            Self::Choice { expected, .. } => expected.charge(b),
            Self::Then { first, .. } => first.charge(b),
            _ => Ok(()),
        }
    }
}
impl CopyCost for ReaderFrame {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.checkpoint.charge(b)?;
        self.phase.charge(b)
    }
}
impl CopyCost for ReaderContinuation {
    fn charge(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        self.session_id.charge(b)?;
        self.plan_schema.charge(b)?;
        self.request.charge(b)?;
        self.frames.charge(b)?;
        self.current.charge(b)?;
        self.pending.charge(b)?;
        self.report.charge(b)
    }
}
