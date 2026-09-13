//! Shared bounded output storage for the HTML and MathML serializers.
use alloc::string::String;
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Default)]
pub(crate) struct Output(String);
impl Output {
    pub(crate) fn literal(&mut self, text: &str, b: &mut Budget) -> Result<(), StopReason> {
        b.charge(Resource::OutputBytes, text.len() as u64)?;
        self.precharged(text, b)
    }

    /// `text::escape` already charged these output bytes. This method charges
    /// storage and copy work, without charging the same emitted bytes twice.
    pub(crate) fn precharged(&mut self, text: &str, b: &mut Budget) -> Result<(), StopReason> {
        b.charge(Resource::Work, text.len() as u64)?;
        let len = self
            .0
            .len()
            .checked_add(text.len())
            .filter(|len| *len <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        if len > self.0.capacity() {
            let capacity = self
                .0
                .capacity()
                .saturating_mul(2)
                .max(len)
                .max(32)
                .min(isize::MAX as usize);
            b.charge(
                Resource::AllocationUnits,
                (capacity - self.0.capacity()) as u64,
            )?;
            b.charge(Resource::Work, self.0.len() as u64)?;
            self.0
                .try_reserve_exact(capacity - self.0.len())
                .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        }
        self.0.push_str(text);
        Ok(())
    }

    pub(crate) fn finish(self) -> String {
        self.0
    }
}
