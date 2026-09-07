use super::*;
use nepl3_core::value_codec::FoundationValueCodec;
use nepl3_core::{
    budget::{StopReason, Usage},
    diagnostic::*,
    facts::*,
};
use nepl3_engine::facts::{FactsReply, FactsRequest};
use nepl3_engine::portable::facts::*;

fn request(tree: ParseTree, schema: &SchemaRef) -> Result<FactsRequest, String> {
    let origin = Origin::Direct(tree.bundle.sources[0].span(0, 1).map_err(err)?);
    let existing = FactSet {
        analysis_id: "analysis".into(),
        namespaces: vec![FactNamespace {
            schema: schema.clone(),
            name: "Value".into(),
            policy: nepl3_core::facts::NamespacePolicy::Lexical,
            root: ScopeId(42),
        }],
        scopes: vec![Scope {
            id: ScopeId(42),
            parent: None,
            origin: Some(OriginId(0)),
        }],
        entities: vec![],
        occurrences: vec![],
        relations: vec![],
        edges: vec![],
        sources: vec![tree.bundle.sources[0].clone()],
        origins: vec![origin],
        source_maps: vec![],
    };
    Ok(FactsRequest {
        tree,
        path: vec![ForeignStep {
            node: NodeRef(1),
            field: "head".into(),
        }],
        node: NodeRef(2),
        existing,
        authority: FactAuthority {
            analysis_id: "analysis".into(),
            current_scope: ScopeId(42),
            namespaces: vec![NamespaceRef(0)],
            writable_scopes: vec![],
            import_scopes: vec![],
            resolution_updates: vec![],
            relation_sources: vec![],
            reservation: FactReservation {
                scopes: IdRange {
                    start: 93,
                    end: 100,
                },
                entities: IdRange {
                    start: 93,
                    end: 100,
                },
                occurrences: IdRange {
                    start: 93,
                    end: 100,
                },
                relations: IdRange {
                    start: 93,
                    end: 100,
                },
            },
        },
    })
}
fn record(v: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
    match v {
        NdfValue::Record(v) => Ok(&mut v.fields),
        _ => Err("record".into()),
    }
}
fn variant(v: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
    match v {
        NdfValue::Variant(v) => Ok(&mut v.fields),
        _ => Err("variant".into()),
    }
}
fn list(v: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
    match v {
        NdfValue::List(v) => Ok(v),
        _ => Err("list".into()),
    }
}
fn expected(name: &str) -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: "nepl3.engine".into(),
        revision: 1,
        name: name.into(),
    })
}

#[test]
fn facts_request_binds_complete_host_authority_and_canonical_target() -> TestResult {
    let (package, registry) = extended()?;
    let profile = profile(&package, &registry)?;
    let packages = [&package];
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &[],
                resources: &[],
            },
            &registry,
            &mut budget(),
        )
        .map_err(err)?;
    let request = request(tree(&package, &registry, &resolved)?, &package.schema)?;
    let empty = SourceStore::default();
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let proof = request
        .issue(&resolved, &mut b, &mut admission)
        .map_err(err)?;
    let mut codec = FoundationCodec::new(&registry, &empty, &mut admission).map_err(err)?;
    let value = request_to_value(&proof, &mut codec, &mut b).map_err(err)?;
    let bytes = nepl3_wire::encode(&value, &mut b).map_err(err)?;
    let wire = nepl3_wire::decode(&bytes, &mut b).map_err(err)?;
    let decoded = request_from_value(&wire, &proof, &mut codec, &mut b).map_err(err)?;
    assert_eq!(decoded.path[0].node, NodeRef(0));
    assert_eq!(decoded.node, NodeRef(0));
    assert_eq!(decoded.existing.scopes[0].id, ScopeId(42));
    assert_eq!(decoded.authority.reservation.entities.start, 93);
    let issued = decoded
        .issue(&resolved, &mut b, codec.source_admission())
        .map_err(err)?;
    assert_eq!(
        request_to_value(&issued, &mut codec, &mut b).map_err(err)?,
        value
    );
    assert_eq!(b.usage().source_bytes, 15);
    for mutation in 0..7 {
        let mut bad = value.clone();
        match mutation {
            0 => record(&mut record(&mut bad)?[2])?[0] = NdfValue::U64(2), // old guest node
            1 => {
                let step = &mut list(&mut record(&mut bad)?[1])?[0];
                record(&mut record(step)?[0])?[0] = NdfValue::U64(1);
            }
            2 => record(&mut record(&mut bad)?[3])?[0] = NdfValue::Text("other analysis".into()),
            3 => record(&mut record(&mut bad)?[4])?[0] = NdfValue::Text("other grant".into()),
            4 => {
                let a = record(&mut record(&mut bad)?[4])?;
                record(&mut record(&mut a[7])?[1])?[1] = NdfValue::U64(101);
            }
            5 | 6 => {
                let a = record(&mut record(&mut bad)?[4])?;
                let current = a[1].clone();
                list(&mut a[if mutation == 5 { 3 } else { 4 }])?.push(current);
            }
            _ => return Err("mutation".into()),
        }
        registry
            .validate(&expected("FactsRequest"), &bad, &mut budget())
            .map_err(err)?;
        assert!(
            matches!(
                request_from_value(&bad, &proof, &mut codec, &mut budget()),
                Err(PortableError::RequestMismatch)
            ),
            "mutation {mutation}"
        );
    }
    Ok(())
}

