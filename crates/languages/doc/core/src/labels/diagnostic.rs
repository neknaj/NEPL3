use super::*;
use crate::portable::PortableError;
use nepl3_core::{
    diagnostic::{
        Diagnostic, Related, Severity,
        validation::{DiagnosticSourceResolver, ReportValidationError},
    },
    source::{SourceError, SourceSnapshot},
    value_codec::{FoundationCodecError, FoundationValueCodec},
};
#[derive(Debug, Eq, PartialEq)]
pub enum LabelDiagnosticError<E> {
    Stopped(StopReason),
    Validation(ReportValidationError),
    Payload(PortableError<E>),
    NotSemantic,
}
impl<E> From<StopReason> for LabelDiagnosticError<E> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl<E> From<ReportValidationError> for LabelDiagnosticError<E> {
    fn from(e: ReportValidationError) -> Self {
        match e {
            ReportValidationError::Stopped(s) => Self::Stopped(s),
            e => Self::Validation(e),
        }
    }
}
struct Declared<'a>(&'a DocumentSyntax);
impl Declared<'_> {
    fn source(
        &self,
        span: &Span,
        b: &mut Budget,
    ) -> Result<&SourceSnapshot, ReportValidationError> {
        for source in &self.0.sources {
            b.charge(
                Resource::Work,
                (source.identity().source.0.len() + span.snapshot_ref().source.0.len()) as u64 + 40,
            )?;
            if source.identity() == span.snapshot_ref() {
                return Ok(source);
            }
        }
        Err(SourceError::MissingSnapshot.into())
    }
}
impl DiagnosticSourceResolver for Declared<'_> {
    fn slice<'a>(&'a self, span: &Span, b: &mut Budget) -> Result<&'a str, ReportValidationError> {
        Ok(self.source(span, b)?.slice(span)?)
    }
}
impl LabelError<'_> {
    /// Materialize one ordinary typed diagnostic for a semantic label failure.
    /// The borrowed failure survives a construction stop. Structural failures
    /// retain their original typed cause and are not relabelled as name errors.
    pub fn diagnostic<C: FoundationValueCodec>(
        &self,
        doc: &DocumentSyntax,
        registry: &SchemaRegistry,
        codec: &mut C,
        b: &mut Budget,
    ) -> Result<Diagnostic, LabelDiagnosticError<C::Error>> {
        b.poll()?;
        let (code, site, previous, paths) = match self {
            Self::Stopped(s) => return Err(LabelDiagnosticError::Stopped(b.stop(*s))),
            Self::Duplicate {
                definition,
                previous,
            } => ("DuplicateLabel", *definition, Some(*previous), None),
            Self::DuplicateOccurrence { definition, paths } => {
                ("DuplicateOccurrence", *definition, None, Some(paths))
            }
            Self::Unresolved { reference } => ("UnresolvedLabel", *reference, None, None),
            _ => return Err(LabelDiagnosticError::NotSemantic),
        };
        let declared = Declared(doc);
        for span in site
            .selection
            .into_iter()
            .chain(previous.and_then(|p| p.selection))
        {
            let source = declared.source(span, b)?;
            codec
                .admit_source(source, b)
                .map_err(|e| match e.stop_reason() {
                    Some(s) => LabelDiagnosticError::Stopped(s),
                    None => LabelDiagnosticError::Payload(PortableError::Foundation(e)),
                })?;
        }
        let schema = registry
            .selected("nepl3.doc", 1)
            .ok_or(ReportValidationError::Schema(
                nepl3_core::schema::SchemaError::UnknownSchema,
            ))?;
        let arguments = crate::portable::label_arguments(site.name, paths, registry, codec, b)
            .map_err(|e| match e {
                PortableError::Stopped(s) => LabelDiagnosticError::Stopped(s),
                e => LabelDiagnosticError::Payload(e),
            })?;
        b.charge(
            Resource::Work,
            (schema.package.len() + code.len() + 6) as u64,
        )?;
        b.charge(
            Resource::AllocationUnits,
            (schema.package.len() + code.len() + 6 + core::mem::size_of::<Diagnostic>()) as u64,
        )?;
        let primary = site
            .selection
            .map(|s| crate::lower::span(s, b))
            .transpose()?;
        let mut related = Vec::new();
        if let Some(previous) = previous {
            let span = previous
                .selection
                .map(|s| crate::lower::span(s, b))
                .transpose()?;
            let args = arguments.clone_with_budget(b)?;
            b.charge(Resource::Work, 18)?;
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Related>() as u64 + 18,
            )?;
            related.push(Related {
                span,
                code: "PreviousDefinition".into(),
                arguments: args,
            });
        }
        let diagnostic = Diagnostic {
            schema: schema.clone(),
            code: code.into(),
            severity: Severity::Error,
            stage: "labels".into(),
            arguments,
            primary,
            related,
            fixes: Vec::new(),
        };
        diagnostic.validate_with_sources(&declared, registry, b)?;
        b.charge(Resource::Diagnostics, 1)?;
        Ok(diagnostic)
    }
}
