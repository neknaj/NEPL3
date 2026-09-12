use super::*;
use nepl3_math_core::{
    check::Category,
    model::*,
    print::{self, GuestPrinter},
};
struct FixtureGuest<'a> {
    expected: Option<&'a nepl3_core::syntax::ForeignClosure>,
    text: &'a str,
    calls: usize,
}
impl GuestPrinter for FixtureGuest<'_> {
    type Error = String;
    fn print(
        &mut self,
        guest: &nepl3_core::syntax::ForeignClosure,
        b: &mut Budget,
    ) -> Result<String, String> {
        b.poll().map_err(err)?;
        if !self
            .expected
            .is_some_and(|expected| core::ptr::eq(expected, guest))
        {
            return Err("different guest closure".into());
        }
        self.calls += 1;
        Ok(self.text.into())
    }
}
fn lower_value(compiled: &Compiled, source: &str, entry: &str) -> Result<MathValue, String> {
    Ok(lower_syntax(compiled, source, entry)?.value)
}
fn lower_syntax(compiled: &Compiled, source: &str, entry: &str) -> Result<MathSyntax, String> {
    let mut output = None;
    with_input(compiled, source, entry, |tree, profile, b, a| {
        let syntax = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let category = match entry {
            "Expr" => Category::Expr,
            "Row" => Category::Row,
            _ => Category::DocGuest,
        };
        output = Some(
            nepl3_math_core::lower::expression(
                &syntax,
                &compiled.others[0].schema,
                category,
                profile.registry(),
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?,
        );
        Ok(())
    })?;
    output.ok_or("missing lower result".into())
}
fn notation(value: &MathValue) -> Vec<MathKind> {
    value
        .nodes
        .iter()
        .map(|node| match &node.kind {
            MathKind::Number { value, .. } => MathKind::Number {
                value: value.clone(),
                spelling: None,
            },
            other => other.clone(),
        })
        .collect()
}

#[test]
fn explicit_print_requests_bind_guest_assertions_and_preserve_stops() -> Result<(), String> {
    use nepl3_math_core::{portable::printing as wire, print::request as op};
    let compiled = compiled()?;
    let registry = &compiled.doc.registry;
    let empty = SourceStore::default();
    let syntax = lower_syntax(&compiled, "label frac 1 0 Doc \"note\"", "Expr")?;
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
    let identity = op::identity(&syntax, registry, &mut codec, &mut budget()).map_err(err)?;
    let raw =
        wire::identity_to_value(&identity, registry, &mut codec, &mut budget()).map_err(err)?;
    assert_eq!(
        wire::identity_from_value(&raw, registry, &mut codec, &mut budget()).map_err(err)?,
        identity
    );
    let mut host = nepl3_tools::doc::printing::DocGuestPrinter {
        registry,
        surface: &compiled.doc.package.schema,
        math_surface: None,
        codec: &mut codec,
    };
    let text = host
        .print(&syntax.value.embeds[0], &mut budget())
        .map_err(err)?;
    let request = MathPrintRequest {
        syntax,
        doc_schema: Some(compiled.doc.package.schema.clone()),
        guests: vec![MathPrintedGuest {
            syntax_digest: identity.syntax_digest,
            guest_digest: identity.guests[0],
            embed: EmbedRef(0),
            text,
        }],
    };
    let expected = MathPrintResult::Complete {
        artifact: MathSourceArtifact {
            text: "label frac 1 0 Doc sentence cons text \"note\" nil".into(),
            entry: MathCategory::Expr,
        },
    };
    let mut full = budget();
    // Fresh admission makes the baseline comparable to each bounded execution.
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
    assert_eq!(
        op::execute(&request, registry, &mut codec, &mut full).map_err(err)?,
        expected
    );
    let raw = wire::request_to_value(&request, registry, &mut codec, &mut budget()).map_err(err)?;
    let bytes = nepl3_wire::encode(&raw, &mut budget()).map_err(err)?;
    let raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
    let mut receive_admission = SourceAdmission::default();
    let mut receiver =
        FoundationCodec::new(registry, &empty, &mut receive_admission).map_err(err)?;
    let mut receive_budget = budget();
    let received = wire::request_from_value(&raw, registry, &mut receiver, &mut receive_budget)
        .map_err(err)?;
    assert_eq!(
        op::execute(&received, registry, &mut receiver, &mut receive_budget).map_err(err)?,
        expected
    );
    for case in 0..8 {
        let mut bad = request.clone();
        let failure = match case {
            0 => {
                bad.guests[0].syntax_digest.0[0] ^= 1;
                MathPrintFailure::InvalidGuestIdentity { entry: 0 }
            }
            1 => {
                bad.guests[0].guest_digest.0[0] ^= 1;
                MathPrintFailure::InvalidGuestIdentity { entry: 0 }
            }
            2 => {
                bad.guests[0].embed = EmbedRef(u64::MAX);
                MathPrintFailure::InvalidGuestIdentity { entry: 0 }
            }
            3 => {
                bad.guests.push(bad.guests[0].clone());
                MathPrintFailure::DuplicateGuest { embed: EmbedRef(0) }
            }
            4 => {
                bad.doc_schema = None;
                MathPrintFailure::MissingBinding { embed: EmbedRef(0) }
            }
            5 => {
                bad.doc_schema = Some(compiled.others[0].schema.clone());
                MathPrintFailure::GuestCategory { embed: EmbedRef(0) }
            }
            6 => {
                bad.guests.clear();
                MathPrintFailure::UnresolvedGuest { embed: EmbedRef(0) }
            }
            _ => {
                bad.guests[0].text = " \n".into();
                MathPrintFailure::EmptyGuest { embed: EmbedRef(0) }
            }
        };
        let result = op::execute(&bad, registry, &mut codec, &mut budget()).map_err(err)?;
        assert_eq!(result, MathPrintResult::Invalid { failure });
        let raw =
            wire::result_to_value(&result, registry, &mut codec, &mut budget()).map_err(err)?;
        let bytes = nepl3_wire::encode(&raw, &mut budget()).map_err(err)?;
        let raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
        assert_eq!(
            wire::result_from_value(&raw, registry, &mut codec, &mut budget()).map_err(err)?,
            result
        );
    }
    let used = full.usage();
    let complete =
        wire::result_to_value(&expected, registry, &mut codec, &mut budget()).map_err(err)?;
    let bytes = nepl3_wire::encode(&complete, &mut budget()).map_err(err)?;
    let complete = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
    assert_eq!(
        wire::result_from_value(&complete, registry, &mut codec, &mut budget()).map_err(err)?,
        expected
    );
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        op::execute(&request, registry, &mut codec, &mut cancelled).map_err(err)?,
        MathPrintResult::Stopped {
            reason: StopReason::Cancelled
        }
    );
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    let mut invalid = request.clone();
    invalid.syntax.value.root = MathRoot::Expr(ExprRef(u64::MAX));
    assert!(matches!(
        op::execute(&invalid, registry, &mut codec, &mut budget()),
        Err(nepl3_math_core::portable::PortableError::Structure(_))
    ));
    let mut named = lower_syntax(&compiled, "let x 1 x", "Expr")?;
    let node = named
        .value
        .nodes
        .iter()
        .position(|n| matches!(n.kind, MathKind::Let { .. }))
        .ok_or("let")?;
    if let MathKind::Let { name, .. } = &mut named.value.nodes[node].kind {
        *name = "bad name".into();
    }
    let named = MathPrintRequest {
        syntax: named,
        doc_schema: None,
        guests: vec![],
    };
    let mut named_admission = SourceAdmission::default();
    let mut named_codec =
        FoundationCodec::new(registry, &empty, &mut named_admission).map_err(err)?;
    let failure = op::execute(&named, registry, &mut named_codec, &mut budget()).map_err(err)?;
    assert_eq!(
        failure,
        MathPrintResult::Invalid {
            failure: MathPrintFailure::UnprintableName { node: node as u64 }
        }
    );
    let raw_failure =
        wire::result_to_value(&failure, registry, &mut named_codec, &mut budget()).map_err(err)?;
    assert_eq!(
        wire::result_from_value(&raw_failure, registry, &mut named_codec, &mut budget())
            .map_err(err)?,
        failure
    );
    for (reason, amount) in [
        (StopReason::WorkLimit, used.work),
        (StopReason::AllocationLimit, used.allocation_units),
        (StopReason::NodeLimit, used.nodes),
        (StopReason::OutputLimit, used.output_bytes),
        (StopReason::DepthLimit, used.depth),
    ] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = amount - 1,
            StopReason::AllocationLimit => limits.allocation_units = amount - 1,
            StopReason::NodeLimit => limits.nodes = amount - 1,
            StopReason::OutputLimit => limits.output_bytes = amount - 1,
            _ => limits.depth = amount - 1,
        }
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
        let mut b = Budget::new(limits);
        let result = op::execute(&request, registry, &mut codec, &mut b).map_err(err)?;
        assert_eq!(result, MathPrintResult::Stopped { reason });
        assert_eq!(b.poll(), Err(reason));
        assert_eq!(b.current_depth(), 0);
        // Transport encoding has its own budget; decoding a stop is data and
        // cannot claim remote usage or stop a receiving operation implicitly.
        let raw =
            wire::result_to_value(&result, registry, &mut codec, &mut budget()).map_err(err)?;
        let mut receive = budget();
        assert_eq!(
            wire::result_from_value(&raw, registry, &mut codec, &mut receive).map_err(err)?,
            result
        );
        assert!(receive.poll().is_ok());
    }
    Ok(())
}

