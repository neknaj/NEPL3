//! Source attribution captured while compiling typed constructors.
use super::*;
use crate::model::Document;
use nepl3_core::source::Span;

/// An error's precise source claims. Only the compiler can construct this
/// wrapper; attribution is never recursively nested or inferred from text search.
#[derive(Debug, Eq, PartialEq)]
pub struct LocatedCompileError {
    cause: CompileError,
    primary: Span,
    related: Vec<Span>,
    expected: Option<TypeDescriptor>,
    actual: Option<TypeDescriptor>,
}
impl LocatedCompileError {
    pub fn cause(&self) -> &CompileError {
        &self.cause
    }
    pub fn primary(&self) -> &Span {
        &self.primary
    }
    pub fn related(&self) -> &[Span] {
        &self.related
    }
    pub fn expected(&self) -> Option<&TypeDescriptor> {
        self.expected.as_ref()
    }
    pub fn actual(&self) -> Option<&TypeDescriptor> {
        self.actual.as_ref()
    }
}
fn copy_span(span: &Span, budget: &mut Budget) -> Result<Span, StopReason> {
    budget.charge(
        Resource::Work,
        span.snapshot_ref().source.0.len() as u64 + 34,
    )?;
    budget.charge(
        Resource::AllocationUnits,
        span.snapshot_ref().source.0.len() as u64,
    )?;
    Ok(span.clone())
}
impl CompileError {
    pub fn cause(&self) -> &Self {
        match self {
            Self::Located(value) => value.cause(),
            _ => self,
        }
    }
    pub fn location(&self) -> Option<&LocatedCompileError> {
        match self {
            Self::Located(value) => Some(value),
            _ => None,
        }
    }
    pub(crate) fn at(self, primary: &Span, related: Option<&Span>, budget: &mut Budget) -> Self {
        if matches!(self, Self::Stopped(_) | Self::Located(_)) {
            return self;
        }
        let prepared = (|| -> Result<_, CompileError> {
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<LocatedCompileError>() as u64,
            )?;
            let primary = copy_span(primary, budget)?;
            let mut references = Vec::new();
            if let Some(span) = related {
                push(&mut references, copy_span(span, budget)?, budget)?;
            }
            Ok((primary, references))
        })();
        match prepared {
            Ok((primary, related)) => Self::Located(Box::new(LocatedCompileError {
                cause: self,
                primary,
                related,
                expected: None,
                actual: None,
            })),
            Err(error) => error,
        }
    }
    pub(crate) fn at_node(self, doc: &Document, id: NodeId, budget: &mut Budget) -> Self {
        let (primary, related) = match &self {
            Self::Declaration { node, related, .. } => (*node, *related),
            Self::WrongConstructor(node) => (*node, None),
            _ => (id, None),
        };
        let spans = (|| -> Result<_, ModelError> {
            Ok((
                &doc.node(primary)?.span,
                related
                    .map(|id| doc.node(id).map(|node| &node.span))
                    .transpose()?,
            ))
        })();
        match spans {
            Ok((primary, related)) => self.at(primary, related, budget),
            Err(error) => error.into(),
        }
    }
    pub(crate) fn with_types(
        mut self,
        expected: &TypeDescriptor,
        actual: &TypeDescriptor,
        budget: &mut Budget,
    ) -> Self {
        let Self::Located(value) = &mut self else {
            return self;
        };
        let result = (|| -> Result<_, CompileError> {
            Ok((
                expected.clone_with_budget(budget)?,
                actual.clone_with_budget(budget)?,
            ))
        })();
        match result {
            Ok((expected, actual)) => {
                value.expected = Some(expected);
                value.actual = Some(actual);
                self
            }
            Err(error) => error,
        }
    }
}

