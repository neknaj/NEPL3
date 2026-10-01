use super::*;
use nepl3_core::value::NdfValue;
use nepl3_engine::portable::{alternatives, analysis};
use nepl3_wire::foundation::FoundationCodec;

#[test]
fn first_receiver_recomputes_declarations_and_rejects_modified_metadata() -> Result<(), String> {
    for text in ["let", "lettext", "let x", "sequence", "x"] {
        super::super::expected::with_input(
            text,
            |_| Ok(()),
            |input, source, profile| {
                let empty = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut c = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(err)?;
                let request = ExpectedReadRequest {
                    key: input.key(),
                    source: source.reference(),
                    offset: text.len() as u64,
                };
                let reply = declared_alternatives(
                    input,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                let tree = analysis::request_to_value(input, &mut c, &mut budget()).map_err(err)?;
                let request_value = alternatives::request_to_value(
                    &request,
                    profile.registry(),
                    &mut c,
                    &mut budget(),
                )
                .map_err(err)?;
                let reply_value =
                    alternatives::reply_to_value(&reply, &request, input, &mut c, &mut budget())
                        .map_err(err)?;
                let bytes = nepl3_wire::encode(
                    &NdfValue::List(vec![tree, request_value, reply_value]),
                    &mut budget(),
                )
                .map_err(err)?;
                let NdfValue::List(ref packet) =
                    nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?
                else {
                    return Err("packet".into());
                };
                let receiver_empty = SourceStore::default();
                let mut receiver_admission = SourceAdmission::default();
                let mut c = FoundationCodec::new(
                    profile.registry(),
                    &receiver_empty,
                    &mut receiver_admission,
                )
                .map_err(err)?;
                let tree = analysis::request_decode(&packet[0], profile, &mut c, &mut budget())
                    .map_err(err)?;
                let received = analysis::prepare_received(&tree, profile, &mut c, &mut budget())
                    .map_err(err)?;
                let request = alternatives::request_decode(
                    &packet[1],
                    profile.registry(),
                    &mut c,
                    &mut budget(),
                )
                .map_err(err)?;
                assert_eq!(
                    alternatives::reply_decode(
                        &packet[2],
                        &request,
                        &received,
                        &mut c,
                        &mut budget()
                    )
                    .map_err(err)?,
                    reply
                );
                if text == "let x" {
                    for which in 0..5 {
                        let mut forged = packet[2].clone();
                        let NdfValue::Record(reply) = &mut forged else {
                            return Err("reply".into());
                        };
                        let NdfValue::Variant(outcome) = &mut reply.fields[1] else {
                            return Err("outcome".into());
                        };
                        let NdfValue::Some(set) = &mut outcome.fields[0] else {
                            return Err("some".into());
                        };
                        let NdfValue::Record(set) = &mut **set else {
                            return Err("set".into());
                        };
                        let NdfValue::Variant(category) = &mut set.fields[1] else {
                            return Err("category".into());
                        };
                        match which {
                            0 => category.fields[1] = NdfValue::U64(999),
                            1 => category.fields[2] = NdfValue::Bool(true),
                            _ => {
                                let NdfValue::List(forms) = &mut category.fields[0] else {
                                    return Err("forms".into());
                                };
                                if which == 2 {
                                    forms.reverse();
                                } else {
                                    let NdfValue::Record(form) = &mut forms[0] else {
                                        return Err("form".into());
                                    };
                                    if which == 3 {
                                        form.fields[0] = NdfValue::U64(999);
                                    } else {
                                        form.fields[1] = NdfValue::Text("invented".into());
                                    }
                                }
                            }
                        }
                        assert!(
                            alternatives::reply_decode(
                                &forged,
                                &request,
                                &received,
                                &mut c,
                                &mut budget()
                            )
                            .is_err()
                        );
                    }
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn failed_declaration_query_transports_no_partial_metadata() -> Result<(), String> {
    super::super::expected::with_input(
        "あ",
        |_| Ok(()),
        |input, source, profile| {
            let request = ExpectedReadRequest {
                key: input.key(),
                source: source.reference(),
                offset: 1,
            };
            let invalid = declared_alternatives(
                input,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let mut b = budget();
            b.cancel();
            let stopped =
                declared_alternatives(input, &request, &mut b, &mut SourceAdmission::default());
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut c =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            for reply in [invalid, stopped] {
                assert!(reply.sources.is_empty());
                let value =
                    alternatives::reply_to_value(&reply, &request, input, &mut c, &mut budget())
                        .map_err(err)?;
                assert_eq!(
                    alternatives::reply_decode(&value, &request, input, &mut c, &mut budget())
                        .map_err(err)?,
                    reply
                );
                let mut forged = reply.clone();
                forged.sources.push(source.clone());
                assert!(
                    alternatives::reply_to_value(&forged, &request, input, &mut c, &mut budget())
                        .is_err()
                );
            }
            Ok(())
        },
    )
}
