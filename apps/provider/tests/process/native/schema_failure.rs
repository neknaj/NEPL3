//! Failure fixtures use the real pipes and require EOF without an operation.
use super::*;
use nepl3_provider::schema::{SchemaAdmissionError, SchemaExchangeError};
use std::io::Read;

#[derive(Clone, Copy)]
pub(super) enum Mode {
    Missing,
    WrongIdentity,
    Eof,
}
impl Mode {
    fn argument(self) -> &'static str {
        match self {
            Self::Missing => "--schema-missing",
            Self::WrongIdentity => "--schema-wrong",
            Self::Eof => "--schema-eof",
        }
    }
}
pub(super) fn child_mode() -> Option<Mode> {
    [Mode::Missing, Mode::WrongIdentity, Mode::Eof]
        .into_iter()
        .find(|mode| std::env::args().any(|arg| arg == mode.argument()))
}
pub(super) fn child(mode: Mode) -> Result<(), String> {
    let registry = bootstrap()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut connection = Connection::new(io::stdin().lock(), io::stdout().lock());
    let request = connection
        .receive(&registry, &sources, &mut admission, &mut budget())
        .map_err(error)?;
    if !matches!(request, Some(ProviderFrame::SchemaRequest { schemas }) if schemas.len() == 1) {
        return Err("expected one schema selection".into());
    }
    let descriptors = match mode {
        Mode::Eof => return Ok(()),
        Mode::Missing => vec![],
        Mode::WrongIdentity => vec![
            nepl3_wire::schema::encode(
                &SchemaDescriptor {
                    package: "test.unrequested".into(),
                    revision: 1,
                    types: vec![NamedType {
                        name: "Other".into(),
                        shape: TypeShape::Record {
                            fields: vec![FieldDescriptor {
                                name: "value".into(),
                                ty: TypeDescriptor::U64,
                            }],
                        },
                        constraints: vec![],
                    }],
                    operations: vec![],
                },
                &registry,
                &mut budget(),
            )
            .map_err(error)?,
        ],
    };
    connection
        .send(
            &ProviderFrame::SchemaReply { descriptors },
            &registry,
            &sources,
            &mut admission,
            &mut budget(),
        )
        .map_err(error)?;
    // Read raw input after the bad response: even one subsequent byte fails.
    let (mut input, _) = connection.into_parts();
    let mut byte = [0];
    if input.read(&mut byte).map_err(error)? != 0 {
        return Err("host sent an operation after rejected schema".into());
    }
    Ok(())
}
pub(super) fn run() -> Result<(), String> {
    for mode in [Mode::Missing, Mode::WrongIdentity, Mode::Eof] {
        run_process(mode.argument(), move |mut connection| {
            let (_, request) = fixture()?;
            let result = connection.request_schemas(
                bootstrap()?,
                &[request.operation.schema],
                &mut budget(),
            );
            let expected = matches!(
                (&mode, &result),
                (
                    Mode::Missing,
                    Err(SchemaExchangeError::Admission(SchemaAdmissionError::Count)),
                ) | (
                    Mode::WrongIdentity,
                    Err(SchemaExchangeError::Admission(SchemaAdmissionError::Wire(
                        nepl3_wire::WireError::Schema(SchemaError::IdentityMismatch)
                    ))),
                ) | (Mode::Eof, Err(SchemaExchangeError::UnexpectedFrame))
            );
            if !expected || !connection.is_closed() {
                return Err(format!("unexpected schema failure: {result:?}"));
            }
            Ok(())
        })?;
    }
    Ok(())
}
