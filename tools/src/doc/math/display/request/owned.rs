//! Sealed preparation ownership, not a launch or reply-admission capability.
use super::{Controls, Error, PreparedDisplay, PreparedRequest};
use nepl3_core::budget::Budget;

pub(in crate::doc::math::display) struct OwnedRequest {
    owner: PreparedDisplay,
    controls: Controls,
    bytes: Vec<u8>,
}
pub(in crate::doc::math::display) enum Preparation {
    Ready(OwnedRequest),
    NotRequested(PreparedDisplay),
    Rejected {
        owner: PreparedDisplay,
        error: Error,
    },
}
pub(in crate::doc::math::display) fn prepare(
    owner: PreparedDisplay,
    controls: Controls,
    cap: usize,
    b: &mut Budget,
) -> Preparation {
    // Move the original encoded buffer out before moving its borrowed owner.
    let input = super::prepare(&owner, controls, cap, b)
        .map(|request| request.map(|request| (request.controls, request.bytes)));
    match input {
        Ok(Some((controls, bytes))) => Preparation::Ready(OwnedRequest {
            owner,
            controls,
            bytes,
        }),
        Ok(None) => Preparation::NotRequested(owner),
        Err(error) => Preparation::Rejected { owner, error },
    }
}
impl OwnedRequest {
    pub(in crate::doc::math::display) fn wire(&self) -> &[u8] {
        &self.bytes
    }
    pub(in crate::doc::math::display) fn controls(&self) -> Controls {
        self.controls
    }

    /// Consume this exact preparation once. R cannot borrow the temporary
    /// request, but may retain unrelated external asset/config lifetimes.
    /// Moving the original Vec neither encodes again nor copies its buffer.
    pub(in crate::doc::math::display) fn consume<R>(
        self,
        f: impl for<'a> FnOnce(&'a PreparedRequest<'a>) -> R,
    ) -> (PreparedDisplay, R) {
        let Self {
            owner,
            controls,
            bytes,
        } = self;
        let result = {
            let request = PreparedRequest {
                owner: &owner,
                controls,
                bytes,
            };
            f(&request)
        };
        (owner, result)
    }
}

#[cfg(test)]
#[path = "owned_tests.rs"]
mod tests;
