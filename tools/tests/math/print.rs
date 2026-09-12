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
            .map_err(err)?
            .value,
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