#[test]
fn portable_source_artifacts_recheck_the_requested_input() -> Result<(), String> {
    use nepl3_math_core::portable::{PortableError, printing as portable};
    let compiled = compiled()?;
    let registry = &compiled.doc.registry;
    let empty = SourceStore::default();
    for (source, entry, expected) in [
        ("frac 1 0", "Expr", "frac 1 0"),
        ("row cons 1 cons -2 nil", "Row", "row cons 1 cons -2 nil"),
        ("Doc \"note\"", "DocGuest", "Doc \"note\""),
    ] {
        let value = lower_value(&compiled, source, entry)?;
        let shape = value.validate_shape(&mut budget()).map_err(err)?;
        let mut guest = FixtureGuest {
            expected: value.embeds.first(),
            text: "\"note\"",
            calls: 0,
        };
        let artifact = print::prefix(&shape, &mut guest, &mut budget()).map_err(err)?;
        assert_eq!(artifact.text, expected);
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
        let raw =
            portable::to_value(&artifact, registry, &mut codec, &mut budget()).map_err(err)?;
        let bytes = nepl3_wire::encode(&raw, &mut budget()).map_err(err)?;
        let raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
        assert_eq!(
            portable::from_value(&raw, registry, &mut codec, &mut budget()).map_err(err)?,
            artifact
        );
        let mut full = budget();
        assert_eq!(
            portable::verify(&raw, &shape, &mut guest, registry, &mut codec, &mut full)
                .map_err(err)?,
            artifact
        );
        for mutate_entry in [false, true] {
            let mut forged = artifact.clone();
            if mutate_entry {
                forged.entry = if forged.entry == MathCategory::Expr {
                    MathCategory::Row
                } else {
                    MathCategory::Expr
                };
            } else {
                forged.text.push(' ');
            }
            let forged =
                portable::to_value(&forged, registry, &mut codec, &mut budget()).map_err(err)?;
            assert!(matches!(
                portable::verify(
                    &forged,
                    &shape,
                    &mut guest,
                    registry,
                    &mut codec,
                    &mut budget()
                ),
                Err(portable::Error::Mismatch)
            ));
        }
        let before = guest.calls;
        assert!(matches!(
            portable::verify(
                &NdfValue::Bool(false),
                &shape,
                &mut guest,
                registry,
                &mut codec,
                &mut budget()
            ),
            Err(portable::Error::Boundary(PortableError::Schema(_)))
        ));
        assert_eq!(guest.calls, before);
        let mut limits = budget().limits();
        limits.work = full.usage().work - 1;
        let mut stopped = Budget::new(limits);
        assert!(
            portable::verify(&raw, &shape, &mut guest, registry, &mut codec, &mut stopped).is_err()
        );
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            portable::verify(
                &raw,
                &shape,
                &mut guest,
                registry,
                &mut codec,
                &mut cancelled
            ),
            Err(portable::Error::Boundary(PortableError::Stopped(
                StopReason::Cancelled
            )))
        ));
    }
    Ok(())
}

