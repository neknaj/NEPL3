use super::{Sink, emit};
use crate::WireError;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::Digest,
    value::NdfValue,
    value_codec::CanonicalDigestInput,
};
use sha2::{Digest as _, Sha256};

#[cfg(test)]
mod tests;

struct State {
    hash: Option<Sha256>,
    digest: Option<Digest>,
}

struct Batch<'a, 'v> {
    inputs: &'a [CanonicalDigestInput<'v>],
    // Addresses identify live immutable references only within this call.
    // They never enter CBOR, hashes, schema data, or persistent cache state.
    index: Vec<(usize, usize)>,
    requested_kinds: u16,
    states: Vec<State>,
    active: Vec<usize>,
    scopes: Vec<usize>,
    // Canonical bytes pending for exactly the current active hash set. Flush
    // before entering/leaving a requested value so sibling bytes never leak
    // into a selected subtree's digest.
    bytes: Vec<u8>,
}

const CHUNK: usize = 1024;

// A requested value can only match a node of the same immutable NDF variant.
// This filter depends on logical input kinds, not allocation addresses. All
// values still pass through emit's complete intrinsic validation and encoding.
fn kind_bit(value: &NdfValue) -> u16 {
    match value {
        NdfValue::Unit => 1 << 0,
        NdfValue::Bool(_) => 1 << 1,
        NdfValue::U64(_) => 1 << 2,
        NdfValue::Integer(_) => 1 << 3,
        NdfValue::Rational(_) => 1 << 4,
        NdfValue::Text(_) => 1 << 5,
        NdfValue::Bytes(_) => 1 << 6,
        NdfValue::List(_) => 1 << 7,
        NdfValue::None => 1 << 8,
        NdfValue::Some(_) => 1 << 9,
        NdfValue::Record(_) => 1 << 10,
        NdfValue::Variant(_) => 1 << 11,
    }
}

impl Batch<'_, '_> {
    fn flush(&mut self, b: &mut Budget) -> Result<(), WireError> {
        if self.bytes.is_empty() {
            return Ok(());
        }
        for &request in &self.active {
            b.charge(Resource::Work, 1)?;
            b.charge(Resource::Work, self.bytes.len() as u64)?;
            self.states[request]
                .hash
                .as_mut()
                .ok_or(WireError::InvalidType)?
                .update(&self.bytes);
        }
        self.bytes.clear();
        Ok(())
    }
}

