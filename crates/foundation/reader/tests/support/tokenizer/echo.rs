use nepl3_core::{
    budget::Budget,
    source::{SourceReservation, SourceStore},
    value::NdfValue,
    value_codec::FoundationValueCodec,
};
use nepl3_reader::{
    portable::PortableError,
    runtime::ProviderReply,
    tokenizer::{TokenizationReply, TokenizationSession},
};
#[derive(Clone, Copy)]
pub enum Projection {
    Continuation,
    Reply,
}
impl Projection {
    pub fn pending<C: FoundationValueCodec>(
        self,
        session: &TokenizationSession<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        match self {
            Self::Continuation => session.pending_continuation_value(c, b),
            Self::Reply => session.pending_reply_value(c, b),
        }
    }
    pub fn resume<C: FoundationValueCodec>(
        self,
        session: &mut TokenizationSession<'_>,
        echo: &NdfValue,
        reply: ProviderReply,
        sources: &SourceStore,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<TokenizationReply, PortableError<C::Error>> {
        match self {
            Self::Continuation => session.resume_continuation_value(echo, reply, sources, c, b),
            Self::Reply => session.resume_reply_value(echo, reply, sources, c, b),
        }
    }
    pub fn reserve<C: FoundationValueCodec>(
        self,
        session: &mut TokenizationSession<'_>,
        echo: &NdfValue,
        reservation: &SourceReservation,
        sources: &SourceStore,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<TokenizationReply, PortableError<C::Error>> {
        match self {
            Self::Continuation => {
                session.reserve_continuation_value(echo, reservation, sources, c, b)
            }
            Self::Reply => session.reserve_reply_value(echo, reservation, sources, c, b),
        }
    }
    pub fn inner(self, value: &NdfValue) -> Result<&NdfValue, String> {
        match (self, value) {
            (Self::Continuation, v) => Ok(v),
            (Self::Reply, NdfValue::Record(r)) if r.fields.len() == 8 => match &r.fields[0] {
                NdfValue::Variant(v) if v.fields.len() == 2 => Ok(&v.fields[1]),
                _ => Err("outcome".into()),
            },
            _ => Err("reply".into()),
        }
    }
    pub fn inner_mut(self, value: &mut NdfValue) -> Result<&mut NdfValue, String> {
        match (self, value) {
            (Self::Continuation, v) => Ok(v),
            (Self::Reply, NdfValue::Record(r)) if r.fields.len() == 8 => match &mut r.fields[0] {
                NdfValue::Variant(v) if v.fields.len() == 2 => Ok(&mut v.fields[1]),
                _ => Err("outcome".into()),
            },
            _ => Err("reply".into()),
        }
    }
}

/// Check duplicated fields independently of successful private-state resume.
pub fn check_reply(echo: &NdfValue) -> Result<(), String> {
    let NdfValue::Record(reply) = echo else {
        return Err("reply record".into());
    };
    assert_eq!(reply.kind, "TokenizationReply");
    assert_eq!(reply.fields.len(), 8);
    let NdfValue::Variant(outcome) = &reply.fields[0] else {
        return Err("outcome".into());
    };
    let NdfValue::Record(cont) = &outcome.fields[1] else {
        return Err("continuation".into());
    };
    let NdfValue::Record(cp) = &cont.fields[9] else {
        return Err("checkpoint".into());
    };
    assert_eq!(reply.fields[1], cp.fields[0]);
    assert_eq!(
        reply.fields[2],
        NdfValue::Some(Box::new(cp.fields[1].clone()))
    );
    assert_eq!(reply.fields[3], cont.fields[10]);
    for (r, c) in [(4, 3), (5, 5), (6, 6)] {
        assert_eq!(reply.fields[r], cp.fields[c]);
    }
    assert_eq!(reply.fields[7], cont.fields[16]);
    let NdfValue::Variant(wait) = &cont.fields[13] else {
        return Err("wait".into());
    };
    if wait.variant == "Provider" {
        assert_eq!(outcome.variant, "Await");
        let NdfValue::Record(reader) = &wait.fields[0] else {
            return Err("reader".into());
        };
        assert_eq!(outcome.fields[0], reader.fields[7]);
    } else {
        assert_eq!(wait.variant, "Reservation");
        assert_eq!(outcome.variant, "Reserve");
        assert_eq!(outcome.fields[0], wait.fields[0]);
    }
    Ok(())
}

pub fn changed_fields(echo: &NdfValue) -> Result<Vec<NdfValue>, String> {
    let mut cases = Vec::new();
    for field in 0..8 {
        let mut v = echo.clone();
        let NdfValue::Record(r) = &mut v else {
            return Err("reply".into());
        };
        r.fields[field] = NdfValue::Unit;
        cases.push(v);
    }
    for case in 0..5 {
        let mut v = echo.clone();
        let NdfValue::Record(r) = &mut v else {
            return Err("reply".into());
        };
        match case {
            0 => r.fields[1] = NdfValue::U64(u64::MAX),
            1 => r.fields[2] = NdfValue::Some(Box::new(NdfValue::Text("wrong state".into()))),
            2 => {
                let NdfValue::Record(report) = &mut r.fields[7] else {
                    return Err("report".into());
                };
                let NdfValue::Record(usage) = &mut report.fields[3] else {
                    return Err("usage".into());
                };
                usage.fields[0] = NdfValue::U64(u64::MAX);
            }
            3 => {
                let NdfValue::Variant(o) = &mut r.fields[0] else {
                    return Err("outcome".into());
                };
                o.variant = if o.variant == "Await" {
                    "Reserve"
                } else {
                    "Await"
                }
                .into();
            }
            _ => {
                let NdfValue::Variant(o) = &mut r.fields[0] else {
                    return Err("outcome".into());
                };
                match &mut o.fields[0] {
                    NdfValue::Variant(call) => call.fields[1] = NdfValue::U64(u64::MAX),
                    NdfValue::Record(request) => request.fields[1] = NdfValue::U64(u64::MAX),
                    _ => return Err("call/request".into()),
                };
            }
        }
        cases.push(v);
    }
    Ok(cases)
}

pub fn malformed_headers(echo: &NdfValue) -> Result<Vec<NdfValue>, String> {
    let mut cases = Vec::new();
    for case in 0..8 {
        let mut v = echo.clone();
        let NdfValue::Record(r) = &mut v else {
            return Err("reply".into());
        };
        match case {
            0 => r.schema.package = "foreign".into(),
            1 => r.kind = "foreign".into(),
            2 => {
                r.fields.pop();
            }
            _ => {
                let NdfValue::Variant(o) = &mut r.fields[0] else {
                    return Err("outcome".into());
                };
                match case {
                    3 => o.schema.package = "foreign".into(),
                    4 => o.type_name = "foreign".into(),
                    5 => o.variant = "Token".into(),
                    6 => {
                        o.fields.pop();
                    }
                    _ => o.fields[1] = NdfValue::Unit,
                }
            }
        }
        cases.push(v);
    }
    Ok(cases)
}
