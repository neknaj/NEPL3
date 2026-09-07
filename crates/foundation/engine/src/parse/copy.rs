//! Owned continuation copies precharge their complete transitive storage.
use super::{build::slot, model::*};
use crate::{
    package::EntryContext,
    selection::{NodeSelection, ShapeSelection},
};
use alloc::vec::Vec;
use nepl3_core::budget::{Budget, Resource, StopReason};
fn charge_entry(v: &EntryContext, b: &mut Budget) -> Result<(), StopReason> {
    slot::<EntryContext>(b)?;
    b.charge(
        Resource::AllocationUnits,
        (v.alias.len() + v.category.len() + v.mode.len() + v.package.schema.package.len()) as u64,
    )?;
    Ok(())
}
pub(super) fn entry(v: &EntryContext, b: &mut Budget) -> Result<EntryContext, StopReason> {
    charge_entry(v, b)?;
    Ok(v.clone())
}
fn shape(v: &ShapeSelection, b: &mut Budget) -> Result<(), StopReason> {
    slot::<ShapeSelection>(b)?;
    if let ShapeSelection::Dynamic {
        provider,
        shape,
        child_contexts,
    } = v
    {
        b.charge(
            Resource::Work,
            (provider.shape.schema.package.len()
                + provider.shape.name.len()
                + provider.child_context.schema.package.len()
                + provider.child_context.name.len()
                + shape.kind.schema.package.len()) as u64
                + 1,
        )?;
        b.charge(
            Resource::AllocationUnits,
            (provider.shape.schema.package.len()
                + provider.shape.name.len()
                + provider.child_context.schema.package.len()
                + provider.child_context.name.len()
                + shape.kind.schema.package.len()) as u64,
        )?;
        slot::<crate::selection::HeadShape>(b)?;
        for context in child_contexts {
            b.charge(
                Resource::Work,
                (context.alias.len()
                    + context.category.len()
                    + context.mode.len()
                    + context.package.schema.package.len()) as u64
                    + 1,
            )?;
            charge_entry(context, b)?;
        }
        for field in &shape.fields {
            b.charge(Resource::Work, field.name.len() as u64 + 1)?;
            slot::<crate::package::FieldSpec>(b)?;
            b.charge(Resource::AllocationUnits, field.name.len() as u64)?;
        }
        for style in &shape.styles {
            b.charge(
                Resource::Work,
                (style.class.schema.package.len() + style.class.name.len()) as u64 + 1,
            )?;
            slot::<crate::package::StyleRule>(b)?;
            b.charge(
                Resource::AllocationUnits,
                (style.class.schema.package.len() + style.class.name.len()) as u64,
            )?;
            if let crate::package::StyleSelector::Field(v)
            | crate::package::StyleSelector::Capture(v) = &style.selector
            {
                b.charge(Resource::Work, v.len() as u64 + 1)?;
                b.charge(Resource::AllocationUnits, v.len() as u64)?;
            }
        }
    }
    Ok(())
}
pub(super) fn selection(
    value: &ShapeSelection,
    budget: &mut Budget,
) -> Result<ShapeSelection, StopReason> {
    shape(value, budget)?;
    Ok(value.clone())
}
pub(super) fn states(
    values: &[LanguageReaderState],
    b: &mut Budget,
) -> Result<Vec<LanguageReaderState>, StopReason> {
    let mut out = Vec::new();
    for value in values {
        slot::<LanguageReaderState>(b)?;
        out.push(LanguageReaderState {
            alias: super::build::text(&value.alias, b)?,
            state: value.state.clone_with_budget(b)?,
        });
    }
    Ok(out)
}
impl ParseProgress {
    pub(super) fn charge_clone(&self, b: &mut Budget) -> Result<(), StopReason> {
        slot::<Self>(b)?;
        b.charge(
            Resource::AllocationUnits,
            (self.request.snapshot.source_id.0.len()
                + self.scope.snapshot.source_id.0.len()
                + self.scope.operation_id.len()) as u64,
        )?;
        charge_entry(&self.request.entry, b)?;
        for value in &self.request.environments {
            slot::<LanguageEnvironmentRef>(b)?;
            b.charge(Resource::AllocationUnits, value.alias.len() as u64)?;
        }
        for value in &self.request.environment_entries {
            value.charge_clone(b)?;
        }
        for value in &self.request.origins {
            value.charge_clone(b)?;
        }
        for value in &self.request.sources {
            value.charge_clone(b)?;
        }
        for values in [&self.request.states, &self.states] {
            for v in values {
                slot::<LanguageReaderState>(b)?;
                b.charge(Resource::AllocationUnits, v.alias.len() as u64)?;
                v.state.charge_clone(b)?;
            }
        }
        for arena in &self.arenas {
            slot::<ParseArena>(b)?;
            path(&arena.path, b)?;
            for v in &arena.sources {
                v.charge_clone(b)?;
            }
            for v in &arena.origins {
                v.charge_clone(b)?;
            }
            for v in &arena.tokens {
                v.charge_clone(b)?;
            }
            for v in &arena.source_maps {
                v.charge_clone(b)?;
            }
            for v in &arena.environments {
                v.charge_clone(b)?;
            }
            for node in &arena.nodes {
                slot::<nepl3_core::syntax::SyntaxNode>(b)?;
                b.charge(
                    Resource::AllocationUnits,
                    (node.schema.package.len() + node.kind.len()) as u64,
                )?;
                for span in node.head.iter().chain(&node.cover) {
                    slot::<nepl3_core::source::Span>(b)?;
                    b.charge(
                        Resource::AllocationUnits,
                        span.snapshot_ref().source.0.len() as u64,
                    )?;
                }
                for field in &node.fields {
                    field.charge_clone(b)?;
                }
            }
            for selected in &arena.selections {
                slot::<NodeSelection>(b)?;
                charge_entry(&selected.entry, b)?;
                shape(&selected.shape, b)?;
            }
        }
        for context in &self.contexts {
            slot::<crate::selection::BundleContext>(b)?;
            path(&context.path, b)?;
            for selected in &context.nodes {
                slot::<NodeSelection>(b)?;
                charge_entry(&selected.entry, b)?;
                shape(&selected.shape, b)?;
            }
        }
        for frame in &self.frames {
            slot::<ParseFrame>(b)?;
            charge_entry(&frame.entry, b)?;
            if let Some(v) = &frame.selection {
                shape(v, b)?;
            }
            for v in &frame.children {
                v.charge_clone(b)?;
            }
        }
        for batch in &self.facts {
            slot::<ReaderFactBatch>(b)?;
            path(&batch.path, b)?;
            charge_entry(&batch.entry, b)?;
            for trivia in &batch.trivia {
                slot::<nepl3_core::view::Trivia>(b)?;
                b.charge(
                    Resource::AllocationUnits,
                    trivia.span.snapshot_ref().source.0.len() as u64,
                )?;
            }
            for fact in &batch.facts {
                slot::<nepl3_reader::model::ReaderFact>(b)?;
                match fact {
                    nepl3_reader::model::ReaderFact::Capture { name, span } => {
                        b.charge(
                            Resource::AllocationUnits,
                            name.len() as u64 + span.snapshot_ref().source.0.len() as u64,
                        )?;
                    }
                    nepl3_reader::model::ReaderFact::Presentation { class, span } => {
                        b.charge(
                            Resource::AllocationUnits,
                            (class.schema.package.len()
                                + class.name.len()
                                + span.snapshot_ref().source.0.len())
                                as u64,
                        )?;
                    }
                    nepl3_reader::model::ReaderFact::Relation {
                        schema,
                        kind,
                        from,
                        to,
                    } => {
                        b.charge(
                            Resource::AllocationUnits,
                            (schema.package.len()
                                + kind.len()
                                + from.snapshot_ref().source.0.len()
                                + to.snapshot_ref().source.0.len())
                                as u64,
                        )?;
                    }
                }
            }
        }
        for bundle in &self.recovery {
            slot::<crate::recovery::BundleRecovery>(b)?;
            for step in &bundle.path {
                slot::<crate::recovery::ForeignStep>(b)?;
                b.charge(Resource::AllocationUnits, step.field.len() as u64)?;
            }
            for recovered in &bundle.entries {
                slot::<crate::recovery::RecoveryEntry>(b)?;
                match &recovered.kind {
                    crate::recovery::RecoveryKind::Missing { expected, anchor } => {
                        charge_entry(expected, b)?;
                        b.charge(
                            Resource::AllocationUnits,
                            anchor.snapshot_ref().source.0.len() as u64,
                        )?;
                    }
                    crate::recovery::RecoveryKind::Unparsed { span, .. } => {
                        b.charge(
                            Resource::AllocationUnits,
                            span.snapshot_ref().source.0.len() as u64,
                        )?;
                    }
                    crate::recovery::RecoveryKind::Unexpected { .. } => {}
                }
            }
        }
        Ok(())
    }
    pub(super) fn clone_with_budget(&self, b: &mut Budget) -> Result<Self, StopReason> {
        self.charge_clone(b)?;
        Ok(self.clone())
    }
}

fn path(values: &[crate::recovery::ForeignStep], b: &mut Budget) -> Result<(), StopReason> {
    for step in values {
        slot::<crate::recovery::ForeignStep>(b)?;
        b.charge(Resource::AllocationUnits, step.field.len() as u64)?;
    }
    Ok(())
}
