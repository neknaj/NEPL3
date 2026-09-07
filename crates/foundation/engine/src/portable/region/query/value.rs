use super::*;
pub(super) fn request_value<C: FoundationValueCodec>(
    v: &RegionQueryRequest,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    record(
        s.engine,
        "RegionQueryRequest",
        [v.region.value(s, c, b)?, v.kind.value(s, c, b)?],
        b,
    )
}
pub(super) fn request_read<C: FoundationValueCodec>(
    v: &NdfValue,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<RegionQueryRequest, PortableError<C::Error>> {
    let f = fields(v, s.engine, "RegionQueryRequest", 2)?;
    Ok(RegionQueryRequest {
        region: Value::read(&f[0], s, c, b)?,
        kind: Value::read(&f[1], s, c, b)?,
    })
}
pub(super) fn outcome_value<C: FoundationValueCodec>(
    v: &RegionQueryOutcome,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    Ok(match v {
        RegionQueryOutcome::Complete { region, queries } => {
            let mut values = Vec::new();
            for query in queries {
                push(
                    &mut values,
                    crate::portable::query::value::outcome_value(query, r, s, c, b)?,
                    b,
                )?;
            }
            variant(
                s.engine,
                "RegionQueryOutcome",
                "Complete",
                [region.value(s, c, b)?, NdfValue::List(values)],
                b,
            )?
        }
        RegionQueryOutcome::Invalid(error) => {
            let error = match error {
                RegionQueryError::Region(v) => variant(
                    s.engine,
                    "RegionQueryError",
                    "Region",
                    [super::super::value::error_value(v, r, s, c, b)?],
                    b,
                )?,
                RegionQueryError::Query(v) => variant(
                    s.engine,
                    "RegionQueryError",
                    "Query",
                    [crate::portable::query::value::error_value(v, r, s, c, b)?],
                    b,
                )?,
            };
            variant(s.engine, "RegionQueryOutcome", "Invalid", [error], b)?
        }
        RegionQueryOutcome::Stopped(reason) => variant(
            s.engine,
            "RegionQueryOutcome",
            "Stopped",
            [crate::portable::facts::stop_value(*reason, s, b)?],
            b,
        )?,
    })
}
pub(super) fn outcome_read<C: FoundationValueCodec>(
    v: &NdfValue,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<RegionQueryOutcome, PortableError<C::Error>> {
    let (case, f) = parts(v, s.engine, "RegionQueryOutcome")?;
    Ok(match (case, f) {
        ("Complete", [region, queries]) => {
            let NdfValue::List(queries) = queries else {
                return Err(PortableError::Shape);
            };
            let mut out = Vec::new();
            for query in queries {
                push(
                    &mut out,
                    crate::portable::query::value::outcome_read(query, r, s, c, b)?,
                    b,
                )?;
            }
            RegionQueryOutcome::Complete {
                region: Value::read(region, s, c, b)?,
                queries: out,
            }
        }
        ("Invalid", [error]) => {
            let (case, f) = parts(error, s.engine, "RegionQueryError")?;
            RegionQueryOutcome::Invalid(match (case, f) {
                ("Region", [v]) => {
                    RegionQueryError::Region(super::super::value::error_read(v, r, s, c, b)?)
                }
                ("Query", [v]) => RegionQueryError::Query(
                    crate::portable::query::value::error_read(v, r, s, c, b)?,
                ),
                _ => return Err(PortableError::Shape),
            })
        }
        ("Stopped", [reason]) => RegionQueryOutcome::Stopped(Value::read(reason, s, c, b)?),
        _ => return Err(PortableError::Shape),
    })
}
