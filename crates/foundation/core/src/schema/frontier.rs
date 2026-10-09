//! Small validation frontiers stay on the stack; wider inputs spill with a budget.
use crate::budget::{Budget, Resource, StopReason};
use alloc::vec::Vec;

pub(super) struct Frontier<T: Copy> {
    inline: [Option<T>; 8],
    len: usize,
    spill: Vec<T>,
    spill_slots: usize,
}
impl<T: Copy> Frontier<T> {
    pub(super) fn new(first: T) -> Self {
        let mut inline = [None; 8];
        inline[0] = Some(first);
        Self {
            inline,
            len: 1,
            spill: Vec::new(),
            spill_slots: 0,
        }
    }
    pub(super) fn pop(&mut self) -> Option<T> {
        if let Some(value) = self.spill.pop() {
            return Some(value);
        }
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        self.inline[self.len].take()
    }
    pub(super) fn push(&mut self, value: T, budget: &mut Budget) -> Result<(), StopReason> {
        budget.poll()?;
        if self.len < self.inline.len() {
            self.inline[self.len] = Some(value);
            self.len += 1;
            return Ok(());
        }
        if self.spill.len() == self.spill_slots {
            let next = if self.spill_slots == 0 {
                8
            } else {
                self.spill_slots
                    .checked_mul(2)
                    .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?
            };
            let bytes = (next - self.spill_slots)
                .checked_mul(core::mem::size_of::<T>())
                .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
            budget.charge(Resource::AllocationUnits, bytes as u64)?;
            self.spill
                .try_reserve_exact(next - self.spill.len())
                .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
            self.spill_slots = next;
        }
        self.spill.push(value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Limits;
    #[test]
    fn drain_refill_preserves_lifo_and_reuses_spill_capacity() -> Result<(), StopReason> {
        let mut b = Budget::new(Limits {
            allocation_units: 1024,
            ..Limits::default()
        });
        let mut f = Frontier::new(0usize);
        for i in 1..17 {
            f.push(i, &mut b)?;
        }
        let reserved = b.usage().allocation_units;
        for i in (0..17).rev() {
            assert_eq!(f.pop(), Some(i));
        }
        assert_eq!(f.pop(), None);
        for i in 20..37 {
            f.push(i, &mut b)?;
        }
        assert_eq!(b.usage().allocation_units, reserved);
        for i in (20..37).rev() {
            assert_eq!(f.pop(), Some(i));
        }
        assert_eq!(f.pop(), None);
        Ok(())
    }
}
