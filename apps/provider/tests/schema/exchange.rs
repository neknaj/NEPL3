use super::*;
use nepl3_core::{
    operation::ProviderFrame,
    source::{SourceAdmission, SourceStore},
};
use nepl3_provider::{Connection, TransportError};
use std::io::Cursor;

fn frame(value: &ProviderFrame, registry: &SchemaRegistry) -> Result<Vec<u8>, String> {
    nepl3_wire::operation::encode_frame(
        value,
        registry,
        &SourceStore::default(),
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)
}

#[test]
fn setup_exchange_installs_only_requested_schema_and_runs_once() -> Result<(), String> {
    let mut catalog = bootstrap()?;
    let descriptor = descriptor("test.remote", TypeDescriptor::U64);
    let identity = descriptor.reference(&mut budget()).map_err(error)?;
    catalog
        .register(identity.clone(), descriptor, &mut budget())
        .map_err(error)?;
    catalog.finalize(&mut budget()).map_err(error)?;
    let request = ProviderFrame::SchemaRequest {
        schemas: vec![identity.clone()],
    };
    let request_bytes = frame(&request, &catalog)?;
    let mut server = Connection::new(Cursor::new(request_bytes.clone()), Vec::new());
    server
        .serve_schemas(&catalog, &mut budget())
        .map_err(error)?;
    let (_, reply_bytes) = server.into_parts();
    let mut client = Connection::new(Cursor::new(reply_bytes), Vec::new());
    let registry = client
        .request_schemas(
            bootstrap()?,
            core::slice::from_ref(&identity),
            &mut budget(),
        )
        .map_err(error)?;
    assert_eq!(registry.selected("test.remote", 1), Some(&identity));
    assert!(registry.is_finalized());
    assert!(!client.is_closed());
    assert!(
        client
            .request_schemas(bootstrap()?, &[identity], &mut budget())
            .is_err()
    );
    assert!(client.is_closed());
    // The wire request matches the independent host selection byte-for-byte.
    let (_, written) = client.into_parts();
    assert_eq!(written, request_bytes);
    Ok(())
}

#[test]
fn unsolicited_missing_wrong_and_interleaved_messages_close_connection() -> Result<(), String> {
    let base = bootstrap()?;
    let identity = descriptor("test.remote", TypeDescriptor::U64)
        .reference(&mut budget())
        .map_err(error)?;
    let response = ProviderFrame::SchemaReply {
        descriptors: vec![],
    };
    let mut unsolicited = Connection::new(Cursor::new(frame(&response, &base)?), Vec::new());
    assert!(matches!(
        unsolicited.receive(
            &base,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(TransportError::ProtocolState)
    ));
    assert!(unsolicited.is_closed());
    for response in [
        response,
        ProviderFrame::Cancel { request_id: 7 },
        ProviderFrame::Close,
    ] {
        let mut client = Connection::new(Cursor::new(frame(&response, &base)?), Vec::new());
        assert!(
            client
                .request_schemas(
                    bootstrap()?,
                    core::slice::from_ref(&identity),
                    &mut budget()
                )
                .is_err()
        );
        assert!(client.is_closed());
    }
    let wrong = descriptor("test.remote", TypeDescriptor::Bool);
    let payload = nepl3_wire::schema::encode(&wrong, &base, &mut budget()).map_err(error)?;
    let mut client = Connection::new(
        Cursor::new(frame(
            &ProviderFrame::SchemaReply {
                descriptors: vec![payload],
            },
            &base,
        )?),
        Vec::new(),
    );
    assert!(
        client
            .request_schemas(
                bootstrap()?,
                core::slice::from_ref(&identity),
                &mut budget()
            )
            .is_err()
    );
    assert!(client.is_closed());
    let mut server = Connection::new(
        Cursor::new(frame(
            &ProviderFrame::SchemaRequest {
                schemas: vec![identity],
            },
            &base,
        )?),
        Vec::new(),
    );
    assert!(server.serve_schemas(&base, &mut budget()).is_err());
    assert!(server.is_closed());
    Ok(())
}
