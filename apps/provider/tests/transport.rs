use nepl3_core::{budget::*, operation::ProviderFrame, schema::*, source::*};
use nepl3_provider::{Connection, TransportError};
use std::io::{self, Cursor, Read, Write};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 100_000,
        work: 10_000_000,
        depth: 128,
        nodes: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 100_000,
        diagnostics: 100,
        events: 100,
    })
}
fn registry() -> Result<SchemaRegistry, String> {
    let descriptor = foundation::descriptor(&mut budget()).map_err(|e| format!("{e:?}"))?;
    let schema = descriptor
        .reference(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema, descriptor, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    Ok(registry)
}
struct Chunks<T>(T);
impl<T: Read> Read for Chunks<T> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let len = bytes.len().min(3);
        self.0.read(&mut bytes[..len])
    }
}
impl<T: Write> Write for Chunks<T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(&bytes[..bytes.len().min(3)])
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[test]
fn chunked_stream_preserves_messages_and_close_terminates_transport() -> Result<(), String> {
    let registry = registry()?;
    let sources = SourceStore::default();
    let mut sender = Connection::new(io::empty(), Chunks(Vec::new()));
    for frame in [
        ProviderFrame::Cancel { request_id: 17 },
        ProviderFrame::Close,
    ] {
        sender
            .send(
                &frame,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
    }
    assert!(sender.is_closed());
    assert!(matches!(
        sender.send(
            &ProviderFrame::Close,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(TransportError::Closed)
    ));
    let (_, bytes) = sender.into_parts();
    let mut receiver = Connection::new(Chunks(Cursor::new(bytes.0)), io::sink());
    for expected in [
        ProviderFrame::Cancel { request_id: 17 },
        ProviderFrame::Close,
    ] {
        let frame = receiver
            .receive(
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(frame, Some(expected));
    }
    assert!(receiver.is_closed());
    Ok(())
}

#[test]
fn clean_eof_and_truncated_frames_are_distinct_and_length_is_checked_first() -> Result<(), String> {
    let registry = registry()?;
    let sources = SourceStore::default();
    let bytes = nepl3_wire::operation::encode_frame(
        &ProviderFrame::Cancel { request_id: 17 },
        &registry,
        &sources,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    for end in 0..bytes.len() {
        let mut receiver = Connection::new(Cursor::new(&bytes[..end]), io::sink());
        let result = receiver.receive(
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        );
        if end == 0 {
            assert!(matches!(result, Ok(None)));
        } else {
            assert!(matches!(result, Err(TransportError::Truncated)));
        }
        assert!(receiver.is_closed());
    }
    let mut receiver = Connection::new(Cursor::new(1000u64.to_be_bytes()), io::sink());
    let mut limited = Budget::new(Limits {
        source_bytes: 8,
        ..budget().limits()
    });
    assert!(matches!(
        receiver.receive(
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut limited
        ),
        Err(TransportError::Stopped(StopReason::SourceLimit))
    ));
    assert_eq!(limited.poll(), Err(StopReason::SourceLimit));
    assert_eq!(limited.usage().allocation_units, 0);
    Ok(())
}

#[test]
fn codec_stops_are_normalized_before_transport_output() -> Result<(), String> {
    let registry = registry()?;
    let mut sender = Connection::new(io::empty(), Vec::new());
    let mut limited = Budget::new(Limits {
        output_bytes: 0,
        ..budget().limits()
    });
    assert!(matches!(
        sender.send(
            &ProviderFrame::Close,
            &registry,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut limited
        ),
        Err(TransportError::Stopped(StopReason::OutputLimit))
    ));
    assert!(sender.is_closed());
    assert!(sender.into_parts().1.is_empty());
    // Schema validation can report a stop through a nested codec error.
    let nested = nepl3_wire::WireError::Schema(SchemaError::Stopped(StopReason::WorkLimit));
    assert!(matches!(
        TransportError::from(nested),
        TransportError::Stopped(StopReason::WorkLimit)
    ));
    assert!(matches!(
        TransportError::from(nepl3_wire::WireError::InvalidLength),
        TransportError::Wire(nepl3_wire::WireError::InvalidLength)
    ));
    Ok(())
}

struct Broken;
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
struct Interrupted;
impl Read for Interrupted {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::Interrupted.into())
    }
}
#[test]
fn io_failure_and_interrupted_read_budget_stop_close_the_connection() -> Result<(), String> {
    let registry = registry()?;
    let sources = SourceStore::default();
    let mut broken = Connection::new(io::empty(), Broken);
    assert!(matches!(
        broken.send(
            &ProviderFrame::Close,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(TransportError::Io(_))
    ));
    assert!(broken.is_closed());
    let mut interrupted = Connection::new(Interrupted, io::sink());
    let mut limited = Budget::new(Limits {
        work: 3,
        ..budget().limits()
    });
    assert!(matches!(
        interrupted.receive(
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut limited
        ),
        Err(TransportError::Stopped(StopReason::WorkLimit))
    ));
    assert!(interrupted.is_closed());
    Ok(())
}