#[test]
fn doc_guest_depths_follow_deepest_shared_occurrence() -> Result<(), String> {
    use nepl3_doc_core::{
        model::{DocKind, DocNode, InlineRef},
        print::guest_depths,
    };
    let compiled = compiled()?;
    let registry = &compiled.doc.registry;
    let math = lower_value(
        &compiled,
        "label x Doc sentence cons math Math x nil",
        "Expr",
    )?;
    let closure = &math.embeds[0];
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
    let checked = closure
        .syntax
        .bundle
        .validate_with_sources(registry, &mut budget(), &mut SourceAdmission::default())
        .map_err(err)?;
    let mut document = nepl3_doc_core::lower::document(
        &checked,
        &compiled.doc.package.schema,
        nepl3_doc_core::check::Category::Sentence,
        registry,
        &mut budget(),
        &mut codec,
    )
    .map_err(err)?;
    let root = match document.value.root {
        nepl3_doc_core::model::DocRoot::Sentence(id) => id.0 as usize,
        _ => return Err("sentence".into()),
    };
    let guest = match &document.value.nodes[root].kind {
        DocKind::Sentence { inlines } => inlines[0],
        _ => return Err("sentence node".into()),
    };
    let wrapper = document.value.nodes.len() as u64;
    document.value.nodes.push(DocNode {
        kind: DocKind::Strong { inline: guest },
        locations: vec![],
        origin: None,
        span: None,
    });
    for inlines in [
        vec![guest, InlineRef(wrapper)],
        vec![InlineRef(wrapper), guest],
    ] {
        document.value.nodes[root].kind = DocKind::Sentence { inlines };
        let shape = document.value.validate_shape(&mut budget()).map_err(err)?;
        let mut b = budget();
        assert_eq!(guest_depths(&shape, &mut b).map_err(err)?, vec![3]);
        let used = b.usage();
        let mut nested = budget();
        let depths = nested
            .with_depth_at_least(7, |b| guest_depths(&shape, b))
            .map_err(err)?;
        assert_eq!(depths, vec![3]);
        assert_eq!(nested.usage().depth, 10);
        assert_eq!(nested.current_depth(), 0);
        for reason in [
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::DepthLimit,
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = used.work - 1,
                StopReason::AllocationLimit => limits.allocation_units = used.allocation_units - 1,
                _ => limits.depth = 2,
            }
            let mut b = Budget::new(limits);
            assert_eq!(guest_depths(&shape, &mut b), Err(reason));
            assert_eq!(b.poll(), Err(reason));
        }
    }
    Ok(())
}

