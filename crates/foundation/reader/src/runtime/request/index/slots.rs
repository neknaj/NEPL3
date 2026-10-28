//! Compact positions with a metered wide fallback; bucket addresses stay usize.
use alloc::vec::Vec;
use nepl3_core::budget::{Budget, Resource, StopReason};

pub(super) enum Slots {
    Compact(Vec<u16>),
    Wide(Vec<usize>),
}
impl Slots {
    pub(super) fn new() -> Self {
        if usize::BITS > u16::BITS {
            Self::Compact(Vec::new())
        } else {
            Self::Wide(Vec::new())
        }
    }
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Compact(v) => v.len(),
            Self::Wide(v) => v.len(),
        }
    }
    pub(super) fn get(&self, at: usize) -> Option<usize> {
        match self {
            Self::Compact(v) => (v[at] != u16::MAX).then_some(v[at] as usize),
            Self::Wide(v) => (v[at] != usize::MAX).then_some(v[at]),
        }
    }
    pub(super) fn set(
        &mut self,
        at: usize,
        position: usize,
        b: &mut Budget,
    ) -> Result<(), StopReason> {
        match self {
            Self::Compact(v) => {
                let position = u16::try_from(position)
                    .ok()
                    .filter(|&n| n != u16::MAX)
                    .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
                v[at] = position;
            }
            Self::Wide(v) => {
                if position == usize::MAX {
                    return Err(b.stop(StopReason::AllocationLimit));
                }
                v[at] = position;
            }
        }
        Ok(())
    }
    pub(super) fn ensure_count(&mut self, count: usize, b: &mut Budget) -> Result<(), StopReason> {
        if count > u16::MAX as usize {
            self.widen(b)?;
        }
        Ok(())
    }
    // Widening is a fresh buffer, not same-Vec growth: charge its full storage.
    fn widen(&mut self, b: &mut Budget) -> Result<(), StopReason> {
        let Self::Compact(old) = self else {
            return Ok(());
        };
        let bytes = old
            .len()
            .checked_mul(core::mem::size_of::<usize>())
            .filter(|&n| n <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        b.charge(Resource::Work, old.len() as u64)?;
        let mut wide = Vec::new();
        wide.try_reserve_exact(old.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        for &position in old.iter() {
            wide.push(if position == u16::MAX {
                usize::MAX
            } else {
                position as usize
            });
        }
        *self = Self::Wide(wide);
        Ok(())
    }
    pub(super) fn grow_and_reset(
        &mut self,
        length: usize,
        b: &mut Budget,
    ) -> Result<(), StopReason> {
        let old_length = self.len();
        let width = match self {
            Self::Compact(_) => core::mem::size_of::<u16>(),
            Self::Wide(_) => core::mem::size_of::<usize>(),
        };
        let bytes = length
            .checked_mul(width)
            .filter(|&n| n <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        let additional = length - old_length;
        b.charge(
            Resource::AllocationUnits,
            (bytes - old_length * width) as u64,
        )?;
        b.charge(Resource::Work, old_length as u64)?;
        b.charge(Resource::Work, length as u64)?;
        match self {
            Self::Compact(v) => {
                v.try_reserve_exact(additional)
                    .map_err(|_| b.stop(StopReason::AllocationLimit))?;
                v.resize(length, u16::MAX);
                v[..old_length].fill(u16::MAX);
            }
            Self::Wide(v) => {
                v.try_reserve_exact(additional)
                    .map_err(|_| b.stop(StopReason::AllocationLimit))?;
                v.resize(length, usize::MAX);
                v[..old_length].fill(usize::MAX);
            }
        }
        Ok(())
    }
}

#[cfg(all(test, any(target_pointer_width = "32", target_pointer_width = "64")))]
mod tests {
    use super::*;
    use alloc::vec;
    use nepl3_core::budget::Limits;
    fn limits() -> Limits {
        Limits {
            source_bytes: 1000,
            work: 1000,
            depth: 10,
            nodes: 1000,
            allocation_units: 1000,
            output_bytes: 1000,
            diagnostics: 10,
            events: 10,
        }
    }
    #[test]
    fn widening_preserves_empty_slots_positions_and_exact_stops() -> Result<(), StopReason> {
        let initial = vec![u16::MAX, 0, u16::MAX - 1, 7, u16::MAX];
        let wide_bytes = (5 * core::mem::size_of::<usize>()) as u64;
        for (work, allocation, reason) in [
            (5, wide_bytes, None),
            (4, wide_bytes, Some(StopReason::WorkLimit)),
            (5, wide_bytes - 1, Some(StopReason::AllocationLimit)),
        ] {
            let mut slots = Slots::Compact(initial.clone());
            let mut b = Budget::new(Limits {
                work,
                allocation_units: allocation,
                ..limits()
            });
            let result = slots.ensure_count(u16::MAX as usize + 1, &mut b);
            if let Some(reason) = reason {
                assert_eq!(result, Err(reason));
                assert_eq!(b.poll(), Err(reason));
                assert!(matches!(&slots, Slots::Compact(v) if v == &initial));
            } else {
                result?;
                assert!(matches!(slots, Slots::Wide(_)));
                assert_eq!(
                    (0..5).map(|i| slots.get(i)).collect::<Vec<_>>(),
                    vec![None, Some(0), Some(u16::MAX as usize - 1), Some(7), None]
                );
                assert_eq!(b.usage().work, 5);
                assert_eq!(b.usage().allocation_units, wide_bytes);
            }
        }
        let mut slots = Slots::Compact(initial);
        slots.ensure_count(u16::MAX as usize, &mut Budget::new(limits()))?;
        assert!(matches!(slots, Slots::Compact(_)));
        let mut b = Budget::new(limits());
        assert_eq!(
            slots.set(0, u16::MAX as usize, &mut b),
            Err(StopReason::AllocationLimit)
        );
        assert_eq!(slots.get(0), None);
        Ok(())
    }
}
