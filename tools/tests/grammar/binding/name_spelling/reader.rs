use super::*;
use nepl3_core::diagnostic::{Diagnostic, Severity};

/// Deterministic report behavior from the declared input snapshot, not ambient state.
pub(super) fn decorate(
    reply: &mut nepl3_reader::model::ReadReply,
    source: &SourceSnapshot,
    registry: &nepl3_core::schema::SchemaRegistry,
    b: &mut Budget,
) -> Result<(), String> {
    b.charge(Resource::Work, source.text().len() as u64 + 1)
        .map_err(err)?;
    let severities: &[Severity] = match source.text() {
        "lambda parseError parseError" => &[Severity::Error],
        "lambda parseWarning parseWarning" => &[Severity::Warning],
        "lambda parseInformation parseInformation" => &[Severity::Information],
        "lambda parseHint parseHint" => &[Severity::Hint],
        "lambda parseMixed parseMixed" => &[Severity::Warning, Severity::Error],
        _ => return Ok(()),
    };
    let nepl3_reader::model::ReadReply::Matched { report, .. } = reply else {
        return Ok(());
    };
    for severity in severities {
        b.charge(Resource::AllocationUnits, 2048).map_err(err)?;
        b.charge(Resource::Diagnostics, 1).map_err(err)?;
        let schema = registry
            .selected("nepl3.engine", 1)
            .ok_or("engine schema")?
            .clone();
        report.diagnostics.push(Diagnostic {
            schema: schema.clone(),
            code: "QualityParseFixture".into(),
            severity: *severity,
            stage: "parse".into(),
            arguments: nepl3_core::value::TypedValue::Record(nepl3_core::value::Record {
                schema,
                kind: "BindingDiagnosticArguments".into(),
                fields: vec![
                    NdfValue::Text("Value".into()),
                    NdfValue::Text("fixture".into()),
                ],
            }),
            primary: None,
            related: vec![],
            fixes: vec![],
        });
    }
    report.usage = b.usage();
    Ok(())
}
