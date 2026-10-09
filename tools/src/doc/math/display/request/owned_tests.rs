use super::*;
use crate::doc::{
    math::{MathDisplayHost, display::Preference},
    source::{budget, compiled, err, with_input},
};
use nepl3_core::source::{SourceAdmission, SourceStore};
use nepl3_doc_core::{check::Category, lower, model::DocKind};
use nepl3_wire::foundation::FoundationCodec;
fn owner(preference: Preference) -> Result<PreparedDisplay, String> {
    let c = compiled()?;
    with_input(
        &c,
        "article en \"T\" body cons display Math frac 1 2 nil",
        "Article",
        |tree, profile, _, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let doc = lower::document(
                tree.syntax(),
                &c.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let node = doc
                .value
                .nodes
                .iter()
                .position(|n| matches!(n.kind, DocKind::DisplayMath { .. }))
                .ok_or("Math")?;
            let mut host = MathDisplayHost {
                registry: profile.registry(),
                math_surface: &c.others[0].schema,
                sentence_surface: Some(&c.others[3].schema),
                doc_surface: None,
                codec: &mut codec,
            };
            host.prepare_node(&doc, node as u64, preference, &mut budget())
                .map_err(err)
        },
    )
}
fn controls() -> Controls {
    Controls {
        limits: super::super::RenderLimits {
            input_bytes: 4096,
            output_bytes: 4096,
        },
        parse_limits: super::super::ParseLimits {
            input_bytes: 4096,
            nodes: 4096,
            depth: 64,
        },
        options: super::super::Options {
            timeout_millis: 1000,
            diagnostic_bytes: 256,
            reply_bytes: 4096,
            module_bytes: 4096,
        },
    }
}
#[test]
fn owned_request_moves_exact_wire_and_owner_without_another_charge() -> Result<(), String> {
    let original = owner(Preference::KaTeXPreferred)?;
    let pointer = original.mathml().syntax.value.nodes.as_ptr();
    let mut reference_budget = budget();
    let reference = super::super::prepare(&original, controls(), 4096, &mut reference_budget)
        .map_err(err)?
        .ok_or("request")?;
    let bytes = reference.bytes().to_vec();
    drop(reference);
    let mut b = budget();
    let Preparation::Ready(request) = prepare(original, controls(), 4096, &mut b) else {
        return Err("owned preparation".into());
    };
    assert_eq!(b.usage(), reference_budget.usage());
    let wire_pointer = request.bytes.as_ptr();
    let usage = b.usage();
    let external = String::from("external lifetime");
    let (restored, result) = request.consume(|request| {
        assert_eq!(request.bytes(), bytes);
        assert_eq!(request.bytes().as_ptr(), wire_pointer);
        assert_eq!(request.controls(), controls());
        assert_eq!(
            request.owner().mathml().syntax.value.nodes.as_ptr(),
            pointer
        );
        external.as_str()
    });
    assert_eq!(result, external);
    assert_eq!(restored.mathml().syntax.value.nodes.as_ptr(), pointer);
    assert_eq!(b.usage(), usage);
    Ok(())
}
#[test]
fn owned_request_preserves_skipped_rejected_and_stopped_inputs() -> Result<(), String> {
    for mode in 0..3 {
        let original = owner(if mode == 0 {
            Preference::MathMLOnly
        } else {
            Preference::KaTeXPreferred
        })?;
        let pointer = original.mathml().syntax.value.nodes.as_ptr();
        let mut c = controls();
        if mode == 1 {
            c.options.timeout_millis = 0;
        }
        let mut b = budget();
        if mode == 2 {
            b.cancel();
        }
        let prepared = match (mode, prepare(original, c, 4096, &mut b)) {
            (0, Preparation::NotRequested(owner)) => owner,
            (
                1,
                Preparation::Rejected {
                    owner,
                    error: Error::InvalidControls,
                },
            ) => owner,
            (
                2,
                Preparation::Rejected {
                    owner,
                    error: Error::Stopped(nepl3_core::budget::StopReason::Cancelled),
                },
            ) => owner,
            _ => return Err("unexpected preparation classification".into()),
        };
        assert_eq!(prepared.mathml().syntax.value.nodes.as_ptr(), pointer);
        assert_eq!(b.usage().allocation_units, 0);
        assert_eq!(b.usage().output_bytes, 0);
    }
    Ok(())
}
