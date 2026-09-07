use super::*;
use crate::package::{EntryContext, ReadSpecId};
use nepl3_core::syntax::TokenRef;

impl Value for ShapeSelection {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        let name = "ShapeSelection";
        match self {
            Self::Form { index } => variant(s.engine, name, "Form", [index.value(s, c, b)?], b),
            Self::Leaf { index } => variant(s.engine, name, "Leaf", [index.value(s, c, b)?], b),
            Self::Builtin { read } => variant(s.engine, name, "Builtin", [read.value(s, c, b)?], b),
            Self::List { read, cons } => variant(
                s.engine,
                name,
                "List",
                [read.value(s, c, b)?, cons.value(s, c, b)?],
                b,
            ),
            Self::Recovery => variant(s.engine, name, "Recovery", [], b),
            Self::Dynamic { .. } => Err(crate::tree::TreeError::UnvalidatedDynamic.into()),
        }
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        Ok(match parts(v, s.engine, "ShapeSelection")? {
            ("Form", [v]) => Self::Form {
                index: <u64 as Value>::read(v, s, c, b)?,
            },
            ("Leaf", [v]) => Self::Leaf {
                index: <u64 as Value>::read(v, s, c, b)?,
            },
            ("Builtin", [v]) => Self::Builtin {
                read: ReadSpecId::read(v, s, c, b)?,
            },
            ("List", [v, cons]) => Self::List {
                read: ReadSpecId::read(v, s, c, b)?,
                cons: <bool as Value>::read(cons, s, c, b)?,
            },
            ("Recovery", []) => Self::Recovery,
            ("Dynamic", _) => return Err(crate::tree::TreeError::UnvalidatedDynamic.into()),
            _ => return Err(PortableError::Shape),
        })
    }
}
impl Value for NodeSelection {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        record(
            s.engine,
            "NodeSelection",
            [
                self.node.value(s, c, b)?,
                self.entry.value(s, c, b)?,
                self.execution_digest.value(s, c, b)?,
                self.shape.value(s, c, b)?,
            ],
            b,
        )
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let f = fields(v, s.engine, "NodeSelection", 4)?;
        Ok(Self {
            node: Value::read(&f[0], s, c, b)?,
            entry: Value::read(&f[1], s, c, b)?,
            execution_digest: Value::read(&f[2], s, c, b)?,
            shape: Value::read(&f[3], s, c, b)?,
        })
    }
}
impl Value for RecoveryKind {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        match self {
            Self::Missing { expected, anchor } => variant(
                s.engine,
                "RecoveryKind",
                "Missing",
                [expected.value(s, c, b)?, anchor.value(s, c, b)?],
                b,
            ),
            Self::Unexpected { token } => variant(
                s.engine,
                "RecoveryKind",
                "Unexpected",
                [token.value(s, c, b)?],
                b,
            ),
            Self::Unparsed { span, reason } => variant(
                s.engine,
                "RecoveryKind",
                "Unparsed",
                [
                    span.value(s, c, b)?,
                    variant(
                        s.engine,
                        "UnparsedReason",
                        match reason {
                            UnparsedReason::UnknownHead => "UnknownHead",
                            UnparsedReason::CategoryMismatch => "CategoryMismatch",
                            UnparsedReason::ProviderFailure => "ProviderFailure",
                        },
                        [],
                        b,
                    )?,
                ],
                b,
            ),
        }
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        Ok(match parts(v, s.engine, "RecoveryKind")? {
            ("Missing", [expected, anchor]) => Self::Missing {
                expected: EntryContext::read(expected, s, c, b)?,
                anchor: Value::read(anchor, s, c, b)?,
            },
            ("Unexpected", [token]) => Self::Unexpected {
                token: TokenRef::read(token, s, c, b)?,
            },
            ("Unparsed", [span, reason]) => Self::Unparsed {
                span: Value::read(span, s, c, b)?,
                reason: match parts(reason, s.engine, "UnparsedReason")? {
                    ("UnknownHead", []) => UnparsedReason::UnknownHead,
                    ("CategoryMismatch", []) => UnparsedReason::CategoryMismatch,
                    ("ProviderFailure", []) => UnparsedReason::ProviderFailure,
                    _ => return Err(PortableError::Shape),
                },
            },
            _ => return Err(PortableError::Shape),
        })
    }
}
impl Value for RecoveryEntry {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        record(
            s.engine,
            "RecoveryEntry",
            [self.node.value(s, c, b)?, self.kind.value(s, c, b)?],
            b,
        )
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let f = fields(v, s.engine, "RecoveryEntry", 2)?;
        Ok(Self {
            node: Value::read(&f[0], s, c, b)?,
            kind: Value::read(&f[1], s, c, b)?,
        })
    }
}
