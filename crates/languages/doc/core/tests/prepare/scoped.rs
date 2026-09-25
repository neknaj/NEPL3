use super::*;
use nepl3_doc_core::check::{RegistryValidatedDocumentSyntax, StructureError};

#[test]
fn retained_document_checks_receiver_limits_and_registry() -> Result<(), String> {
    let r = registry()?;
    let mut d = document(&r)?;
    let source = |name: &str, bytes: &[u8]| {
        SourceSnapshot::new(
            SourceId(name.into()),
            1,
            format!("memory:{name}"),
            bytes.to_vec(),
            &mut b(),
        )
        .map_err(err)
    };
    d.sources.push(source("doc-only", b"document bytes")?);
    let DocContent::Syntax { closure } = &mut d.value.embeds[0].content else {
        return Err("syntax fixture".into());
    };
    closure.provenance = nepl3_core::syntax::OwnerProvenance::from_parts(
        vec![],
        vec![source("owner-only", b"owner bytes")?],
        vec![],
    );
    closure
        .syntax
        .bundle
        .sources
        .push(source("declared-only", b"unused guest declaration")?);
    let mut creation = b();
    // An unrelated earlier high-water mark must not inflate the saved depth.
    creation.observe_depth(9000).map_err(err)?;
    let proof = RegistryValidatedDocumentSyntax::new(
        &d,
        &r,
        &mut creation,
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert_eq!(proof.syntax().len(), 1);
    assert!(core::ptr::eq(proof.structure().document(), &d));
    let mut full = b();
    d.validate_structure(&r, &mut full, &mut SourceAdmission::default())
        .map_err(err)?;
    let mut reused = b();
    proof
        .validate_for(&r, &mut reused, &mut SourceAdmission::default())
        .map_err(err)?;
    assert_eq!(reused.usage().depth, full.usage().depth);
    assert_eq!(reused.usage().source_bytes, full.usage().source_bytes);
    assert!(reused.usage().work < full.usage().work);
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::SourceLimit,
        StopReason::DepthLimit,
    ] {
        for below in [false, true] {
            let mut limits = b().limits();
            let usage = reused.usage();
            let field = match reason {
                StopReason::WorkLimit => (&mut limits.work, usage.work),
                StopReason::AllocationLimit => {
                    (&mut limits.allocation_units, usage.allocation_units)
                }
                StopReason::SourceLimit => (&mut limits.source_bytes, usage.source_bytes),
                StopReason::DepthLimit => (&mut limits.depth, usage.depth),
                _ => unreachable!(),
            };
            *field.0 = field
                .1
                .checked_sub(u64::from(below))
                .ok_or("nonzero boundary")?;
            let mut receiver = Budget::new(limits);
            let result = proof.validate_for(&r, &mut receiver, &mut SourceAdmission::default());
            if below {
                assert_eq!(result, Err(StructureError::Stopped(reason)));
                assert_eq!(receiver.poll(), Err(reason));
            } else {
                result.map_err(err)?;
            }
        }
    }
    let other = registry()?;
    let mut received = b();
    proof
        .validate_for(&other, &mut received, &mut SourceAdmission::default())
        .map_err(err)?;
    assert_eq!(received.usage().work, full.usage().work + 1);
    let mut missing = SchemaRegistry::default();
    missing.finalize(&mut b()).map_err(err)?;
    assert!(
        proof
            .validate_for(&missing, &mut b(), &mut SourceAdmission::default())
            .is_err()
    );
    let mut nested = b();
    nested
        .with_depth_at_least(7, |b| {
            proof.validate_for(&r, b, &mut SourceAdmission::default())
        })
        .map_err(err)?;
    assert_eq!(nested.usage().depth, full.usage().depth + 7);
    assert_eq!(nested.current_depth(), 0);
    // A fresh receiver can already own a conflicting identity. Stored proof
    // does not authorize replacing that operation's admitted source contents.
    let mut receiver = b();
    let mut admission = SourceAdmission::default();
    admission
        .admit_existing(&source("owner-only", b"different bytes")?, &mut receiver)
        .map_err(err)?;
    assert!(matches!(
        proof.validate_for(&r, &mut receiver, &mut admission),
        Err(StructureError::Source(
            nepl3_core::source::SourceError::IdentityConflict
        ))
    ));
    Ok(())
}

#[test]
fn retained_document_encoding_preserves_value_and_receiving_admission() -> Result<(), String> {
    let r = registry()?;
    let d = document(&r)?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let expected = portable::to_value(&d, &r, &mut codec, &mut b()).map_err(err)?;
    for limited in [false, true] {
        let proof =
            RegistryValidatedDocumentSyntax::new(&d, &r, &mut b(), &mut SourceAdmission::default())
                .map_err(err)?;
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let mut limits = b().limits();
        if limited {
            limits.source_bytes = 0;
        }
        let mut receiver = Budget::new(limits);
        let result = portable::to_value_validated(proof, &mut codec, &mut receiver);
        if limited {
            assert_eq!(
                result,
                Err(portable::PortableError::Stopped(StopReason::SourceLimit))
            );
        } else {
            assert_eq!(result.map_err(err)?, expected);
        }
    }
    Ok(())
}