fn storage<T>(count: usize, b: &mut Budget) -> Result<Vec<T>, WireError> {
    let bytes = count
        .checked_mul(core::mem::size_of::<T>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    Ok(Vec::with_capacity(count))
}

fn bound(index: &[(usize, usize)], address: usize, b: &mut Budget) -> Result<usize, WireError> {
    // Charge the element-count search bound before comparisons. Pointer order
    // varies with allocation layout; logical Usage must not depend on it.
    b.charge(
        Resource::Work,
        (usize::BITS - index.len().leading_zeros()) as u64,
    )?;
    let (mut start, mut end) = (0, index.len());
    while start < end {
        let middle = start + (end - start) / 2;
        if index[middle].0 < address {
            start = middle + 1;
        } else {
            end = middle;
        }
    }
    Ok(start)
}

fn sort_index(index: &mut Vec<(usize, usize)>, b: &mut Budget) -> Result<(), WireError> {
    let count = index.len();
    if count < 2 {
        return Ok(());
    }
    let mut scratch = storage(count, b)?;
    let mut width = 1_usize;
    while width < count {
        scratch.clear();
        let mut start = 0_usize;
        while start < count {
            let middle = start.saturating_add(width).min(count);
            let end = middle.saturating_add(width).min(count);
            let (mut left, mut right) = (start, middle);
            while left < middle || right < end {
                // One comparison and one copied slot per output element.
                // Precharge even when a run is exhausted so pointer layout
                // cannot change logical Usage or the stopping boundary.
                b.charge(Resource::Work, 2)?;
                let position = if left < middle && (right == end || index[left].0 <= index[right].0)
                {
                    let position = left;
                    left += 1;
                    position
                } else {
                    let position = right;
                    right += 1;
                    position
                };
                scratch.push(index[position]);
            }
            start = end;
        }
        core::mem::swap(index, &mut scratch);
        width = width.saturating_mul(2);
    }
    Ok(())
}

impl Sink for Batch<'_, '_> {
    fn enter(&mut self, value: &NdfValue, b: &mut Budget) -> Result<bool, WireError> {
        b.charge(Resource::Work, 1)?;
        if self.requested_kinds & kind_bit(value) == 0 {
            return Ok(false);
        }
        let address = core::ptr::from_ref(value).addr();
        let mut position = bound(&self.index, address, b)?;
        let mut count = 0;
        loop {
            // Charge the final comparison even at the end of the index. The
            // address layout must not change Usage for an absent request.
            b.charge(Resource::Work, 1)?;
            let Some(&(candidate, request)) = self.index.get(position) else {
                break;
            };
            if candidate != address {
                break;
            }
            position += 1;
            b.charge(Resource::Work, 1)?;
            let state = &mut self.states[request];
            if state.digest.is_some() {
                continue;
            }
            if state.hash.is_some() {
                return Err(WireError::InvalidType);
            }
            if count == 0 {
                self.flush(b)?;
            }
            let domain = self.inputs[request].domain;
            b.charge(Resource::Work, domain.len() as u64)?;
            let mut hash = Sha256::new();
            hash.update(domain);
            self.states[request].hash = Some(hash);
            self.active.push(request);
            count += 1;
        }
        if count > 0 {
            b.charge(Resource::Work, 1)?;
            self.scopes.push(count);
        }
        Ok(count > 0)
    }

    fn leave(&mut self, b: &mut Budget) -> Result<(), WireError> {
        self.flush(b)?;
        let count = self.scopes.pop().ok_or(WireError::InvalidType)?;
        for _ in 0..count {
            b.charge(Resource::Work, 1)?;
            let request = self.active.pop().ok_or(WireError::InvalidType)?;
            let state = &mut self.states[request];
            let hash = state.hash.take().ok_or(WireError::InvalidType)?;
            state.digest = Some(Digest(hash.finalize().into()));
        }
        Ok(())
    }

    fn write(&mut self, bytes: &[u8], b: &mut Budget) -> Result<(), WireError> {
        // Charge each encoded byte before copying it. Hash work is charged at
        // flush, immediately before updating each active state. Enter/leave
        // flush before changing that set; a stopped flush publishes no digest.
        // Small encoder fragments therefore need no repeated active-state walk.
        b.charge(Resource::Work, bytes.len() as u64)?;
        let mut remaining = bytes;
        while !remaining.is_empty() {
            let count = remaining.len().min(CHUNK - self.bytes.len());
            self.bytes.extend_from_slice(&remaining[..count]);
            remaining = &remaining[count..];
            if self.bytes.len() == CHUNK {
                self.flush(b)?;
            }
        }
        Ok(())
    }
}

pub(crate) fn digests(
    inputs: &[CanonicalDigestInput<'_>],
    b: &mut Budget,
) -> Result<Vec<Digest>, WireError> {
    b.poll()?;
    let count = inputs.len();
    let mut batch = Batch {
        inputs,
        index: storage(count, b)?,
        requested_kinds: 0,
        states: storage(count, b)?,
        active: storage(count, b)?,
        scopes: storage(count, b)?,
        bytes: storage(if count == 0 { 0 } else { CHUNK }, b)?,
    };
    let mut result = storage(count, b)?;
    for (request, input) in inputs.iter().enumerate() {
        b.charge(Resource::Work, 2)?;
        b.charge(Resource::OutputBytes, 32)?;
        let address = core::ptr::from_ref(input.value).addr();
        batch.index.push((address, request));
        batch.requested_kinds |= kind_bit(input.value);
        batch.states.push(State {
            hash: None,
            digest: None,
        });
    }
    sort_index(&mut batch.index, b)?;
    for (request, input) in inputs.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        if batch.states[request].digest.is_none() {
            emit(input.value, &mut batch, b)?;
        }
    }
    for state in batch.states {
        b.charge(Resource::Work, 1)?;
        result.push(state.digest.ok_or(WireError::InvalidType)?);
    }
    Ok(result)
}
