//! Host byte-stream adapter for the portable provider protocol.
//! Operation dispatch, schema negotiation and request lifetimes are supplied by
//! the host. Blocking stream implementations must supply their own deadline and
//! interruption mechanism; Budget is checked between I/O calls.
pub mod control;
pub mod delegation;
pub mod dispatch;
#[cfg(not(target_family = "wasm"))]
pub mod process;
pub mod reply;
pub mod schema;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    operation::ProviderFrame,
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceStore},
    value_codec::FoundationCodecError,
};
use nepl3_wire::WireError;
use std::io::{self, Read, Write};

#[derive(Debug)]
pub enum TransportError {
    ProtocolState,
    Io(io::Error),
    Wire(WireError),
    Stopped(StopReason),
    Truncated,
    Closed,
}
impl From<WireError> for TransportError {
    fn from(error: WireError) -> Self {
        match error.stop_reason() {
            Some(reason) => Self::Stopped(reason),
            None => Self::Wire(error),
        }
    }
}
impl From<StopReason> for TransportError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// A failed or partially transferred frame terminates this transport. Retrying
/// its byte suffix as a new frame would lose message alignment. Clean EOF also
/// closes it; the host cancels remaining requests when it observes either case.
pub struct Connection<R, W> {
    reader: R,
    writer: W,
    closed: bool,
    phase: Phase,
}
#[derive(Clone, Copy)]
enum Phase {
    Fresh,
    RequestSent,
    RequestReceived,
    Ready,
}
impl<R: Read, W: Write> Connection<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            closed: false,
            phase: Phase::Fresh,
        }
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn into_parts(self) -> (R, W) {
        (self.reader, self.writer)
    }
    fn operation_phase(&mut self) -> Result<(), TransportError> {
        if self.closed {
            return Err(TransportError::Closed);
        }
        if matches!(self.phase, Phase::RequestSent | Phase::RequestReceived) {
            self.closed = true;
            return Err(TransportError::ProtocolState);
        }
        Ok(())
    }

    pub fn receive(
        &mut self,
        registry: &SchemaRegistry,
        sources: &SourceStore,
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<Option<ProviderFrame>, TransportError> {
        if self.closed {
            return Err(TransportError::Closed);
        }
        let result = self.receive_inner(registry, sources, admission, b);
        if !matches!(result, Ok(Some(_))) || matches!(result, Ok(Some(ProviderFrame::Close))) {
            self.closed = true;
        }
        result
    }
    fn receive_inner(
        &mut self,
        registry: &SchemaRegistry,
        sources: &SourceStore,
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<Option<ProviderFrame>, TransportError> {
        let mut header = [0; 8];
        let count = fill(&mut self.reader, &mut header, b)?;
        if count == 0 {
            return Ok(None);
        }
        if count != header.len() {
            return Err(TransportError::Truncated);
        }
        let payload = nepl3_wire::frame::payload_length(&header, b)?;
        let total = payload.checked_add(8).ok_or(WireError::InvalidLength)?;
        b.charge(Resource::AllocationUnits, total as u64)?;
        b.charge(Resource::Work, total as u64)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        bytes.resize(total, 0);
        bytes[..8].copy_from_slice(&header);
        if fill(&mut self.reader, &mut bytes[8..], b)? != payload {
            return Err(TransportError::Truncated);
        }
        let (frame, suffix) =
            nepl3_wire::operation::decode_frame(&bytes, true, registry, sources, admission, b)?
                .ok_or(TransportError::Truncated)?;
        if !suffix.is_empty() {
            return Err(WireError::InvalidLength.into());
        }
        self.advance(&frame, true)?;
        Ok(Some(frame))
    }
    pub fn send(
        &mut self,
        frame: &ProviderFrame,
        registry: &SchemaRegistry,
        sources: &SourceStore,
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<(), TransportError> {
        if self.closed {
            return Err(TransportError::Closed);
        }
        let result = (|| {
            self.advance(frame, false)?;
            let bytes =
                nepl3_wire::operation::encode_frame(frame, registry, sources, admission, b)?;
            let mut rest = bytes.as_slice();
            while !rest.is_empty() {
                b.charge(Resource::Work, 1)?;
                match self.writer.write(rest) {
                    Ok(0) => return Err(TransportError::Io(io::ErrorKind::WriteZero.into())),
                    Ok(count) => rest = &rest[count..],
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(TransportError::Io(error)),
                }
            }
            loop {
                b.charge(Resource::Work, 1)?;
                match self.writer.flush() {
                    Ok(()) => break,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(TransportError::Io(error)),
                }
            }
            Ok(())
        })();
        if result.is_err() || matches!(frame, ProviderFrame::Close) {
            self.closed = true;
        }
        result
    }

    fn advance(&mut self, frame: &ProviderFrame, incoming: bool) -> Result<(), TransportError> {
        self.phase = match (self.phase, frame, incoming) {
            (_, ProviderFrame::Close, _) => Phase::Ready,
            (Phase::Fresh, ProviderFrame::SchemaRequest { .. }, false) => Phase::RequestSent,
            (Phase::Fresh, ProviderFrame::SchemaRequest { .. }, true) => Phase::RequestReceived,
            (Phase::RequestSent, ProviderFrame::SchemaReply { .. }, true) => Phase::Ready,
            (Phase::RequestReceived, ProviderFrame::SchemaReply { .. }, false) => Phase::Ready,
            (_, ProviderFrame::SchemaRequest { .. } | ProviderFrame::SchemaReply { .. }, _) => {
                return Err(TransportError::ProtocolState);
            }
            (Phase::Fresh | Phase::Ready, _, _) => Phase::Ready,
            _ => return Err(TransportError::ProtocolState),
        };
        Ok(())
    }
}
fn fill(reader: &mut impl Read, bytes: &mut [u8], b: &mut Budget) -> Result<usize, TransportError> {
    let mut filled = 0;
    while filled < bytes.len() {
        b.charge(Resource::Work, 1)?;
        match reader.read(&mut bytes[filled..]) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(TransportError::Io(error)),
        }
    }
    Ok(filled)
}
