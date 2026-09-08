use super::*;
use nepl3_core::budget::Budget;
use nepl3_engine::portable::{head::*, tree};

fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

pub(super) fn answer(
    call: &HeadCall,
    profile: &ResolvedParseProfile<'_>,
    chosen: &HeadShape,
    sources: &SourceStore,
    parent: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<HeadReply, String> {
    let reserve = HeadTransportReserve {
        work: 200_000,
        nodes: 20_000,
        allocation_units: 2_000_000,
        output_bytes: 200_000,
        diagnostics: 10,
        events: 10,
    };
    let mut issued = IssuedHeadDelegation::issue(
        call,
        profile,
        &[sources.snapshots()],
        reserve,
        parent,
        admission,
    )
    .map_err(error)?;
    let empty = SourceStore::default();
    let mut host_codec =
        FoundationCodec::new(profile.registry(), &empty, admission).map_err(error)?;
    let value = issued.to_value(&mut host_codec).map_err(error)?;
    let bytes = issued
        .transport(|b| nepl3_wire::encode(&value, b))
        .map_err(error)?;
    drop(value);
    // This same-process host knows the exact issued grant. An actual transport
    // must authenticate the dispatch and constrain untrusted header Limits
    // before allocating the receiver's budget; raw decode grants no authority.
    let authenticated_limits = issued.limits();
    let mut framing = Budget::new(authenticated_limits);
    let received_value = nepl3_wire::decode(&bytes, &mut framing).map_err(error)?;
    let mut remote_admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(profile.registry(), &empty, &mut remote_admission).map_err(error)?;
    let limits = delegation_limits(
        &received_value,
        profile.registry(),
        &mut codec,
        &mut framing,
    )
    .map_err(error)?;
    assert_eq!(limits, authenticated_limits);
    let mut remote = Budget::new(receiver_limits(limits, framing.usage()).map_err(error)?);
    // Receiver only has the selected Profile, packet, and empty source store.
    // It is not passed the sender's native HeadCall or SourceSnapshot table.
    let received =
        delegation_decode(&received_value, profile, &mut codec, &mut remote).map_err(error)?;
    let reply = super::answer(&received.call, profile, chosen, &mut remote).map_err(error)?;
    let value = delivery_to_value(
        &reply,
        &received.call,
        profile,
        &mut codec,
        &mut remote,
        framing.usage(),
    )
    .map_err(error)?;
    let bytes = nepl3_wire::encode(&value, &mut remote).map_err(error)?;
    let observed = metered_usage(framing.usage(), remote.usage()).map_err(error)?;
    // The host already knows the terminated child's actual cost. Record it
    // before decoding an untrusted delivery, so a malformed packet neither
    // loses that work nor cancels a still-retryable parser pending slot.
    issued.settle(observed).map_err(error)?;
    let value = issued
        .transport(|b| nepl3_wire::decode(&bytes, b))
        .map_err(error)?;
    let mut malformed = value.clone(); // Adversarial test input, outside the codec.
    let NdfValue::Record(delivery) = &mut malformed else {
        return Err("delivery record".into());
    };
    let NdfValue::Record(reply) = &mut delivery.fields[0] else {
        return Err("reply record".into());
    };
    let NdfValue::Record(identity) = &mut reply.fields[0] else {
        return Err("identity record".into());
    };
    identity.fields[1] = NdfValue::U64(call.identity.call_id + 1);
    assert!(delivery_decode(&malformed, &mut issued, &mut host_codec).is_err());
    issued.transport(|b| b.poll()).map_err(error)?;
    // The same issued call and ParseSession Await remain available. A good
    // packet can now be validated without charging the child observation twice.
    let delivery = delivery_decode(&value, &mut issued, &mut host_codec).map_err(error)?;
    issued.accept(delivery, observed).map_err(error)
}

pub(super) fn tree_roundtrip(
    value: &nepl3_engine::recovery::ParseTree,
    profile: &ResolvedParseProfile<'_>,
) -> Result<(), String> {
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(error)?;
    let mut b = budget();
    let encoded = tree::to_value(value, profile, &mut codec, &mut b).map_err(error)?;
    let bytes = nepl3_wire::encode(&encoded, &mut b).map_err(error)?;
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(error)?;
    let raw = nepl3_wire::decode(&bytes, &mut b).map_err(error)?;
    let decoded = tree::from_value(&raw, profile, &mut codec, &mut b).map_err(error)?;
    assert_eq!(
        tree::to_value(&decoded, profile, &mut codec, &mut b).map_err(error)?,
        encoded
    );
    let selection = decoded
        .contexts
        .iter()
        .flat_map(|c| &c.nodes)
        .find(|n| matches!(n.shape, ShapeSelection::Dynamic { .. }))
        .ok_or("dynamic selection")?;
    let ShapeSelection::Dynamic {
        child_contexts,
        shape,
        ..
    } = &selection.shape
    else {
        return Err("dynamic shape".into());
    };
    assert_eq!(child_contexts.len(), shape.fields.len());
    assert_eq!(child_contexts[1].mode, "Alt");
    Ok(())
}

#[test]
fn portable_callback_and_dynamic_tree_match_owned_native_execution() -> TestResult {
    let input = "choose alt @let z x tail";
    let native = run_case(input, Case::Owned, false)?;
    let portable = run_case(input, Case::Portable, false)?;
    let ParseOutcome::Complete {
        tree: native,
        cursor: native_cursor,
        ..
    } = native.outcome
    else {
        return Err("native Complete".into());
    };
    let ParseOutcome::Complete {
        tree: portable,
        cursor: portable_cursor,
        ..
    } = portable.outcome
    else {
        return Err("portable Complete".into());
    };
    assert_eq!(native_cursor, 19);
    assert_eq!(portable_cursor, native_cursor);
    assert_eq!(portable, native);
    Ok(())
}
