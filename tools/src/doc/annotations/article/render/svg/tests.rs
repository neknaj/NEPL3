use super::*;
use crate::doc::{
    annotations::article,
    source::{budget, compiled, err, with_input_route},
};
use nepl3_core::source::{SourceAdmission, SourceStore};
use nepl3_doc_core::{
    check::Category,
    labels::{LabelError, LabelSite},
    prepare::{NamespacePreparationError, PreparationError},
};
use nepl3_wire::foundation::FoundationCodec;

#[test]
fn planned_svg_host_owns_borrowed_diagnostics_and_preserves_stop_precedence() -> Result<(), String>
{
    let compiled = compiled()?;
    with_input_route(
        true,
        &compiled,
        r#"article en "Title" body nil"#,
        "Article",
        |tree, profile, initial, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let root = nepl3_suite::adapters::sentence::literal::lower(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                initial,
                &mut codec,
            )
            .map_err(err)?;
            let mut host = SentenceAnnotationRenderer {
                registry: profile.registry(),
                surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                doc_surface: Some(&compiled.doc.package.schema),
                codec: &mut codec,
            };
            let selection = article::collect_structural(&root, &mut host, initial).map_err(err)?;
            let (_, ()) = namespace::resolve_with(&selection, &[], &mut host, initial, |checked, members, host, _| {
            // Synthetic conversion-boundary failure, not an actual label-resolution
            // result. Its borrowed name must not escape into the returned error.
            let mut convert = |member, b: &mut Budget| {
                let name = String::from("temporary-label");
                let failure = namespace_assets::PreparationError::Input(NamespacePreparationError {
                    member,
                    error: PreparationError::Label(LabelError::Unresolved { reference: LabelSite { node: 0, name: &name, selection: None, range: None } }),
                });
                preparation_failure::<_, ()>(failure, checked, members, host, b)
            };
            let mut baseline = budget();
            let failure = convert(Some(MemberId(0)), &mut baseline);
            let Error::Preparation { location, error } = failure else { return Err(err(failure)); };
            assert_eq!(location, Location { member: Some(MemberId(0)), origin: Some(MemberOrigin::Article) });
            let AssetFailure::Document(document::Error::Label(diagnostic)) = *error else { return Err(err(error)); };
            assert_eq!(diagnostic.code, "UnresolvedLabel");
            let used = baseline.usage().allocation_units;
            for short in [false, true] {
                let mut limits = baseline.limits(); limits.allocation_units = used - u64::from(short);
                let mut b = Budget::new(limits); let failure = convert(Some(MemberId(0)), &mut b);
                if short { assert!(matches!(failure, Error::Stopped(StopReason::AllocationLimit))); assert_eq!(b.poll(), Err(StopReason::AllocationLimit)); }
                else { assert!(matches!(failure, Error::Preparation { error, .. } if matches!(*error, AssetFailure::Document(document::Error::Label(_))))); }
            }
            let unlocated = convert(None, &mut budget());
            assert!(matches!(unlocated, Error::Preparation { location: Location { member: None, origin: None }, error } if matches!(*error, AssetFailure::Document(document::Error::Selection))));
            let mut cancelled = budget(); cancelled.cancel();
            assert!(matches!(convert(Some(MemberId(0)), &mut cancelled), Error::Stopped(StopReason::Cancelled)));
            Ok::<_, String>(())
        }).map_err(err)?;
            Ok(())
        },
    )
}