#[test]
fn facts_first_receiver_decodes_without_the_senders_request_or_source_store() -> TestResult {
    let (package, registry) = extended()?;
    let profile = profile(&package, &registry)?;
    let packages = [&package];
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &[],
                resources: &[],
            },
            &registry,
            &mut budget(),
        )
        .map_err(err)?;
    let bytes = {
        let request = request(tree(&package, &registry, &resolved)?, &package.schema)?;
        let mut b = budget();
        let mut a = SourceAdmission::default();
        let proof = request.issue(&resolved, &mut b, &mut a).map_err(err)?;
        let empty = SourceStore::default();
        let mut codec = FoundationCodec::new(&registry, &empty, &mut a).map_err(err)?;
        nepl3_wire::encode(
            &request_to_value(&proof, &mut codec, &mut b).map_err(err)?,
            &mut b,
        )
        .map_err(err)?
    }; // Sender's request, proof, source store, codec and admission are gone.
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let empty = SourceStore::default();
    let mut codec = FoundationCodec::new(&registry, &empty, &mut a).map_err(err)?;
    let value = nepl3_wire::decode(&bytes, &mut b).map_err(err)?;
    let received = request_decode(&value, &resolved, &mut codec, &mut b).map_err(err)?;
    assert_eq!(received.node, NodeRef(0));
    assert_eq!(received.path[0].node, NodeRef(0));
    assert_eq!(received.existing.scopes[0].id, ScopeId(42));
    assert_eq!(b.usage().source_bytes, 15);
    // Raw decode is data validation, not authorization: a different but valid
    // grant remains raw input for the host's separate transport/policy decision.
    let mut changed = value;
    let a = record(&mut record(&mut changed)?[4])?;
    let current = a[1].clone();
    list(&mut a[4])?.push(current);
    let other = request_decode(&changed, &resolved, &mut codec, &mut budget()).map_err(err)?;
    assert_eq!(other.authority.import_scopes, vec![ScopeId(42)]);
    Ok(())
}

