//! Connect transport termination and peer control to host request lifetimes.
use crate::*;
use nepl3_core::operation::lifetime::{LifetimeError, RequestLifetimes};

#[derive(Debug)]
pub enum ControlError {
    Transport(TransportError),
    Lifetime(LifetimeError),
}

impl<R: Read, W: Write> Connection<R, W> {
    /// Receive a frame and apply its control effects to this connection's table.
    /// Invoke/Resume/Reply are returned for host admission and dispatch. Cancel
    /// atomically cancels its active subtree before returning the control frame.
    /// Close, EOF and any receive/control failure cancel all remaining requests.
    ///
    /// `lifetimes` must contain only requests owned by this connection. The
    /// callback signals their execution adapters to interrupt work; it must not
    /// block or panic. Fallback closure is allocation-free and remains available
    /// when the control budget has stopped. Requests already terminal receive no
    /// additional notification. The host owns process termination and reaping.
    #[allow(clippy::too_many_arguments)]
    pub fn receive_managed(
        &mut self,
        lifetimes: &mut RequestLifetimes,
        registry: &SchemaRegistry,
        sources: &SourceStore,
        admission: &mut SourceAdmission,
        transport: &mut Budget,
        control: &mut Budget,
        mut cancel: impl FnMut(u64),
    ) -> Result<Option<ProviderFrame>, ControlError> {
        let result = (|| {
            let frame = self
                .receive(registry, sources, admission, transport)
                .map_err(ControlError::Transport)?;
            match &frame {
                Some(ProviderFrame::Cancel { request_id }) => lifetimes
                    .cancel_tree(*request_id, control, &mut cancel)
                    .map_err(ControlError::Lifetime)?,
                Some(ProviderFrame::Close) | None => lifetimes.close(&mut cancel),
                _ => {}
            }
            Ok(frame)
        })();
        if result.is_err() {
            self.closed = true;
            lifetimes.close(cancel);
        }
        result
    }
}
