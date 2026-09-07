use super::*;
use nepl3_core::{
    origin::OriginError, schema::SchemaError, source::SourceError, syntax::SyntaxError,
    view::ViewError,
};
use nepl3_engine::{package::PackageError, profile::ProfileError, tree::TreeError};
use nepl3_reader::plan::PlanError;

fn roundtrip(
    cause: BindingError,
    path: &[&str],
    registry: &nepl3_core::schema::SchemaRegistry,
) -> Result<(), String> {
    let mut b = budget();
    let value = wire_binding::failure_to_value(&cause, registry, &mut b).map_err(err)?;
    let bytes = nepl3_wire::encode(&value, &mut b).map_err(err)?;
    let received = nepl3_wire::decode(&bytes, &mut b).map_err(err)?;
    assert_eq!(
        wire_binding::failure_from_value(&received, registry, &mut b).map_err(err)?,
        cause
    );
    let mut cursor = &received;
    for (i, case) in path.iter().enumerate() {
        let NdfValue::Variant(v) = cursor else {
            return Err(format!("cause path {path:?}"));
        };
        assert_eq!(v.variant, *case);
        if i + 1 < path.len() {
            assert_eq!(v.fields.len(), 1);
            cursor = &v.fields[0];
        }
    }
    Ok(())
}
#[test]
fn every_binding_failure_variant_keeps_its_typed_nested_cause() -> Result<(), String> {
    let compiled = execution()?;
    let r = &compiled.registry;
    macro_rules! cases {($ty:ident,[$($case:ident),*])=>{[$((stringify!($case),$ty::$case)),*]};}
    for (name, value) in cases!(
        BindingError,
        [
            AnalysisId,
            Target,
            Name,
            MissingNamespace,
            NamespaceBoundary,
            RecoveredTree,
            MissingProvider,
            UnsupportedPlan,
            DuplicateGlobal
        ]
    ) {
        roundtrip(value, &[name], r)?;
    }
    for (name, value) in cases!(
        SourceError,
        [
            Bounds,
            ScalarBoundary,
            SnapshotMismatch,
            Position,
            LineTerminator,
            IdentityConflict,
            MissingSnapshot,
            Revision,
            OverlappingEdits,
            ExpectedDigest,
            Locator
        ]
    ) {
        roundtrip(BindingError::Source(value), &["Source", name], r)?;
    }
    for len in [None, Some(4)] {
        let value = SourceError::Decode {
            valid_up_to: u64::MAX,
            error_len: len,
        };
        let packet =
            wire_binding::failure_to_value(&BindingError::Source(value.clone()), r, &mut budget())
                .map_err(err)?;
        let NdfValue::Variant(outer) = &packet else {
            return Err("failure".into());
        };
        let NdfValue::Variant(inner) = &outer.fields[0] else {
            return Err("source cause".into());
        };
        assert_eq!(inner.fields[0], NdfValue::U64(u64::MAX));
        assert_eq!(
            inner.fields[1],
            match len {
                None => NdfValue::None,
                Some(v) => NdfValue::Some(Box::new(NdfValue::U64(v))),
            }
        );
        roundtrip(BindingError::Source(value), &["Source", "Decode"], r)?;
    }
    for (name, value) in cases!(
        SchemaError,
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
        ]
    ) {
        roundtrip(BindingError::Schema(value), &["Schema", name], r)?;
    }
    for (name, value) in cases!(
        OriginError,
        [
            Reference,
            Cycle,
            EmptyReason,
            Ambiguous,
            Irreversible,
            Unmapped
        ]
    ) {
        roundtrip(
            BindingError::Fact(FactError::Origin(value)),
            &["Fact", "Origin", name],
            r,
        )?;
    }
    for (name, value) in cases!(
        ViewError,
        [
            Reference,
            Cycle,
            Cover,
            DuplicateField,
            Presentation,
            Trivia
        ]
    ) {
        roundtrip(
            BindingError::Tree(TreeError::Syntax(SyntaxError::View(value))),
            &["Tree", "Syntax", "View", name],
            r,
        )?;
    }
    for (name, value) in cases!(
        SyntaxError,
        [
            Reference,
            Cycle,
            Cover,
            ChildOrder,
            DuplicateSource,
            DuplicateEnvironment,
            Environment,
            ForeignRoot,
            ResourceDigest
        ]
    ) {
        roundtrip(
            BindingError::Tree(TreeError::Syntax(value)),
            &["Tree", "Syntax", name],
            r,
        )?;
    }
    for (name, value) in cases!(
        FactError,
        [
            Analysis,
            DuplicateId,
            MissingScope,
            MissingNamespace,
            MissingEntity,
            MissingOccurrence,
            MissingOrigin,
            Cycle,
            Name,
            Span,
            Resolution,
            Authority,
            Reservation
        ]
    ) {
        roundtrip(BindingError::Fact(value), &["Fact", name], r)?;
    }
    for (name, value) in cases!(
        PlanError,
        [
            Reference,
            DuplicateRule,
            DuplicateProvider,
            EmptyName,
            InvalidRange,
            InvalidRepeat,
            EmptyDelimiter,
            UnknownSchema,
            UnknownKind,
            ProviderSignature,
            OutputType,
            NonProgress,
            LeftRecursion
        ]
    ) {
        roundtrip(
            BindingError::Profile(ProfileError::Package(PackageError::Reader(value))),
            &["Profile", "Package", "Reader", name],
            r,
        )?;
    }
    for (name, value) in cases!(
        PackageError,
        [
            EmptyName,
            DuplicateName,
            MissingMode,
            MissingCategory,
            MissingReader,
            MissingNamespace,
            MissingExtension,
            SignatureMismatch,
            InvalidRead,
            InvalidBinding,
            DirectCycle,
            KindShape,
            InvalidSelector,
            UnvisitedField,
            Provenance
        ]
    ) {
        roundtrip(
            BindingError::Profile(ProfileError::Package(value)),
            &["Profile", "Package", name],
            r,
        )?;
    }
    for (name, value) in cases!(
        ProfileError,
        [
            EmptyName,
            Duplicate,
            MissingPackage,
            PackageIdentity,
            MissingSchema,
            MissingAlias,
            MissingCategory,
            MissingMode,
            MissingProvider,
            ProviderIdentity,
            NotAllowed,
            MissingResource,
            ResourceIdentity,
            HeadSignature
        ]
    ) {
        roundtrip(BindingError::Profile(value), &["Profile", name], r)?;
    }
    for (name, value) in cases!(
        TreeError,
        [
            Path,
            Duplicate,
            Selection,
            ExecutionIdentity,
            Recovery,
            Unreachable,
            UnvalidatedDynamic
        ]
    ) {
        roundtrip(BindingError::Tree(value), &["Tree", name], r)?;
    }
    // Every nested constructor is exercised as well as each unit case above.
    let nested = [
        (
            BindingError::Tree(TreeError::Package(PackageError::Schema(
                SchemaError::FieldCount,
            ))),
            vec!["Tree", "Package", "Schema", "FieldCount"],
        ),
        (
            BindingError::Tree(TreeError::Profile(ProfileError::Schema(
                SchemaError::UnknownSchema,
            ))),
            vec!["Tree", "Profile", "Schema", "UnknownSchema"],
        ),
        (
            BindingError::Profile(ProfileError::Package(PackageError::Source(
                SourceError::Bounds,
            ))),
            vec!["Profile", "Package", "Source", "Bounds"],
        ),
        (
            BindingError::Profile(ProfileError::Package(PackageError::Origin(
                OriginError::Source(SourceError::MissingSnapshot),
            ))),
            vec!["Profile", "Package", "Origin", "Source", "MissingSnapshot"],
        ),
        (
            BindingError::Profile(ProfileError::Package(PackageError::Reader(
                PlanError::Schema(SchemaError::WrongType),
            ))),
            vec!["Profile", "Package", "Reader", "Schema", "WrongType"],
        ),
        (
            BindingError::Fact(FactError::Source(SourceError::Revision)),
            vec!["Fact", "Source", "Revision"],
        ),
        (
            BindingError::Fact(FactError::Schema(SchemaError::ConflictingSchema)),
            vec!["Fact", "Schema", "ConflictingSchema"],
        ),
        (
            BindingError::Tree(TreeError::Syntax(SyntaxError::Source(
                SourceError::Position,
            ))),
            vec!["Tree", "Syntax", "Source", "Position"],
        ),
        (
            BindingError::Tree(TreeError::Syntax(SyntaxError::Origin(OriginError::Cycle))),
            vec!["Tree", "Syntax", "Origin", "Cycle"],
        ),
        (
            BindingError::Tree(TreeError::Syntax(SyntaxError::Schema(
                SchemaError::WrongType,
            ))),
            vec!["Tree", "Syntax", "Schema", "WrongType"],
        ),
        (
            BindingError::Tree(TreeError::Syntax(SyntaxError::View(ViewError::Source(
                SourceError::Bounds,
            )))),
            vec!["Tree", "Syntax", "View", "Source", "Bounds"],
        ),
        (
            BindingError::Tree(TreeError::Syntax(SyntaxError::View(ViewError::Origin(
                OriginError::Unmapped,
            )))),
            vec!["Tree", "Syntax", "View", "Origin", "Unmapped"],
        ),
        (
            BindingError::Tree(TreeError::Syntax(SyntaxError::View(ViewError::Schema(
                SchemaError::UnknownType,
            )))),
            vec!["Tree", "Syntax", "View", "Schema", "UnknownType"],
        ),
    ];
    for (value, path) in nested {
        roundtrip(value, &path, r)?;
    }
    for (name, reason) in cases!(
        StopReason,
        [
            Cancelled,
            SourceLimit,
            WorkLimit,
            DepthLimit,
            NodeLimit,
            AllocationLimit,
            OutputLimit,
            DiagnosticLimit,
            EventLimit
        ]
    ) {
        roundtrip(BindingError::Stopped(reason), &["Stopped", name], r)?;
        roundtrip(
            BindingError::Source(SourceError::Stopped(reason)),
            &["Source", "Stopped", name],
            r,
        )?;
        roundtrip(
            BindingError::Schema(SchemaError::Stopped(reason)),
            &["Schema", "Stopped", name],
            r,
        )?;
        roundtrip(
            BindingError::Fact(FactError::Stopped(reason)),
            &["Fact", "Stopped", name],
            r,
        )?;
        roundtrip(
            BindingError::Tree(TreeError::Stopped(reason)),
            &["Tree", "Stopped", name],
            r,
        )?;
        roundtrip(
            BindingError::Profile(ProfileError::Stopped(reason)),
            &["Profile", "Stopped", name],
            r,
        )?;
        roundtrip(
            BindingError::Tree(TreeError::Package(PackageError::Stopped(reason))),
            &["Tree", "Package", "Stopped", name],
            r,
        )?;
        roundtrip(
            BindingError::Tree(TreeError::Package(PackageError::Reader(
                PlanError::Stopped(reason),
            ))),
            &["Tree", "Package", "Reader", "Stopped", name],
            r,
        )?;
        roundtrip(
            BindingError::Tree(TreeError::Syntax(SyntaxError::Stopped(reason))),
            &["Tree", "Syntax", "Stopped", name],
            r,
        )?;
        roundtrip(
            BindingError::Tree(TreeError::Syntax(SyntaxError::View(ViewError::Stopped(
                reason,
            )))),
            &["Tree", "Syntax", "View", "Stopped", name],
            r,
        )?;
        roundtrip(
            BindingError::Fact(FactError::Origin(OriginError::Stopped(reason))),
            &["Fact", "Origin", "Stopped", name],
            r,
        )?;
    }
    Ok(())
}