#[derive(Debug)]
pub enum DiagnosticError<E> {
    Stopped(StopReason),
    Validation(nepl3_core::diagnostic::validation::ReportValidationError),
    Boundary(E),
}
impl<E> From<StopReason> for DiagnosticError<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl<E> From<nepl3_core::diagnostic::validation::ReportValidationError> for DiagnosticError<E> {
    fn from(error: nepl3_core::diagnostic::validation::ReportValidationError) -> Self {
        match error {
            nepl3_core::diagnostic::validation::ReportValidationError::Stopped(reason) => {
                Self::Stopped(reason)
            }
            error => Self::Validation(error),
        }
    }
}
fn boundary<E: nepl3_core::value_codec::FoundationCodecError>(error: E) -> DiagnosticError<E> {
    match error.stop_reason() {
        Some(reason) => DiagnosticError::Stopped(reason),
        None => DiagnosticError::Boundary(error),
    }
}
struct Declared<'a>(&'a Document);
impl Declared<'_> {
    fn source<'a>(
        &'a self,
        span: &Span,
        budget: &mut Budget,
    ) -> Result<
        &'a nepl3_core::source::SourceSnapshot,
        nepl3_core::diagnostic::validation::ReportValidationError,
    > {
        for source in &self.0.sources {
            budget.charge(
                Resource::Work,
                (source.identity().source.0.len() + span.snapshot_ref().source.0.len()) as u64 + 34,
            )?;
            if source.identity() == span.snapshot_ref() {
                return Ok(source);
            }
        }
        Err(nepl3_core::source::SourceError::MissingSnapshot.into())
    }
}
impl nepl3_core::diagnostic::validation::DiagnosticSourceResolver for Declared<'_> {
    fn slice<'a>(
        &'a self,
        span: &Span,
        budget: &mut Budget,
    ) -> Result<&'a str, nepl3_core::diagnostic::validation::ReportValidationError> {
        Ok(self.source(span, budget)?.slice(span)?)
    }
}
impl CompileError {
    /// Materialize one ordinary, schema-validated compiler diagnostic. The
    /// document supplies the exact declaration closure; a fresh operation admits
    /// referenced snapshots through the codec's shared SourceAdmission. No host
    /// store lookup can replace a missing declaration. The borrowed error and its
    /// locations remain available if materialization stops. A stopped compilation
    /// itself never manufactures a diagnostic.
    pub fn diagnostic<C: nepl3_core::value_codec::FoundationValueCodec>(
        &self,
        document: &CheckedDocument<'_>,
        registry: &SchemaRegistry,
        codec: &mut C,
        budget: &mut Budget,
    ) -> Result<nepl3_core::diagnostic::Diagnostic, DiagnosticError<C::Error>> {
        use nepl3_core::{
            diagnostic::{Diagnostic, Related, Severity, validation::ReportValidationError},
            value::{NdfValue, Record, TypedValue},
        };
        budget.poll()?;
        if let Self::Stopped(reason) = self.cause() {
            return Err(DiagnosticError::Stopped(budget.stop(*reason)));
        }
        let declared = Declared(document.document());
        if let Some(location) = self.location() {
            for span in core::iter::once(location.primary()).chain(location.related()) {
                let source = declared.source(span, budget)?;
                codec.admit_source(source, budget).map_err(boundary)?;
            }
        }
        let schema = registry
            .selected("nepl3.grammar", 1)
            .ok_or(ReportValidationError::Schema(SchemaError::UnknownSchema))?;
        let mut fields = Vec::new();
        for ty in [
            self.location().and_then(LocatedCompileError::expected),
            self.location().and_then(LocatedCompileError::actual),
        ] {
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<NdfValue>() as u64,
            )?;
            fields.push(if let Some(ty) = ty {
                let value = codec.encode_type_descriptor(ty, budget).map_err(boundary)?;
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<NdfValue>() as u64,
                )?;
                NdfValue::Some(Box::new(value))
            } else {
                NdfValue::None
            });
        }
        let code = self.code();
        let kind = "CompileDiagnosticArguments";
        budget.charge(
            Resource::Work,
            (schema.package.len() * 2 + kind.len() + code.len() + 7) as u64,
        )?;
        budget.charge(
            Resource::AllocationUnits,
            (schema.package.len() * 2 + kind.len() + code.len() + 7) as u64
                + core::mem::size_of::<Diagnostic>() as u64,
        )?;
        let arguments = TypedValue::Record(Record {
            schema: schema.clone(),
            kind: kind.into(),
            fields,
        });
        let mut related = Vec::new();
        let primary = if let Some(location) = self.location() {
            for span in location.related() {
                let span = copy_span(span, budget)?;
                let arguments = arguments.clone_with_budget(budget)?;
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<Related>() as u64 + code.len() as u64,
                )?;
                budget.charge(Resource::Work, code.len() as u64 + 1)?;
                related.push(Related {
                    span: Some(span),
                    code: code.into(),
                    arguments,
                });
            }
            Some(copy_span(location.primary(), budget)?)
        } else {
            None
        };
        let diagnostic = Diagnostic {
            schema: schema.clone(),
            code: code.into(),
            severity: Severity::Error,
            stage: "compile".into(),
            arguments,
            primary,
            related,
            fixes: Vec::new(),
        };
        diagnostic.validate_with_sources(&declared, registry, budget)?;
        budget.charge(Resource::Diagnostics, 1)?;
        Ok(diagnostic)
    }
    pub fn code(&self) -> &'static str {
        match self.cause() {
            Self::Located(value) => value.cause().code(),
            Self::Stopped(_) => "Stopped",
            Self::Model(_) => "Model",
            Self::Schema(_) => "Schema",
            Self::Reader(error) => plan_code(error),
            Self::Package(error) => package_code(error),
            Self::WrongConstructor(_) => "WrongConstructor",
            Self::NaturalOverflow => "NaturalOverflow",
            Self::InvalidRange => "InvalidRange",
            Self::InvalidRepeat => "InvalidRepeat",
            Self::MissingProvider => "MissingProvider",
            Self::ProviderKind => "ProviderKind",
            Self::MissingClass => "MissingClass",
            Self::MissingView => "MissingView",
            Self::MissingRule => "MissingRule",
            Self::DuplicateRule => "DuplicateRule",
            Self::OutputType => "OutputType",
            Self::InvalidCatalogName(_) => "InvalidCatalogName",
            Self::DuplicateCatalogName(_) => "DuplicateCatalogName",
            Self::Declaration { reason, .. } => match reason {
                DeclarationError::KindShape => "KindShape",
                DeclarationError::EmptyName => "EmptyName",
                DeclarationError::DuplicateName => "DuplicateName",
                DeclarationError::MissingExtension => "MissingExtension",
                DeclarationError::Signature => "Signature",
                DeclarationError::MissingReader => "MissingReader",
                DeclarationError::MissingToken => "MissingToken",
                DeclarationError::MissingClass => "MissingClass",
                DeclarationError::MissingView => "MissingView",
                DeclarationError::InvalidBuiltin => "InvalidBuiltin",
                DeclarationError::InvalidRead => "InvalidRead",
                DeclarationError::InvalidBinding => "InvalidBinding",
                DeclarationError::InvalidSelector => "InvalidSelector",
            },
        }
    }
}
fn plan_code(error: &PlanError) -> &'static str {
    match error {
        PlanError::Schema(_) => "Schema",
        PlanError::Stopped(_) => "Stopped",
        PlanError::Reference => "Reference",
        PlanError::DuplicateRule => "DuplicateRule",
        PlanError::DuplicateProvider => "DuplicateProvider",
        PlanError::EmptyName => "EmptyName",
        PlanError::InvalidRange => "InvalidRange",
        PlanError::InvalidRepeat => "InvalidRepeat",
        PlanError::EmptyDelimiter => "EmptyDelimiter",
        PlanError::UnknownSchema => "UnknownSchema",
        PlanError::UnknownKind => "UnknownKind",
        PlanError::ProviderSignature => "ProviderSignature",
        PlanError::OutputType => "OutputType",
        PlanError::NonProgress => "NonProgress",
        PlanError::LeftRecursion => "LeftRecursion",
    }
}
fn package_code(error: &nepl3_engine::package::PackageError) -> &'static str {
    use nepl3_engine::package::PackageError as P;
    match error {
        P::Stopped(_) => "Stopped",
        P::Schema(_) => "Schema",
        P::Reader(error) => plan_code(error),
        P::Source(_) => "Source",
        P::Origin(_) => "Origin",
        P::EmptyName => "EmptyName",
        P::DuplicateName => "DuplicateName",
        P::MissingMode => "MissingMode",
        P::MissingCategory => "MissingCategory",
        P::MissingReader => "MissingReader",
        P::MissingNamespace => "MissingNamespace",
        P::MissingExtension => "MissingExtension",
        P::SignatureMismatch => "SignatureMismatch",
        P::InvalidRead => "InvalidRead",
        P::InvalidBinding => "InvalidBinding",
        P::DirectCycle => "DirectCycle",
        P::KindShape => "KindShape",
        P::InvalidSelector => "InvalidSelector",
        P::UnvisitedField => "UnvisitedField",
        P::Provenance => "Provenance",
    }
}