#[test]
fn selected_math_doc_printers_compose_without_evaluation() -> Result<(), String> {
    let compiled = compiled()?;
    let registry = &compiled.doc.registry;
    let empty = SourceStore::default();
    for (source, expected) in [
        (
            "label x Doc sentence cons math Math add 1 2 nil",
            "label symbol \"x\" Doc sentence cons math Math add 1 2 nil",
        ),
        (
            "label x Doc sentence cons math Math label frac 1 0 Doc \"[漢字/かんじ]\" nil",
            "label symbol \"x\" Doc sentence cons math Math label frac 1 0 Doc sentence cons ruby text \"漢字\" text \"かんじ\" nil nil",
        ),
    ] {
        let value = lower_value(&compiled, source, "Expr")?;
        let shape = value.validate_shape(&mut budget()).map_err(err)?;
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
        let mut host = nepl3_tools::doc::printing::DocGuestPrinter {
            registry,
            surface: &compiled.doc.package.schema,
            math_surface: Some(&compiled.others[0].schema),
            codec: &mut codec,
        };
        let mut full = budget();
        let artifact = print::prefix(&shape, &mut host, &mut full).map_err(err)?;
        // Expected source follows the declared constructors, not evaluation:
        // addition and division by zero must remain notation at every depth.
        assert_eq!(artifact.text, expected);
        let reparsed = lower_value(&compiled, &artifact.text, "Expr")?;
        assert_eq!(notation(&value), notation(&reparsed));
        let used = full.usage();
        let mut wrong_surface = compiled.others[0].schema.clone();
        wrong_surface.digest.0[0] ^= 1;
        let mut wrong_host = nepl3_tools::doc::printing::DocGuestPrinter {
            registry,
            surface: &compiled.doc.package.schema,
            math_surface: Some(&wrong_surface),
            codec: &mut codec,
        };
        assert!(matches!(
            print::prefix(&shape, &mut wrong_host, &mut budget()),
            Err(print::PrintError::Guest {
                error: nepl3_tools::doc::printing::Error::Selection,
                ..
            })
        ));
        let mut host = nepl3_tools::doc::printing::DocGuestPrinter {
            registry,
            surface: &compiled.doc.package.schema,
            math_surface: Some(&compiled.others[0].schema),
            codec: &mut codec,
        };
        let mut limited = budget();
        let original_limits = limited.limits();
        let result = limited.with_depth_at_least(64, |b| print::prefix(&shape, &mut host, b));
        assert!(matches!(
            result,
            Err(print::PrintError::Stopped(StopReason::DepthLimit))
        ));
        assert_eq!(limited.current_depth(), 0);
        assert_eq!(limited.limits(), original_limits);
        assert_eq!(limited.poll(), Err(StopReason::DepthLimit));
        for (reason, amount) in [
            (StopReason::WorkLimit, used.work),
            (StopReason::AllocationLimit, used.allocation_units),
            (StopReason::OutputLimit, used.output_bytes),
            (StopReason::NodeLimit, used.nodes),
            (StopReason::DepthLimit, used.depth),
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = amount - 1,
                StopReason::AllocationLimit => limits.allocation_units = amount - 1,
                StopReason::OutputLimit => limits.output_bytes = amount - 1,
                StopReason::NodeLimit => limits.nodes = amount - 1,
                _ => limits.depth = amount - 1,
            }
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
            let mut host = nepl3_tools::doc::printing::DocGuestPrinter {
                registry,
                surface: &compiled.doc.package.schema,
                math_surface: Some(&compiled.others[0].schema),
                codec: &mut codec,
            };
            let mut b = Budget::new(limits);
            assert!(matches!(print::prefix(&shape, &mut host, &mut b),
                Err(print::PrintError::Stopped(actual)) if actual == reason));
            assert_eq!(b.poll(), Err(reason));
            assert_eq!(b.current_depth(), 0);
        }
    }
    Ok(())
}

