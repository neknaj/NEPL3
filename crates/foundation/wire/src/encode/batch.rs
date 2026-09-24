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
    states: Vec<State>,
    active: Vec<usize>,
    scopes: Vec<usize>,
}

fn storage<T>(count: usize, b: &mut Budget) -> Result<Vec<T>, WireError> {
    let bytes = count
        .checked_mul(core::mem::size_of::<T>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    Ok(Vec::with_capacity(count))
}

fn bound(
    index: &[(usize, usize)],
    address: usize,
    upper: bool,
    b: &mut Budget,
) -> Result<usize, WireError> {
    // Charge the element-count search bound before comparisons. Pointer order
    // varies with allocation layout; logical Usage must not depend on it.
    b.charge(
        Resource::Work,
        (usize::BITS - index.len().leading_zeros()) as u64,
    )?;
    let (mut start, mut end) = (0, index.len());
    while start < end {
        let middle = start + (end - start) / 2;
        if index[middle].0 < address || (upper && index[middle].0 == address) {
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
        let address = core::ptr::from_ref(value).addr();
        let start = bound(&self.index, address, false, b)?;
        let end = bound(&self.index, address, true, b)?;
        let mut count = 0;
        for &(_, request) in &self.index[start..end] {
            b.charge(Resource::Work, 1)?;
            let state = &mut self.states[request];
            if state.digest.is_some() {
                continue;
            }
            if state.hash.is_some() {
                return Err(WireError::InvalidType);
            }
            let domain = self.inputs[request].domain;
            b.charge(Resource::Work, domain.len() as u64)?;
            let mut hash = Sha256::new();
            hash.update(domain);
            state.hash = Some(hash);
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
        // One encoding traversal feeds every active enclosing/selected hash.
        b.charge(Resource::Work, bytes.len() as u64)?;
        for &request in &self.active {
            b.charge(Resource::Work, 1)?;
            b.charge(Resource::Work, bytes.len() as u64)?;
            self.states[request]
                .hash
                .as_mut()
                .ok_or(WireError::InvalidType)?
                .update(bytes);
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
        states: storage(count, b)?,
        active: storage(count, b)?,
        scopes: storage(count, b)?,
    };
    let mut result = storage(count, b)?;
    for (request, input) in inputs.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        b.charge(Resource::OutputBytes, 32)?;
        let address = core::ptr::from_ref(input.value).addr();
        batch.index.push((address, request));
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
