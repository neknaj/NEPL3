//! Collision-safe operation-local lookup. Every slot, probe and rehash is metered.
mod slots;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::SourceSnapshot,
};
use slots::Slots;

pub(super) enum Lookup {
    Present,
    Vacant(Option<usize>),
}
pub(super) struct Index {
    slots: Slots,
}
impl Index {
    pub(super) fn new() -> Self {
        Self {
            slots: Slots::new(),
        }
    }
    pub(super) fn reserve_hint(
        &mut self,
        count: usize,
        budget: &mut Budget,
    ) -> Result<(), StopReason> {
        self.grow_for(count, &[], &SourceSnapshot::identity_lookup_hash, budget)?;
        Ok(())
    }
    pub(super) fn lookup(
        &self,
        values: &[SourceSnapshot],
        source: &SourceSnapshot,
        key: u64,
        hash: &impl Fn(&SourceSnapshot) -> u64,
        budget: &mut Budget,
    ) -> Result<Lookup, StopReason> {
        if self.slots.len() == 0 {
            return Ok(Lookup::Vacant(None));
        }
        let length = self.slots.len();
        let mut at = (key as usize) % length;
        for _ in 0..self.slots.len() {
            budget.charge(Resource::Work, 1)?;
            let Some(old) = self.slots.get(at) else {
                return Ok(Lookup::Vacant(Some(at)));
            };
            // The probe unit includes the slot and its fixed-u64 cached-key
            // filter. Matching keys still pay the full identity comparison.
            if hash(&values[old]) == key
                && values[old]
                    .identity()
                    .compare_with_budget(source.identity(), budget)?
                    .is_eq()
            {
                return Ok(Lookup::Present);
            }
            at = if at + 1 == length { 0 } else { at + 1 };
        }
        Ok(Lookup::Vacant(None))
    }
    pub(super) fn reserve_one(
        &mut self,
        values: &[SourceSnapshot],
        hash: &impl Fn(&SourceSnapshot) -> u64,
        budget: &mut Budget,
    ) -> Result<bool, StopReason> {
        let count = values
            .len()
            .checked_add(1)
            .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
        self.grow_for(count, values, hash, budget)
    }
    fn grow_for(
        &mut self,
        count: usize,
        values: &[SourceSnapshot],
        hash: &impl Fn(&SourceSnapshot) -> u64,
        budget: &mut Budget,
    ) -> Result<bool, StopReason> {
        self.slots.ensure_count(count, budget)?;
        let needed = count
            .checked_mul(3)
            .and_then(|n| n.checked_add(1))
            .map(|n| n / 2)
            .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
        if self.slots.len() >= needed {
            return Ok(false);
        }
        let doubled = self
            .slots
            .len()
            .checked_mul(2)
            .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
        let length = needed.max(doubled).max(4);
        self.slots.grow_and_reset(length, budget)?;
        // Any stopped rebuild aborts the entire private request builder.
        for (position, value) in values.iter().enumerate() {
            budget.charge(Resource::Work, 1)?;
            let at = self.vacant(hash(value), budget)?;
            self.insert_at(at, position, budget)?;
        }
        Ok(true)
    }
    // Growth guarantees a nonempty, at-most-two-thirds-full table. Every probe is
    // charged even under adversarial collisions; a hash never proves identity.
    pub(super) fn vacant(&self, key: u64, budget: &mut Budget) -> Result<usize, StopReason> {
        let length = self.slots.len();
        let mut at = (key as usize) % length;
        loop {
            budget.charge(Resource::Work, 1)?;
            if self.slots.get(at).is_none() {
                return Ok(at);
            }
            at = if at + 1 == length { 0 } else { at + 1 };
        }
    }
    pub(super) fn insert_at(
        &mut self,
        at: usize,
        position: usize,
        budget: &mut Budget,
    ) -> Result<(), StopReason> {
        self.slots.set(at, position, budget)
    }
}