#[test]
fn production_doc_guest_printer_preserves_annotation_semantics() -> Result<(), String> {
    use nepl3_math_core::print::GuestPrinter;
    let compiled = compiled()?;
    let registry = &compiled.doc.registry;
    let empty = SourceStore::default();
    for source in [
        "label x Doc \"[漢字/かんじ]{語/word}\"",
        "label frac 1 0 Doc sentence cons ruby text \"漢字\" text \"かんじ\" cons break cons anno text \"語\" cons text \"word\" nil nil",
        "label x Doc sentence cons strong text \"a\\n b\" cons code \"x < y\" nil",
    ] {
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
        let value = lower_value(&compiled, source, "Expr")?;
        let shape = value.validate_shape(&mut budget()).map_err(err)?;
        let mut host = nepl3_tools::doc::printing::DocGuestPrinter {
            registry,
            surface: &compiled.doc.package.schema,
            math_surface: None,
            codec: &mut codec,
        };
        let printed = print::prefix(&shape, &mut host, &mut budget()).map_err(err)?;
        assert!(printed.text.contains("Doc sentence"));
        let actual = lower_value(&compiled, &printed.text, "Expr")?;
        assert_eq!(notation(&value), notation(&actual));
        let mut documents = Vec::new();
        for closure in [&value.embeds[0], &actual.embeds[0]] {
            // These test parses assign the same local source name to different
            // revisions; compare their meanings in separate admission contexts.
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
            let checked = closure
                .syntax
                .bundle
                .validate_with_sources(registry, &mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let doc = nepl3_doc_core::lower::document(
                &checked,
                &compiled.doc.package.schema,
                nepl3_doc_core::check::Category::Sentence,
                registry,
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            documents.push(doc);
        }
        assert_eq!(documents[0].value.root, documents[1].value.root);
        let kinds = |doc: &nepl3_doc_core::model::DocumentSyntax| {
            doc.value
                .nodes
                .iter()
                .map(|n| n.kind.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(kinds(&documents[0]), kinds(&documents[1]), "{source}");
        let mut wrong = value.embeds[0].clone();
        wrong.syntax.category = "Body".into();
        let mut host = nepl3_tools::doc::printing::DocGuestPrinter {
            registry,
            surface: &compiled.doc.package.schema,
            math_surface: None,
            codec: &mut codec,
        };
        assert!(matches!(
            host.print(&wrong, &mut budget()),
            Err(nepl3_tools::doc::printing::Error::Selection)
        ));
        let mut stopped = budget();
        stopped.cancel();
        assert!(matches!(
            host.print(&value.embeds[0], &mut stopped),
            Err(nepl3_tools::doc::printing::Error::Stopped(
                StopReason::Cancelled
            ))
        ));
    }
    let nested = lower_value(
        &compiled,
        "label x Doc sentence cons math Math add 1 2 nil",
        "Expr",
    )?;
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
    let mut host = nepl3_tools::doc::printing::DocGuestPrinter {
        registry,
        surface: &compiled.doc.package.schema,
        math_surface: None,
        codec: &mut codec,
    };
    assert!(matches!(
        host.print(&nested.embeds[0], &mut budget()),
        Err(nepl3_tools::doc::printing::Error::Print(
            nepl3_doc_core::print::PrintFailure::MissingBinding { .. }
        ))
    ));
    Ok(())
}
#[test]
fn prefix_print_reparses_every_math_constructor_without_evaluation() -> Result<(), String> {
    let compiled = compiled()?;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/fixtures/math/lower.json"
    ))
    .map_err(err)?;
    let cases = fixture["cases"].as_array().ok_or("cases")?;
    for case in cases {
        let source = case["source"].as_str().ok_or("source")?;
        let entry = case["entry"].as_str().ok_or("entry")?;
        let value = lower_value(&compiled, source, entry)?;
        let shape = value.validate_shape(&mut budget()).map_err(err)?;
        // Explicit test host supplies the fixture's known Doc Sentence. This is
        // not a production fallback that trusts retained source spans.
        let mut guest = FixtureGuest {
            expected: value.embeds.first(),
            text: if entry == "DocGuest" {
                "\"[x/ex]\""
            } else {
                "\"note\""
            },
            calls: 0,
        };
        let printed = print::prefix(&shape, &mut guest, &mut budget()).map_err(err)?;
        assert_eq!(guest.calls, value.embeds.len());
        assert_eq!(
            printed.entry,
            match entry {
                "Expr" => Category::Expr,
                "Row" => Category::Row,
                _ => Category::DocGuest,
            }
        );
        let reparsed = lower_value(&compiled, &printed.text, entry)
            .map_err(|e| format!("{source} -> {}: {e}", printed.text))?;
        assert_eq!(
            notation(&value),
            notation(&reparsed),
            "{source} -> {}",
            printed.text
        );
        assert_eq!(value.root, reparsed.root);
        assert_eq!(value.embeds.len(), reparsed.embeds.len());
    }
    assert_eq!(cases.len(), 31);
    for source in [
        "frac 1 0",
        "symbol \"add\"",
        "text \"a\\n\\t\\\\\\\"日本語\"",
        "let x 1 add x x",
        "-0.125",
    ] {
        let value = lower_value(&compiled, source, "Expr")?;
        let shape = value.validate_shape(&mut budget()).map_err(err)?;
        let mut guest = FixtureGuest {
            expected: None,
            text: "",
            calls: 0,
        };
        let artifact = print::prefix(&shape, &mut guest, &mut budget()).map_err(err)?;
        let actual = lower_value(&compiled, &artifact.text, "Expr")?;
        assert_eq!(notation(&actual), notation(&value));
    }
    Ok(())
}

#[test]
fn prefix_print_preserves_limits_and_rejects_unprintable_host_inputs() -> Result<(), String> {
    let compiled = compiled()?;
    let mut previous = 0;
    for width in [128, 256, 512] {
        let shared = MathValue {
            root: MathRoot::Expr(ExprRef(0)),
            nodes: vec![
                MathNode {
                    kind: MathKind::Vector {
                        values: vec![ExprRef(1); width],
                    },
                    origin: None,
                    span: None,
                    locations: vec![],
                },
                MathNode {
                    kind: MathKind::Symbol { name: "x".into() },
                    origin: None,
                    span: None,
                    locations: vec![],
                },
            ],
            embeds: vec![],
        };
        let shape = shared.validate_shape(&mut budget()).map_err(err)?;
        let mut host = FixtureGuest {
            expected: None,
            text: "",
            calls: 0,
        };
        let mut b = budget();
        let printed = print::prefix(&shape, &mut host, &mut b).map_err(err)?;
        assert_eq!(printed.text.matches("symbol \"x\"").count(), width);
        assert_eq!(b.usage().nodes, width as u64 + 1);
        assert_eq!(b.usage().depth, 2);
        if previous > 0 {
            assert!(b.usage().work < previous * 3);
        }
        previous = b.usage().work;
    }
    let value = lower_value(
        &compiled,
        "let x 1.25 vector cons x cons frac 1 0 nil",
        "Expr",
    )?;
    let shape = value.validate_shape(&mut budget()).map_err(err)?;
    let mut guest = FixtureGuest {
        expected: None,
        text: "",
        calls: 0,
    };
    let mut full = budget();
    let artifact = print::prefix(&shape, &mut guest, &mut full).map_err(err)?;
    assert_eq!(full.usage().output_bytes, artifact.text.len() as u64);
    let used = full.usage();
    for (reason, amount) in [
        (StopReason::WorkLimit, used.work),
        (StopReason::AllocationLimit, used.allocation_units),
        (StopReason::OutputLimit, used.output_bytes),
        (StopReason::NodeLimit, used.nodes),
        (StopReason::DepthLimit, used.depth),
    ] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = amount - 1,
            StopReason::AllocationLimit => limits.allocation_units = amount - 1,
            StopReason::OutputLimit => limits.output_bytes = amount - 1,
            StopReason::NodeLimit => limits.nodes = amount - 1,
            _ => limits.depth = amount - 1,
        }
        let mut b = Budget::new(limits);
        assert!(
            matches!(print::prefix(&shape,&mut guest,&mut b),Err(print::PrintError::Stopped(actual)) if actual==reason)
        );
        assert_eq!(b.poll(), Err(reason));
    }
    let mut nested = budget();
    nested
        .with_depth_at_least(7, |b| print::prefix(&shape, &mut guest, b))
        .map_err(err)?;
    assert_eq!(nested.usage().depth, used.depth + 7);
    let mut invalid = value.clone();
    let MathRoot::Expr(root) = invalid.root else {
        return Err("root".into());
    };
    let MathKind::Let { name, .. } = &mut invalid.nodes[root.0 as usize].kind else {
        return Err("let".into());
    };
    *name = "bad name".into();
    let shape = invalid.validate_shape(&mut budget()).map_err(err)?;
    assert!(matches!(
        print::prefix(&shape, &mut guest, &mut budget()),
        Err(print::PrintError::UnprintableName { .. })
    ));
    let doc = lower_value(&compiled, "Doc \"note\"", "DocGuest")?;
    let shape = doc.validate_shape(&mut budget()).map_err(err)?;
    struct DepthGuest {
        seen: u64,
        fail: bool,
    }
    impl GuestPrinter for DepthGuest {
        type Error = String;
        fn print(
            &mut self,
            _: &nepl3_core::syntax::ForeignClosure,
            b: &mut Budget,
        ) -> Result<String, String> {
            self.seen = b.current_depth();
            b.observe_depth(2).map_err(err)?;
            if self.fail {
                Err("host failure".into())
            } else {
                Ok("\"note\"".into())
            }
        }
    }
    for fail in [false, true] {
        let mut host = DepthGuest { seen: 0, fail };
        let mut b = budget();
        let result = b.with_depth_at_least(7, |b| print::prefix(&shape, &mut host, b));
        assert_eq!(host.seen, 8);
        assert_eq!(b.usage().depth, 10);
        assert_eq!(b.current_depth(), 0);
        if fail {
            assert!(matches!(result, Err(print::PrintError::Guest { .. })));
        } else {
            result.map_err(err)?;
        }
    }
    let mut host = DepthGuest {
        seen: 0,
        fail: false,
    };
    let mut limits = budget().limits();
    limits.depth = 9;
    let mut b = Budget::new(limits);
    assert!(matches!(
        b.with_depth_at_least(7, |b| print::prefix(&shape, &mut host, b)),
        Err(print::PrintError::Stopped(StopReason::DepthLimit))
    ));
    assert_eq!(b.current_depth(), 0);
    assert_eq!(b.poll(), Err(StopReason::DepthLimit));
    assert!(matches!(
        print::prefix(&shape, &mut guest, &mut budget()),
        Err(print::PrintError::Guest { .. })
    ));
    let mut empty = FixtureGuest {
        expected: doc.embeds.first(),
        text: "  ",
        calls: 0,
    };
    assert!(matches!(
        print::prefix(&shape, &mut empty, &mut budget()),
        Err(print::PrintError::EmptyGuest { .. })
    ));
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(matches!(
        print::prefix(&shape, &mut empty, &mut cancelled),
        Err(print::PrintError::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}
