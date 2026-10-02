use super::*;
use crate::package::{
    BindingId, FieldSpec, PackageIdentity, ReadSpecId, SelectionRule, StyleRule, StyleSelector,
};
use alloc::{boxed::Box, format, vec};
use nepl3_core::{
    budget::Limits,
    source::Digest,
    value::{KindRef, OperationRef, SchemaRef},
};

fn schema(name: &str) -> SchemaRef {
    SchemaRef {
        package: name.into(),
        revision: 1,
        digest: Digest::of(b"schema"),
    }
}
fn dynamic(width: usize, package_bytes: usize) -> ShapeSelection {
    let operation = OperationRef {
        schema: schema("provider"),
        name: "shape".into(),
    };
    let entry = EntryContext {
        package: PackageIdentity {
            schema: schema(&"p".repeat(package_bytes)),
            semantic_digest: Digest::of(b"package"),
        },
        alias: "Guest".into(),
        category: "Expr".into(),
        mode: "Alt".into(),
    };
    ShapeSelection::Dynamic {
        provider: HeadProviderRef {
            shape: operation.clone(),
            child_context: OperationRef {
                name: "context".into(),
                ..operation
            },
        },
        shape: Box::new(HeadShape {
            kind: KindRef {
                schema: schema("syntax"),
                local_kind: 0,
            },
            fields: (0..width)
                .map(|i| FieldSpec {
                    name: format!("field-{i}"),
                    read: ReadSpecId(0),
                })
                .collect(),
            binding: BindingId(0),
            styles: (0..width)
                .map(|i| StyleRule {
                    selector: StyleSelector::Field(format!("field-{i}")),
                    class: nepl3_core::view::PresentationClass {
                        schema: schema("presentation"),
                        name: format!("style-{i}"),
                        fallback: nepl3_core::view::FallbackRole::Content,
                    },
                })
                .collect(),
            selection_rules: (0..width)
                .map(|i| SelectionRule {
                    selector: StyleSelector::Capture(format!("capture-{i}")),
                    priority: i as u64,
                })
                .collect(),
        }),
        child_contexts: vec![entry; width],
    }
}
fn budget(work: u64) -> Budget {
    Budget::new(Limits {
        work,
        ..Limits::default()
    })
}
#[test]
fn exact_work_boundary_covers_wide_dynamic_metadata_and_long_package_names()
-> Result<(), StopReason> {
    let value = dynamic(100, 1000);
    let mut measured = budget(u64::MAX);
    assert!(value.same_resolved_with_budget(&value, &mut measured)?);
    let work = measured.usage().work;
    assert!(work > 200_000);
    assert_eq!(
        value.same_resolved_with_budget(&value, &mut budget(work - 1)),
        Err(StopReason::WorkLimit)
    );
    let mut exact = budget(work);
    assert!(value.same_resolved_with_budget(&value, &mut exact)?);
    assert_eq!(exact.usage(), measured.usage());
    Ok(())
}
#[test]
fn dynamic_context_identity_and_last_selection_rule_are_compared() -> Result<(), StopReason> {
    let value = dynamic(20, 10);
    let mut other = value.clone();
    let ShapeSelection::Dynamic { shape, .. } = &mut other else {
        return Err(StopReason::WorkLimit);
    };
    let Some(last) = shape.selection_rules.last_mut() else {
        return Err(StopReason::WorkLimit);
    };
    last.priority += 1;
    assert!(!value.same_resolved_with_budget(&other, &mut budget(u64::MAX))?);
    let mut other = value.clone();
    let ShapeSelection::Dynamic { child_contexts, .. } = &mut other else {
        return Err(StopReason::WorkLimit);
    };
    let Some(last) = child_contexts.last_mut() else {
        return Err(StopReason::WorkLimit);
    };
    last.package.schema.package.push('x');
    assert!(!value.same_resolved_with_budget(&other, &mut budget(u64::MAX))?);
    Ok(())
}
#[test]
fn cancelled_comparison_and_recovery_never_report_resolved_equality() {
    let value = dynamic(1, 1);
    let mut cancelled = budget(u64::MAX);
    cancelled.cancel();
    assert_eq!(
        value.same_resolved_with_budget(&value, &mut cancelled),
        Err(StopReason::Cancelled)
    );
    assert_eq!(
        ShapeSelection::Recovery
            .same_resolved_with_budget(&ShapeSelection::Recovery, &mut budget(1)),
        Ok(false)
    );
}