#[cfg(all(test, any(target_pointer_width = "32", target_pointer_width = "64")))]
mod tests {
    use super::*;
    use alloc::{format, vec::Vec};
    use nepl3_core::{
        budget::Limits,
        source::{SourceError, SourceId},
    };
    fn budget() -> Budget {
        Budget::new(Limits {
            source_bytes: 10000,
            work: 2_000_000,
            depth: 100,
            nodes: 10000,
            allocation_units: 2_000_000,
            output_bytes: 10000,
            diagnostics: 10,
            events: 10,
        })
    }
    #[test]
    fn promoted_index_keeps_wrapped_lookup_and_rehashes_after_growth() -> Result<(), SourceError> {
        let values = (0..3)
            .map(|i| {
                SourceSnapshot::new(
                    SourceId(format!("s{i}")),
                    0,
                    "memory:source".into(),
                    b"x".to_vec(),
                    &mut budget(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let missing = SourceSnapshot::new(
            SourceId("missing".into()),
            0,
            "memory:source".into(),
            b"x".to_vec(),
            &mut budget(),
        )?;
        let hash = |_: &SourceSnapshot| 4;
        let mut index = Index::new();
        let mut b = budget();
        index.reserve_hint(3, &mut b)?;
        for position in 0..3 {
            let at = index.vacant(4, &mut b)?;
            index.insert_at(at, position, &mut b)?;
        }
        assert_eq!(index.slots.len(), 5);
        index.slots.ensure_count(u16::MAX as usize + 1, &mut b)?;
        assert!(matches!(index.slots, Slots::Wide(_)));
        for value in &values {
            assert!(matches!(
                index.lookup(&values, value, 4, &hash, &mut b)?,
                Lookup::Present
            ));
        }
        assert!(matches!(
            index.lookup(&values, &missing, 4, &hash, &mut b)?,
            Lookup::Vacant(Some(2))
        ));
        assert!(index.grow_for(8, &values, &hash, &mut b)?);
        for value in &values {
            assert!(matches!(
                index.lookup(&values, value, 4, &hash, &mut b)?,
                Lookup::Present
            ));
        }
        assert!(matches!(
            index.lookup(&values, &missing, 4, &hash, &mut b)?,
            Lookup::Vacant(Some(7))
        ));
        // Width must change even when reserved bucket capacity is sufficient.
        // The few live positions are valid; count is a conservative upper bound.
        let mut index = Index::new();
        let mut b = budget();
        index.slots.grow_and_reset(100_000, &mut b)?;
        for position in 0..3 {
            let at = index.vacant(4, &mut b)?;
            index.insert_at(at, position, &mut b)?;
        }
        assert!(!index.grow_for(u16::MAX as usize + 1, &values, &hash, &mut b)?);
        assert!(matches!(index.slots, Slots::Wide(_)));
        for value in &values {
            assert!(matches!(
                index.lookup(&values, value, 4, &hash, &mut b)?,
                Lookup::Present
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod probe_tests {
    use super::*;
    use nepl3_core::{
        budget::Limits,
        source::{SourceError, SourceId},
    };
    fn limits() -> Limits {
        Limits {
            source_bytes: 10000,
            work: 10000,
            depth: 100,
            nodes: 10000,
            allocation_units: 10000,
            output_bytes: 10000,
            diagnostics: 10,
            events: 10,
        }
    }
    fn source(name: &str, uri: &str) -> Result<SourceSnapshot, SourceError> {
        SourceSnapshot::new(
            SourceId(name.into()),
            0,
            uri.into(),
            b"x".to_vec(),
            &mut Budget::new(limits()),
        )
    }
    #[test]
    fn bounded_probe_filters_cached_keys_but_identity_bytes_remain_charged()
    -> Result<(), SourceError> {
        let values = [source("s2", "memory:source")?];
        let other = source("s7", "memory:source")?;
        let equal = source("s2", "memory:other")?;
        let key = values[0].identity_lookup_hash();
        let other_key = other.identity_lookup_hash();
        assert_ne!(key, other_key);
        assert_eq!((key as usize) % 4, (other_key as usize) % 4);
        let mut setup = Budget::new(limits());
        let mut index = Index::new();
        index.reserve_hint(2, &mut setup)?;
        let at = index.vacant(key, &mut setup)?;
        index.insert_at(at, 0, &mut setup)?;
        let mut b = Budget::new(Limits {
            work: 2,
            allocation_units: 0,
            ..limits()
        });
        assert!(matches!(
            index.lookup(
                &values,
                &other,
                other_key,
                &SourceSnapshot::identity_lookup_hash,
                &mut b
            )?,
            Lookup::Vacant(Some(_))
        ));
        assert_eq!(
            b.usage().work,
            2,
            "occupied fixed-key filter plus empty probe"
        );
        let mut b = Budget::new(Limits {
            work: 2,
            allocation_units: 0,
            ..limits()
        });
        assert!(matches!(
            index.lookup(
                &values,
                &values[0],
                key,
                &SourceSnapshot::identity_lookup_hash,
                &mut b
            )?,
            Lookup::Present
        ));
        assert_eq!(b.usage().work, 2, "probe plus same-live-ID confirmation");
        let mut b = Budget::new(Limits {
            work: 44,
            allocation_units: 0,
            ..limits()
        });
        assert!(matches!(
            index.lookup(
                &values,
                &equal,
                key,
                &SourceSnapshot::identity_lookup_hash,
                &mut b
            )?,
            Lookup::Present
        ));
        assert_eq!(
            b.usage().work,
            44,
            "probe plus 2 UTF-8 bytes and 41 identity Work"
        );
        let mut b = Budget::new(Limits {
            work: 43,
            allocation_units: 0,
            ..limits()
        });
        assert!(matches!(
            index.lookup(
                &values,
                &equal,
                key,
                &SourceSnapshot::identity_lookup_hash,
                &mut b
            ),
            Err(StopReason::WorkLimit)
        ));
        assert_eq!(
            b.usage().work,
            2,
            "probe and first identity charge remain consumed"
        );
        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        // Forced same hash but different IDs must inspect bytes, then probe the empty slot.
        let mut b = Budget::new(Limits {
            work: 45,
            allocation_units: 0,
            ..limits()
        });
        assert!(matches!(
            index.lookup(&values, &other, key, &|_| key, &mut b)?,
            Lookup::Vacant(Some(_))
        ));
        assert_eq!(b.usage().work, 45);
        Ok(())
    }
}
