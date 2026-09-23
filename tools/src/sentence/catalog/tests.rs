use super::*;
use compile::package::ForeignForm;
use nepl3_core::source::Digest;

fn form() -> ForeignForm<'static> {
    ForeignForm {
        kind: "InlineMath",
        category: "Inline",
        spelling: "math",
        field: "syntax",
        alias: "Math",
        guest_category: "Expr",
        origin_reason: "explicit test composition",
    }
}

#[test]
fn typed_foreign_forms_change_only_the_selected_surface() -> Result<(), String> {
    let implementation = Digest::of(b"test readers");
    let base = standard(
        implementation,
        &mut crate::doc::source::budget(),
        &mut SourceAdmission::default(),
    )?;
    let document = standard_document(
        implementation,
        &mut crate::doc::source::budget(),
        &mut SourceAdmission::default(),
    )?;
    let selected = standard_with_foreign_forms(
        implementation,
        &[form()],
        &mut crate::doc::source::budget(),
        &mut SourceAdmission::default(),
    )?;
    assert_ne!(base.package.schema, selected.package.schema);
    assert_eq!(base.package.forms.len() + 1, selected.package.forms.len());
    assert!(!base.package.forms.iter().any(|f| f.spelling == "math"));
    let mut full = crate::doc::source::budget();
    compile_with_foreign_forms(
        &document,
        "nepl3.syntax.sentence",
        &[form()],
        &mut full,
        &mut SourceAdmission::default(),
    )?;
    for allocation in [false, true] {
        let mut limits = full.limits();
        if allocation {
            limits.allocation_units = full.usage().allocation_units - 1;
        } else {
            limits.work = full.usage().work - 1;
        }
        let mut bounded = Budget::new(limits);
        let result = compile_with_foreign_forms(
            &document,
            "nepl3.syntax.sentence",
            &[form()],
            &mut bounded,
            &mut SourceAdmission::default(),
        );
        assert!(
            result.is_err(),
            "allocation={allocation}, full={:?}, bounded={:?}",
            full.usage(),
            bounded.usage()
        );
        assert_eq!(
            bounded.poll(),
            Err(if allocation {
                nepl3_core::budget::StopReason::AllocationLimit
            } else {
                nepl3_core::budget::StopReason::WorkLimit
            })
        );
    }
    let added = selected.package.forms.last().ok_or("added form")?;
    assert_eq!(added.category, "Inline");
    assert_eq!(added.fields.len(), 1);
    assert_eq!(
        selected.package.reads[added.fields[0].read.0 as usize],
        nepl3_engine::package::ReadSpec::Foreign {
            alias: "Math".into(),
            category: "Expr".into()
        }
    );
    let declaration = selected
        .package
        .provenance
        .declarations
        .last()
        .ok_or("origin")?;
    assert!(
        matches!(&selected.package.provenance.origins[declaration.origin.0 as usize],
        nepl3_core::origin::Origin::Synthetic { reason, anchor: None } if reason == "explicit test composition")
    );
    // Source provenance is retained byte-for-byte; generated forms have no fake spans.
    assert_eq!(
        base.package.provenance.sources,
        selected.package.provenance.sources
    );
    for case in 0..5 {
        let mut invalid = form();
        match case {
            0 => invalid.kind = "Text",
            1 => invalid.spelling = "text",
            2 => invalid.category = "Missing",
            3 => invalid.alias = "",
            _ => invalid.origin_reason = "",
        }
        assert!(
            standard_with_foreign_forms(
                implementation,
                &[invalid],
                &mut crate::doc::source::budget(),
                &mut SourceAdmission::default()
            )
            .is_err(),
            "case {case}"
        );
    }
    assert!(
        standard_with_foreign_forms(
            implementation,
            &[form(), form()],
            &mut crate::doc::source::budget(),
            &mut SourceAdmission::default()
        )
        .is_err()
    );
    Ok(())
}
