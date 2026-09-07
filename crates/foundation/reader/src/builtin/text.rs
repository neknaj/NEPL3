use super::*;
use alloc::string::String;
use nepl3_core::origin::{Mapping, MappingKind};

struct Segment {
    source_start: usize,
    source_end: usize,
    target_start: usize,
    target_end: usize,
    kind: MappingKind,
}

pub(super) fn read(
    request: ReadRequest<'_>,
    reservation: &SourceReservation,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<ReadReply, ReaderError> {
    let input = request.snapshot.slice_range(request.start, request.limit)?;
    if !input.starts_with('"') {
        let scan = if input.is_empty() && !request.final_input {
            lexical::Scan::NeedMore
        } else {
            lexical::Scan::NoMatch
        };
        return rejection(scan, BuiltinReader::Text, &request, registry, budget);
    }
    let mut decoded = String::new();
    let mut segments: Vec<Segment> = Vec::new();
    let mut cursor = 1;
    loop {
        budget.charge(Resource::Work, 1)?;
        let Some(ch) = input[cursor..].chars().next() else {
            return incomplete("UnterminatedLiteral", cursor, &request, registry, budget);
        };
        if ch == '"' {
            cursor += 1;
            break;
        }
        if matches!(ch, '\r' | '\n') {
            return rejection(
                lexical::Scan::Failed("DirectLineBreak", cursor),
                BuiltinReader::Text,
                &request,
                registry,
                budget,
            );
        }
        let source_start = cursor;
        cursor += ch.len_utf8();
        let (value, kind) = if ch == '\\' {
            let Some(escaped) = input[cursor..].chars().next() else {
                return incomplete("InvalidEscape", cursor, &request, registry, budget);
            };
            cursor += escaped.len_utf8();
            let value = match escaped {
                '\\' => '\\',
                '"' => '"',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'u' => {
                    if cursor == input.len() {
                        return incomplete("InvalidEscape", cursor, &request, registry, budget);
                    }
                    if input.as_bytes()[cursor] != b'{' {
                        return rejection(
                            lexical::Scan::Failed("InvalidEscape", cursor),
                            BuiltinReader::Text,
                            &request,
                            registry,
                            budget,
                        );
                    }
                    cursor += 1;
                    let mut scalar = 0u32;
                    let mut digits = 0;
                    loop {
                        budget.charge(Resource::Work, 1)?;
                        let Some(hex) = input[cursor..].chars().next() else {
                            return incomplete("InvalidEscape", cursor, &request, registry, budget);
                        };
                        if hex == '}' {
                            cursor += 1;
                            break;
                        }
                        let Some(digit) = hex.to_digit(16).filter(|_| hex.is_ascii_hexdigit())
                        else {
                            return rejection(
                                lexical::Scan::Failed("InvalidEscape", cursor),
                                BuiltinReader::Text,
                                &request,
                                registry,
                                budget,
                            );
                        };
                        if digits == 6 {
                            return rejection(
                                lexical::Scan::Failed("InvalidScalar", cursor),
                                BuiltinReader::Text,
                                &request,
                                registry,
                                budget,
                            );
                        }
                        scalar = scalar * 16 + digit;
                        digits += 1;
                        cursor += 1;
                    }
                    let Some(scalar) = char::from_u32(scalar).filter(|_| digits != 0) else {
                        return rejection(
                            lexical::Scan::Failed("InvalidScalar", source_start),
                            BuiltinReader::Text,
                            &request,
                            registry,
                            budget,
                        );
                    };
                    scalar
                }
                _ => {
                    return rejection(
                        lexical::Scan::Failed("InvalidEscape", source_start),
                        BuiltinReader::Text,
                        &request,
                        registry,
                        budget,
                    );
                }
            };
            (value, MappingKind::Transformed)
        } else {
            (ch, MappingKind::Exact)
        };
        budget.charge(Resource::OutputBytes, value.len_utf8() as u64)?;
        budget.charge(Resource::AllocationUnits, value.len_utf8() as u64)?;
        let target_start = decoded.len();
        let previous = segments.last_mut().filter(|previous| {
            previous.kind == MappingKind::Exact
                && kind == MappingKind::Exact
                && previous.source_end == source_start
                && previous.target_end == target_start
                && previous.source_end - previous.source_start
                    == previous.target_end - previous.target_start
        });
        if let Some(previous) = previous {
            // Adjacent unchanged UTF-8 bytes have the same exact displacement.
            // Escape relations remain separate; no transformed segment or empty
            // insertion anchor is merged into a literal run.
            decoded.push(value);
            previous.source_end = cursor;
            previous.target_end = decoded.len();
        } else {
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Segment>() as u64,
            )?;
            decoded.push(value);
            segments.push(Segment {
                source_start,
                source_end: cursor,
                target_start,
                target_end: decoded.len(),
                kind,
            });
        }
    }
    if segments.is_empty() {
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Segment>() as u64,
        )?;
        segments.push(Segment {
            source_start: 1,
            source_end: 1,
            target_start: 0,
            target_end: 0,
            kind: MappingKind::Exact,
        });
    }
    budget.charge(Resource::Work, sources.snapshots().len() as u64)?;
    for source in sources.snapshots() {
        if source.identity().source == reservation.source_id
            && source.identity().revision == reservation.revision
            && (source.uri() != reservation.uri || source.text() != decoded)
        {
            return Err(SourceError::IdentityConflict.into());
        }
    }
    budget.charge(
        Resource::AllocationUnits,
        (reservation.source_id.0.len()
            + reservation.uri.len()
            + decoded.len()
            + core::mem::size_of::<nepl3_core::source::SourceSnapshot>()) as u64,
    )?;
    let generated = admission.import(
        reservation.source_id.clone(),
        reservation.revision,
        reservation.uri.clone(),
        decoded.as_bytes().to_vec(),
        budget,
    )?;
    let mut maps = Vec::new();
    for segment in segments {
        budget.charge(Resource::Work, 1)?;
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Mapping>() as u64,
        )?;
        maps.push(Mapping {
            source: request.snapshot.span_with_budget(
                request.start + segment.source_start as u64,
                request.start + segment.source_end as u64,
                budget,
            )?,
            target: generated.span_with_budget(
                segment.target_start as u64,
                segment.target_end as u64,
                budget,
            )?,
            kind: segment.kind,
        });
    }
    let new_state = request.state.clone_with_budget(budget)?;
    Ok(ReadReply::Matched {
        value: NdfValue::Text(decoded),
        end: request.start + cursor as u64,
        new_state,
        view: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        facts: vec![],
        sources: vec![generated],
        source_maps: maps,
        report: report(budget),
    })
}
fn incomplete(
    code: &'static str,
    offset: usize,
    request: &ReadRequest<'_>,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<ReadReply, ReaderError> {
    rejection(
        if request.final_input {
            lexical::Scan::Failed(code, offset)
        } else {
            lexical::Scan::NeedMore
        },
        BuiltinReader::Text,
        request,
        registry,
        budget,
    )
}
