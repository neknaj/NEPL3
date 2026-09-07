use super::*;
use nepl3_core::schema::{TypeDescriptor, TypeRef};
fn typed(name: &str) -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: "nepl3.engine".into(),
        revision: 1,
        name: name.into(),
    })
}
pub(super) fn boundaries(
    reply: &RenameReply,
    request: &RenameRequest,
    packet: &NdfValue,
    r: &nepl3_core::schema::SchemaRegistry,
) -> Result<(), String> {
    for mutation in 0..6 {
        let mut changed = packet.clone();
        let NdfValue::Record(record) = &mut changed else {
            return Err("reply".into());
        };
        if mutation == 0 {
            record.fields[3] = NdfValue::List(vec![]);
        } else {
            let NdfValue::Variant(outcome) = &mut record.fields[1] else {
                return Err("outcome".into());
            };
            if mutation == 1 {
                let NdfValue::Record(key) = &mut outcome.fields[0] else {
                    return Err("key".into());
                };
                key.fields[1] = NdfValue::Bytes(vec![0; 32]);
            } else {
                let NdfValue::List(edits) = &mut outcome.fields[1] else {
                    return Err("edits".into());
                };
                if mutation == 4 {
                    edits.push(edits[0].clone());
                } else if mutation == 5 {
                    edits.clear();
                } else {
                    let NdfValue::Record(edit) = &mut edits[0] else {
                        return Err("edit".into());
                    };
                    edit.fields[if mutation == 2 { 1 } else { 2 }] = if mutation == 2 {
                        NdfValue::Bytes(vec![0; 32])
                    } else {
                        NdfValue::Text("forged".into())
                    };
                }
            }
        }
        r.validate(&typed("RenameReply"), &changed, &mut budget())
            .map_err(err)?;
        let mut ambient = SourceStore::default();
        for source in &reply.sources {
            ambient.insert(source.clone()).map_err(err)?;
        }
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(r, &ambient, &mut a).map_err(err)?;
        assert!(
            wire_rename::reply_decode(&changed, request, r, &mut c, &mut budget()).is_err(),
            "rename mutation {mutation}"
        );
        assert_eq!(
            wire_rename::reply_decode(packet, request, r, &mut c, &mut budget()).map_err(err)?,
            *reply
        );
    }
    let empty = SourceStore::default();
    for resource in 0..6 {
        let mut limits = budget().limits();
        let reason = match resource {
            0 => {
                limits.work = 0;
                StopReason::WorkLimit
            }
            1 => {
                limits.source_bytes = 0;
                StopReason::SourceLimit
            }
            2 => {
                limits.nodes = 0;
                StopReason::NodeLimit
            }
            3 => {
                limits.allocation_units = 0;
                StopReason::AllocationLimit
            }
            4 => {
                limits.depth = 0;
                StopReason::DepthLimit
            }
            _ => StopReason::Cancelled,
        };
        let mut b = Budget::new(limits);
        if resource == 5 {
            b.cancel();
        }
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
        assert!(
            matches!(wire_rename::reply_decode(packet,request,r,&mut c,&mut b),Err(nepl3_engine::portable::PortableError::Stopped(v)) if v==reason)
        );
        assert_eq!(b.poll(), Err(reason));
    }
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
    let mut value =
        wire_rename::request_to_value(request, r, &mut c, &mut budget()).map_err(err)?;
    let NdfValue::Record(record) = &mut value else {
        return Err("request".into());
    };
    let NdfValue::List(writable) = &mut record.fields[4] else {
        return Err("writable".into());
    };
    writable.push(writable[0].clone());
    r.validate(&typed("RenameRequest"), &value, &mut budget())
        .map_err(err)?;
    assert!(wire_rename::request_decode(&value, r, &mut c, &mut budget()).is_err());
    let mut invalid = reply.clone();
    invalid.sources.clear();
    invalid.outcome = RenameOutcome::Invalid(RenameError::Source(
        nepl3_core::source::SourceError::Stopped(StopReason::WorkLimit),
    ));
    assert!(wire_rename::reply_to_value(&invalid, request, r, &mut c, &mut budget()).is_err());
    Ok(())
}
