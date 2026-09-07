use super::*;
use crate::{
    package::{EntryContext, StyleSelector},
    selection::HeadShape,
};
use nepl3_core::{
    budget::{Budget, Resource},
    value::SchemaRef,
};

fn bytes(n: usize, b: &mut Budget) -> Result<(), HeadError> {
    b.charge(Resource::Work, n as u64 + 1)?;
    b.charge(Resource::AllocationUnits, n as u64)?;
    Ok(())
}
fn slot<T>(b: &mut Budget) -> Result<(), HeadError> {
    b.charge(Resource::Work, 1)?;
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    Ok(())
}
fn schema(v: &SchemaRef, b: &mut Budget) -> Result<(), HeadError> {
    bytes(v.package.len(), b)
}
fn span(v: &ProjectedSpan, b: &mut Budget) -> Result<(), HeadError> {
    slot::<ProjectedSpan>(b)?;
    bytes(v.source.source_id.0.len(), b)
}
fn entry(v: &EntryContext, b: &mut Budget) -> Result<(), HeadError> {
    slot::<EntryContext>(b)?;
    schema(&v.package.schema, b)?;
    for text in [&v.alias, &v.category, &v.mode] {
        bytes(text.len(), b)?;
    }
    Ok(())
}
fn token(v: &ProjectedToken, b: &mut Budget) -> Result<(), HeadError> {
    slot::<ProjectedToken>(b)?;
    schema(&v.kind.schema, b)?;
    span(&v.head, b)?;
    v.payload.charge_clone(b)?;
    Ok(())
}
fn window(v: &SourceWindow, b: &mut Budget) -> Result<(), HeadError> {
    slot::<SourceWindow>(b)?;
    span(&v.span, b)?;
    bytes(v.bytes.len(), b)
}
impl HeadShape {
    pub(crate) fn charge_clone(&self, b: &mut Budget) -> Result<(), HeadError> {
        slot::<Self>(b)?;
        schema(&self.kind.schema, b)?;
        for field in &self.fields {
            slot::<crate::package::FieldSpec>(b)?;
            bytes(field.name.len(), b)?;
        }
        for style in &self.styles {
            slot::<crate::package::StyleRule>(b)?;
            schema(&style.class.schema, b)?;
            bytes(style.class.name.len(), b)?;
            if let StyleSelector::Field(v) | StyleSelector::Capture(v) = &style.selector {
                bytes(v.len(), b)?;
            }
        }
        Ok(())
    }
    pub(crate) fn clone_with_budget(&self, b: &mut Budget) -> Result<Self, HeadError> {
        self.charge_clone(b)?;
        Ok(self.clone())
    }
}
impl HeadCall {
    pub fn charge_clone(&self, b: &mut Budget) -> Result<(), HeadError> {
        slot::<Self>(b)?;
        bytes(self.identity.session_id.len(), b)?;
        schema(&self.identity.operation.schema, b)?;
        bytes(self.identity.operation.name.len(), b)?;
        entry(&self.entry, b)?;
        token(&self.head.token, b)?;
        window(&self.head.window, b)?;
        if let HeadRequest::ChildContext {
            shape, completed, ..
        } = &self.request
        {
            shape.charge_clone(b)?;
            for _ in &completed.roots {
                slot::<ProjectedNodeRef>(b)?;
            }
            for v in &completed.windows {
                window(v, b)?;
            }
            for node in &completed.nodes {
                slot::<ProjectedNode>(b)?;
                schema(&node.schema, b)?;
                bytes(node.kind.len(), b)?;
                for v in node.head.iter().chain(&node.cover) {
                    span(v, b)?;
                }
                if let Some(v) = &node.token {
                    token(v, b)?;
                }
                for field in &node.fields {
                    slot::<ProjectedFieldValue>(b)?;
                    match field {
                        ProjectedFieldValue::Atom(v) => super::projection::charge_scalar(v, b)?,
                        ProjectedFieldValue::Child(_) => {}
                        ProjectedFieldValue::Children(v) => {
                            for _ in v {
                                slot::<ProjectedNodeRef>(b)?;
                            }
                        }
                        ProjectedFieldValue::Foreign {
                            schema: s,
                            category,
                            ..
                        } => {
                            schema(s, b)?;
                            bytes(category.len(), b)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub fn clone_with_budget(&self, b: &mut Budget) -> Result<Self, HeadError> {
        self.charge_clone(b)?;
        Ok(self.clone())
    }
}

fn typed(v: &nepl3_core::value::TypedValue, b: &mut Budget) -> Result<(), HeadError> {
    use nepl3_core::value::TypedValue;
    slot::<TypedValue>(b)?;
    let fields = match v {
        TypedValue::Record(v) => {
            schema(&v.schema, b)?;
            bytes(v.kind.len(), b)?;
            &v.fields
        }
        TypedValue::Variant(v) => {
            schema(&v.schema, b)?;
            bytes(v.type_name.len(), b)?;
            bytes(v.variant.len(), b)?;
            &v.fields
        }
    };
    for value in fields {
        value.charge_clone(b)?;
    }
    Ok(())
}
impl ProjectedDiagnostic {
    pub fn charge_clone(&self, b: &mut Budget) -> Result<(), HeadError> {
        slot::<Self>(b)?;
        schema(&self.schema, b)?;
        bytes(self.code.len(), b)?;
        bytes(self.stage.len(), b)?;
        typed(&self.arguments, b)?;
        if let Some(v) = &self.primary {
            span(v, b)?;
        }
        for v in &self.related {
            slot::<ProjectedRelated>(b)?;
            bytes(v.code.len(), b)?;
            typed(&v.arguments, b)?;
            if let Some(v) = &v.span {
                span(v, b)?;
            }
        }
        for fix in &self.fixes {
            slot::<ProjectedFix>(b)?;
            bytes(fix.id.len(), b)?;
            for edit in &fix.edits {
                slot::<ProjectedEdit>(b)?;
                span(&edit.span, b)?;
                bytes(edit.replacement.len(), b)?;
            }
        }
        Ok(())
    }
}
impl HeadReply {
    pub fn charge_clone(&self, b: &mut Budget) -> Result<(), HeadError> {
        slot::<Self>(b)?;
        bytes(self.identity.session_id.len(), b)?;
        schema(&self.identity.operation.schema, b)?;
        bytes(self.identity.operation.name.len(), b)?;
        match &self.outcome {
            HeadOutcome::Shape { shape: Some(shape) } => shape.charge_clone(b)?,
            HeadOutcome::ChildContext { context } => entry(context, b)?,
            HeadOutcome::Failed { diagnostic } => diagnostic.charge_clone(b)?,
            _ => {}
        }
        for value in &self.report.diagnostics {
            value.charge_clone(b)?;
        }
        for value in &self.report.events {
            slot::<ProjectedEvent>(b)?;
            schema(&value.schema, b)?;
            bytes(value.kind.len(), b)?;
            typed(&value.payload, b)?;
            if let Some(v) = &value.span {
                span(v, b)?;
            }
            b.charge(Resource::Work, value.operation_path.len() as u64)?;
            b.charge(
                Resource::AllocationUnits,
                (value.operation_path.len() as u64).saturating_mul(8),
            )?;
        }
        Ok(())
    }
    pub fn clone_with_budget(&self, b: &mut Budget) -> Result<Self, HeadError> {
        self.charge_clone(b)?;
        Ok(self.clone())
    }
}