#[test]
fn facts_all_reply_branches_keep_report_only_sources_and_request_relative_grants() -> TestResult {
    let (package, registry) = extended()?;
    let profile = profile(&package, &registry)?;
    let packages = [&package];
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &[],
                resources: &[],
            },
            &registry,
            &mut budget(),
        )
        .map_err(err)?;
    let request = request(tree(&package, &registry, &resolved)?, &package.schema)?;
    let guest = match &request.tree.bundle.nodes[1].fields[0] {
        FieldValue::Foreign(g) => &g.bundle.sources[0],
        _ => return Err("guest".into()),
    };
    let auxiliary = SourceSnapshot::new(
        SourceId("delta".into()),
        0,
        "memory:delta".into(),
        b"d".to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    let reported = SourceSnapshot::new(
        SourceId("reported".into()),
        0,
        "memory:reported".into(),
        b"r".to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    let delta = FactDelta {
        analysis_id: "analysis".into(),
        origin_base: 1,
        scopes: vec![Scope {
            id: ScopeId(93),
            parent: Some(ScopeId(42)),
            origin: Some(OriginId(1)),
        }],
        entities: vec![Entity {
            id: EntityId(93),
            scope: ScopeId(93),
            namespace: NamespaceRef(0),
            name: "d".into(),
            definition: Some(auxiliary.span(0, 1).map_err(err)?),
            selection: Some(auxiliary.span(0, 1).map_err(err)?),
            origin: Some(OriginId(1)),
        }],
        occurrences: vec![],
        relations: vec![],
        edges: vec![],
        resolutions: vec![],
        sources: vec![auxiliary.clone()],
        origins: vec![Origin::Direct(auxiliary.span(0, 1).map_err(err)?)],
        source_maps: vec![],
    };
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?;
    let arguments = TypedValue::Record(Record {
        schema: foundation.clone(),
        kind: "SchemaRef".into(),
        fields: vec![
            NdfValue::Text(package.schema.package.clone()),
            NdfValue::U64(package.schema.revision),
            NdfValue::Bytes(package.schema.digest.0.to_vec()),
        ],
    });
    let report = Report {
        diagnostics: vec![Diagnostic {
            schema: package.schema.clone(),
            code: "FactsValidation".into(),
            severity: Severity::Error,
            stage: "facts".into(),
            arguments: arguments.clone(),
            primary: Some(reported.span(0, 1).map_err(err)?),
            related: vec![Related {
                span: Some(guest.span(0, 3).map_err(err)?),
                code: "Target".into(),
                arguments,
            }],
            fixes: vec![Fix {
                id: "guest".into(),
                edits: vec![TextEdit {
                    span: guest.span(4, 5).map_err(err)?,
                    expected_digest: Digest::of(b"x"),
                    replacement: "X".into(),
                }],
            }],
        }],
        usage: Usage {
            diagnostics: 1,
            ..Usage::default()
        },
        ..Report::default()
    };
    let mapping = Mapping {
        source: guest.span(0, 1).map_err(err)?,
        target: reported.span(0, 1).map_err(err)?,
        kind: MappingKind::Transformed,
    };
    let mut ambient = SourceStore::default();
    ambient.insert(reported.clone()).map_err(err)?;
    for branch in 0..5 {
        let reply = match branch {
            0 => FactsReply::Complete {
                delta: delta.clone(),
                report: report.clone(),
                sources: vec![reported.clone()],
                source_maps: vec![mapping.clone()],
            },
            1 | 2 => FactsReply::Invalid {
                partial: if branch == 1 {
                    None
                } else {
                    Some(delta.clone())
                },
                report: report.clone(),
                sources: vec![reported.clone()],
                source_maps: vec![mapping.clone()],
            },
            _ => FactsReply::Stopped {
                reason: StopReason::WorkLimit,
                partial: if branch == 3 {
                    None
                } else {
                    Some(delta.clone())
                },
                report: report.clone(),
                sources: vec![reported.clone()],
                source_maps: vec![mapping.clone()],
            },
        };
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let proof = request
            .issue(&resolved, &mut b, &mut admission)
            .map_err(err)?;
        reply
            .validate(&proof, &mut b, &mut admission)
            .map_err(err)?;
        let mut codec = FoundationCodec::new(&registry, &ambient, &mut admission).map_err(err)?;
        let value = reply_to_value(&reply, &proof, &mut codec, &mut b).map_err(err)?;
        let bytes = nepl3_wire::encode(&value, &mut b).map_err(err)?;
        let received = nepl3_wire::decode(&bytes, &mut b).map_err(err)?;
        let decoded = reply_from_value(&received, &proof, &mut codec, &mut b).map_err(err)?;
        assert_eq!(
            reply_to_value(&decoded, &proof, &mut codec, &mut b).map_err(err)?,
            value
        );
        assert_eq!(
            b.usage().source_bytes,
            if branch == 1 || branch == 3 { 16 } else { 17 }
        );
        for resource in 0..5 {
            for encode in [true, false] {
                let mut limits = budget().limits();
                let reason = match resource {
                    0 => {
                        limits.source_bytes = 0;
                        StopReason::SourceLimit
                    }
                    1 => {
                        limits.work = 0;
                        StopReason::WorkLimit
                    }
                    2 => {
                        limits.depth = 0;
                        StopReason::DepthLimit
                    }
                    3 => {
                        limits.allocation_units = 0;
                        StopReason::AllocationLimit
                    }
                    _ => StopReason::Cancelled,
                };
                let mut limited = nepl3_core::budget::Budget::new(limits);
                if reason == StopReason::Cancelled {
                    limited.cancel();
                }
                let mut a = SourceAdmission::default();
                let mut c = FoundationCodec::new(&registry, &ambient, &mut a).map_err(err)?;
                let result = if encode {
                    reply_to_value(&reply, &proof, &mut c, &mut limited).map(|_| ())
                } else {
                    reply_from_value(&value, &proof, &mut c, &mut limited).map(|_| ())
                };
                assert!(
                    matches!(result,Err(PortableError::Stopped(r)) if r==reason),
                    "branch {branch} encode {encode}: {result:?}"
                );
            }
        }
        if branch == 0 {
            let mut duplicate = value.clone();
            let sources = list(&mut variant(&mut duplicate)?[2])?;
            sources.push(sources[0].clone());
            registry
                .validate(&expected("FactsReply"), &duplicate, &mut budget())
                .map_err(err)?;
            assert!(reply_from_value(&duplicate, &proof, &mut codec, &mut budget()).is_err());
            let mut forged = value.clone();
            let d = &mut variant(&mut forged)?[0];
            let entity = &mut list(&mut record(d)?[3])?[0];
            record(&mut record(entity)?[0])?[0] = NdfValue::U64(101);
            registry
                .validate(&expected("FactsReply"), &forged, &mut budget())
                .map_err(err)?;
            assert!(reply_from_value(&forged, &proof, &mut codec, &mut budget()).is_err());
        }
        let mut missing = value.clone();
        let fields = variant(&mut missing)?;
        fields[if branch >= 3 { 3 } else { 2 }] = NdfValue::List(vec![]);
        registry
            .validate(&expected("FactsReply"), &missing, &mut budget())
            .map_err(err)?;
        assert!(reply_from_value(&missing, &proof, &mut codec, &mut budget()).is_err());
    }
    // The host reservation remains the authority even if a delta is otherwise a
    // valid standalone facts graph. A provider cannot allocate ID 101 itself.
    let mut bad = delta;
    bad.entities[0].id = EntityId(101);
    let reply = FactsReply::Complete {
        delta: bad,
        report: Report::default(),
        sources: vec![],
        source_maps: vec![],
    };
    let mut admission = SourceAdmission::default();
    let proof = request
        .issue(&resolved, &mut budget(), &mut admission)
        .map_err(err)?;
    assert!(
        reply
            .validate(&proof, &mut budget(), &mut admission)
            .is_err()
    );
    // A report-only table cannot rename an already declared guest source.
    let conflicting = SourceSnapshot::new(
        SourceId("guest".into()),
        0,
        "memory:different-guest".into(),
        b"let x y".to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    let duplicate = FactsReply::Invalid {
        partial: None,
        report: Report::default(),
        sources: vec![reported.clone(), reported.clone()],
        source_maps: vec![],
    };
    assert!(
        duplicate
            .validate(&proof, &mut budget(), &mut SourceAdmission::default())
            .is_err()
    );
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&registry, &ambient, &mut a).map_err(err)?;
    assert!(reply_to_value(&duplicate, &proof, &mut c, &mut budget()).is_err());
    let conflict = FactsReply::Invalid {
        partial: None,
        report: Report::default(),
        sources: vec![conflicting],
        source_maps: vec![],
    };
    assert!(
        conflict
            .validate(&proof, &mut budget(), &mut SourceAdmission::default())
            .is_err()
    );
    Ok(())
}
