use super::*;
use nepl3_engine::{
    analysis::{BindingOptions, alternatives::*, expected::ExpectedReadRequest},
    portable::analysis,
};

#[test]
fn root_declaration_metadata_records_dynamic_registration_without_calling_provider() -> TestResult {
    run_case_inspect("", Case::Foreign, false, |reply, profile, source| {
        let ParseOutcome::Recovered { tree, .. } = &reply.outcome else {
            return Err(format!("{reply:?}"));
        };
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut c = FoundationCodec::new(profile.registry(), &empty, &mut admission)
            .map_err(|e| format!("{e:?}"))?;
        let input = analysis::prepare(
            "alternative-dynamic",
            tree,
            BindingOptions,
            budget().limits(),
            profile,
            &mut c,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let reply = declared_alternatives(
            &input,
            &ExpectedReadRequest {
                key: input.key(),
                source: source.reference(),
                offset: 0,
            },
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        let DeclaredAlternativesOutcome::Complete(Some(value)) = reply.outcome else {
            return Err(format!("{reply:?}"));
        };
        let DeclaredReadAlternatives::Category {
            dynamic_fallback_registered,
            ..
        } = value.alternatives
        else {
            return Err("category".into());
        };
        assert!(dynamic_fallback_registered);
        // The query accepts no host callback and does not manufacture heads.
        assert_eq!(value.read.expected.alias, "Host");
        assert_eq!(value.read.expected.category, "Expr");
        Ok(())
    })?;
    Ok(())
}
