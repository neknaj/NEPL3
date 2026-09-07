//! Exact typed causes, including nested foundation/reader failures.
use super::*;
use crate::facts::FactsError;
use crate::{binding::BindingError, package::PackageError, profile::ProfileError, tree::TreeError};
use nepl3_core::diagnostic::validation::ReportValidationError;
use nepl3_core::{
    facts::FactError, origin::OriginError, schema::SchemaError, source::SourceError,
    syntax::SyntaxError, value::SchemaRef, view::ViewError,
};
use nepl3_reader::plan::PlanError;
trait StopCause {
    fn stop_cause(&self) -> Option<StopReason>;
}
pub(in crate::portable) fn source_value<E>(
    error: &SourceError,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    source(error, &ErrorSchemas::new(registry)?, b)
}
pub(in crate::portable) fn source_read<E>(
    value: &NdfValue,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<SourceError, PortableError<E>> {
    source_from(value, &ErrorSchemas::new(registry)?, b)
}
pub(super) fn stop_reason(error: &BindingError) -> Option<StopReason> {
    error.stop_cause()
}
struct ErrorSchemas<'a> {
    engine: &'a SchemaRef,
    foundation: &'a SchemaRef,
    reader: &'a SchemaRef,
}
impl<'a> ErrorSchemas<'a> {
    fn new<E>(registry: &'a SchemaRegistry) -> Result<Self, PortableError<E>> {
        Ok(Self {
            engine: registry
                .selected("nepl3.engine", 1)
                .ok_or(SchemaError::UnknownSchema)?,
            foundation: registry
                .selected("nepl3.foundation", 1)
                .ok_or(SchemaError::UnknownSchema)?,
            reader: registry
                .selected("nepl3.reader", 1)
                .ok_or(SchemaError::UnknownSchema)?,
        })
    }
    fn value_schemas(&self) -> Schemas<'_> {
        Schemas {
            engine: self.engine,
            foundation: self.foundation,
        }
    }
}
pub(super) fn encode<E>(
    value: &BindingError,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    binding(value, &ErrorSchemas::new(registry)?, b)
}
pub(super) fn decode<E>(
    value: &NdfValue,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<BindingError, PortableError<E>> {
    binding_from(value, &ErrorSchemas::new(registry)?, b)
}
macro_rules! codec {
    ($encode:ident,$decode:ident,$ty:ident,$owner:ident,$name:literal,[$($unit:ident),*],[$($case:ident:$nested:ident/$nested_from:ident),*])=> {
        impl StopCause for $ty {
            fn stop_cause(&self)->Option<StopReason> {
                match self {
                    $($ty::$unit=>None,)*
                    $($ty::$case(v)=>v.stop_cause(),)*
                    $ty::Stopped(reason)=>Some(*reason),
                }
            }
        }
        fn $encode<E>(value:&$ty,s:&ErrorSchemas<'_>,b:&mut Budget)->Result<NdfValue,PortableError<E>> {
            b.with_depth(|b| {
                b.charge(Resource::Work,1)?;
                match value {
                    $($ty::$unit=>variant(s.$owner,$name,stringify!($unit),[],b),)*
                    $($ty::$case(v)=>variant(s.$owner,$name,stringify!($case),[$nested(v,s,b)?],b),)*
                    $ty::Stopped(v)=>variant(s.$owner,$name,"Stopped",[super::super::facts::stop_value(*v,&s.value_schemas(),b)?],b),
                }
            })
        }
        fn $decode<E>(value:&NdfValue,s:&ErrorSchemas<'_>,b:&mut Budget)->Result<$ty,PortableError<E>> {
            b.with_depth(|b| {
                b.charge(Resource::Work,1)?;
                let(name,f)=parts(value,s.$owner,$name)?;
                match(name,f) {
                    $((stringify!($unit),[])=>Ok($ty::$unit),)*
                    $((stringify!($case),[v])=>Ok($ty::$case($nested_from(v,s,b)?)),)*
                    ("Stopped",[v])=>Ok($ty::Stopped(super::super::facts::stop_from(v,&s.value_schemas())?)),
                    _=>Err(PortableError::Shape),
                }
            })
        }
    }
}
impl StopCause for SourceError {
    fn stop_cause(&self) -> Option<StopReason> {
        match self {
            Self::Stopped(reason) => Some(*reason),
            _ => None,
        }
    }
}
codec!(
    schema,
    schema_from,
    SchemaError,
    foundation,
    "SchemaError",
    [
        DuplicateName,
        EmptyName,
        DescriptorDepth,
        IdentityMismatch,
        ConflictingSchema,
        UnknownSchema,
        UnknownType,
        WrongType,
        FieldCount,
        UnknownVariant,
        Unfinalized
    ],
    []
);
codec!(origin,origin_from,OriginError,foundation,"OriginError",
    [Reference,Cycle,EmptyReason,Ambiguous,Irreversible,Unmapped],[Source:source/source_from]);
codec!(view,view_from,ViewError,foundation,"ViewError",
    [Reference,Cycle,Cover,DuplicateField,Presentation,Trivia],[Source:source/source_from,Origin:origin/origin_from,Schema:schema/schema_from]);
codec!(syntax,syntax_from,SyntaxError,foundation,"SyntaxError",
    [Reference,Cycle,Cover,ChildOrder,DuplicateSource,DuplicateEnvironment,Environment,ForeignRoot,ResourceDigest],
    [Source:source/source_from,Origin:origin/origin_from,Schema:schema/schema_from,View:view/view_from]);
codec!(fact,fact_from,FactError,foundation,"FactError",
    [Analysis,DuplicateId,MissingScope,MissingNamespace,MissingEntity,MissingOccurrence,MissingOrigin,Cycle,Name,Span,Resolution,Authority,Reservation],
    [Source:source/source_from,Origin:origin/origin_from,Schema:schema/schema_from]);
codec!(plan,plan_from,PlanError,reader,"PlanError",
    [Reference,DuplicateRule,DuplicateProvider,EmptyName,InvalidRange,InvalidRepeat,EmptyDelimiter,UnknownSchema,UnknownKind,ProviderSignature,OutputType,NonProgress,LeftRecursion],
    [Schema:schema/schema_from]);
codec!(package,package_from,PackageError,engine,"PackageError",
    [EmptyName,DuplicateName,MissingMode,MissingCategory,MissingReader,MissingNamespace,MissingExtension,SignatureMismatch,InvalidRead,InvalidBinding,DirectCycle,KindShape,InvalidSelector,UnvisitedField,Provenance],
    [Schema:schema/schema_from,Reader:plan/plan_from,Source:source/source_from,Origin:origin/origin_from]);
codec!(profile,profile_from,ProfileError,engine,"ProfileError",
    [EmptyName,Duplicate,MissingPackage,PackageIdentity,MissingSchema,MissingAlias,MissingCategory,MissingMode,MissingProvider,ProviderIdentity,NotAllowed,MissingResource,ResourceIdentity,HeadSignature],
    [Package:package/package_from,Schema:schema/schema_from]);
codec!(tree,tree_from,TreeError,engine,"TreeError",
    [Path,Duplicate,Selection,ExecutionIdentity,Recovery,Unreachable,UnvalidatedDynamic],
    [Syntax:syntax/syntax_from,Package:package/package_from,Profile:profile/profile_from]);
codec!(binding,binding_from,BindingError,engine,"BindingFailure",
    [AnalysisId,Target,Name,MissingNamespace,NamespaceBoundary,RecoveredTree,MissingProvider,UnsupportedPlan,DuplicateGlobal,ProviderInvalid],
    [Tree:tree/tree_from,Profile:profile/profile_from,Fact:fact/fact_from,Source:source/source_from,Schema:schema/schema_from,Facts:facts/facts_from]);
codec!(report,report_from,ReportValidationError,foundation,"ReportValidationError",
    [Metadata,Usage],[Source:source/source_from,Schema:schema/schema_from]);
codec!(facts,facts_from,FactsError,engine,"FactsError",
    [Target,Phase],[Tree:tree/tree_from,Fact:fact/fact_from,Source:source/source_from,Origin:origin/origin_from,Report:report/report_from]);
fn source<E>(
    value: &SourceError,
    s: &ErrorSchemas<'_>,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    b.with_depth(|b| {
        b.charge(Resource::Work, 1)?;
        let name = match value {
            SourceError::Stopped(reason) => {
                return variant(
                    s.foundation,
                    "SourceError",
                    "Stopped",
                    [super::super::facts::stop_value(
                        *reason,
                        &s.value_schemas(),
                        b,
                    )?],
                    b,
                );
            }
            SourceError::Decode {
                valid_up_to,
                error_len,
            } => {
                return variant(
                    s.foundation,
                    "SourceError",
                    "Decode",
                    [
                        NdfValue::U64(*valid_up_to),
                        super::super::facts::optional(error_len.map(NdfValue::U64), b)?,
                    ],
                    b,
                );
            }
            SourceError::Bounds => "Bounds",
            SourceError::ScalarBoundary => "ScalarBoundary",
            SourceError::SnapshotMismatch => "SnapshotMismatch",
            SourceError::Position => "Position",
            SourceError::LineTerminator => "LineTerminator",
            SourceError::IdentityConflict => "IdentityConflict",
            SourceError::MissingSnapshot => "MissingSnapshot",
            SourceError::Revision => "Revision",
            SourceError::OverlappingEdits => "OverlappingEdits",
            SourceError::ExpectedDigest => "ExpectedDigest",
            SourceError::Locator => "Locator",
        };
        variant(s.foundation, "SourceError", name, [], b)
    })
}
fn source_from<E>(
    value: &NdfValue,
    s: &ErrorSchemas<'_>,
    b: &mut Budget,
) -> Result<SourceError, PortableError<E>> {
    b.with_depth(|b| {
        b.charge(Resource::Work, 1)?;
        let (name, f) = parts(value, s.foundation, "SourceError")?;
        Ok(match (name, f) {
            ("Stopped", [v]) => {
                SourceError::Stopped(super::super::facts::stop_from(v, &s.value_schemas())?)
            }
            ("Decode", [NdfValue::U64(valid), len]) => SourceError::Decode {
                valid_up_to: *valid,
                error_len: match len {
                    NdfValue::None => None,
                    NdfValue::Some(v) => match v.as_ref() {
                        NdfValue::U64(v) => Some(*v),
                        _ => return Err(PortableError::Shape),
                    },
                    _ => return Err(PortableError::Shape),
                },
            },
            ("Bounds", []) => SourceError::Bounds,
            ("ScalarBoundary", []) => SourceError::ScalarBoundary,
            ("SnapshotMismatch", []) => SourceError::SnapshotMismatch,
            ("Position", []) => SourceError::Position,
            ("LineTerminator", []) => SourceError::LineTerminator,
            ("IdentityConflict", []) => SourceError::IdentityConflict,
            ("MissingSnapshot", []) => SourceError::MissingSnapshot,
            ("Revision", []) => SourceError::Revision,
            ("OverlappingEdits", []) => SourceError::OverlappingEdits,
            ("ExpectedDigest", []) => SourceError::ExpectedDigest,
            ("Locator", []) => SourceError::Locator,
            _ => return Err(PortableError::Shape),
        })
    })
}
