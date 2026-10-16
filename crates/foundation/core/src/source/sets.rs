//! One admission ledger's last fully checked local declaration set.
use super::{SourceAdmission, SourceError, SourceSnapshot, SourceStore};
use crate::budget::{Budget, Resource, StopReason};
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum SourceSetError {
    Source(SourceError),
    Duplicate,
}
impl From<SourceError> for SourceSetError {
    fn from(e: SourceError) -> Self {
        Self::Source(e)
    }
}
impl From<StopReason> for SourceSetError {
    fn from(e: StopReason) -> Self {
        Self::Source(e.into())
    }
}
impl SourceAdmission {
    /// This ledger only accumulates admitted immutable snapshots. A retained
    /// local set is never replaced by the ledger's union. All consumer-specific
    /// origins, maps, tokens, nodes and environments still need validation.
    pub(crate) fn checked_source_set<'a>(
        &'a mut self,
        sources: &[SourceSnapshot],
        b: &mut Budget,
    ) -> Result<&'a SourceStore, SourceSetError> {
        b.poll()?;
        b.charge(Resource::Work, 1)?;
        let mut hit = false;
        if let Some(store) = &self.checked_set
            && store.snapshots().len() == sources.len()
        {
            hit = true;
            for (old, new) in store.snapshots().iter().zip(sources) {
                if !old.eq_with_budget(new, b)? {
                    hit = false;
                    break;
                }
            }
        }
        if !hit {
            let mut store = SourceStore::default();
            for source in sources {
                self.admit_existing(source, b)?;
                store.insert_unique_declaration(source, b)?;
            }
            // Only a fully admitted, duplicate-free set is reusable. All owned
            // snapshots and index storage were charged by insertion above.
            self.checked_set = Some(store);
        }
        self.checked_set
            .as_ref()
            .ok_or(SourceSetError::Source(SourceError::MissingSnapshot))
    }
}

impl SourceStore {
    // Local declarations must be unique even when the snapshots are equal.
    // Keep this separate from public idempotent insertion.
    fn insert_unique_declaration(
        &mut self,
        snapshot: &SourceSnapshot,
        budget: &mut Budget,
    ) -> Result<(), SourceSetError> {
        budget.poll()?;
        let position = super::source_index_position(
            &self.index,
            |i| &self.snapshots[i],
            snapshot,
            self.insertion_hint,
            budget,
        )?;
        let at = match position {
            Ok(_) => return Err(SourceSetError::Duplicate),
            Err(at) => at,
        };
        budget.charge(Resource::Work, (self.index.len() - at) as u64)?;
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<usize>() as u64,
        )?;
        let owned = snapshot.clone_with_budget(budget)?;
        self.index.insert(at, self.snapshots.len());
        self.snapshots.push(owned);
        self.insertion_hint = Some(at);
        self.invalidate_scope();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{budget::Limits, source::SourceId};
    use alloc::{format, vec::Vec};
    fn budget() -> Budget {
        Budget::new(Limits {
            source_bytes: 1_000_000,
            work: 1_000_000,
            depth: 100,
            nodes: 1_000_000,
            allocation_units: 1_000_000,
            output_bytes: 1_000_000,
            diagnostics: 100,
            events: 100,
        })
    }
    fn fixture_sources() -> Result<Vec<SourceSnapshot>, SourceError> {
        (0..8)
            .map(|i| {
                SourceSnapshot::new(
                    SourceId(format!("id-{i}")),
                    1,
                    format!("memory:{i}"),
                    format!("content-{i}").into_bytes(),
                    &mut budget(),
                )
            })
            .collect()
    }
    #[test]
    fn unique_insertion_revision_and_exact_limits() -> Result<(), SourceSetError> {
        let mut sources = fixture_sources()?;
        sources.push(SourceSnapshot::new(
            SourceId("id-0".into()),
            2,
            "memory:0".into(),
            b"revised".to_vec(),
            &mut budget(),
        )?);
        let mut measured = budget();
        let mut baseline = SourceStore::default();
        for source in &sources {
            baseline.insert_unique_declaration(source, &mut measured)?;
        }
        let usage = measured.usage();
        for (work, allocation, succeeds) in [
            (usage.work, usage.allocation_units, true),
            (usage.work - 1, usage.allocation_units, false),
            (usage.work, usage.allocation_units - 1, false),
        ] {
            let mut b = Budget::new(Limits {
                work,
                allocation_units: allocation,
                ..budget().limits()
            });
            let mut store = SourceStore::default();
            let result = sources.iter().try_for_each(|source| {
                let snapshots = store.snapshots.clone();
                let index = store.index.clone();
                let hint = store.insertion_hint;
                let result = store.insert_unique_declaration(source, &mut b);
                if result.is_err() {
                    assert_eq!(store.snapshots, snapshots);
                    assert_eq!(store.index, index);
                    assert_eq!(store.insertion_hint, hint);
                }
                result
            });
            assert_eq!(result.is_ok(), succeeds);
            if succeeds {
                assert_eq!(store.snapshots(), baseline.snapshots());
                assert_eq!(b.usage(), usage);
            } else {
                assert!(
                    store
                        .insert_unique_declaration(&sources[0], &mut b)
                        .is_err()
                );
            }
        }
        Ok(())
    }

    #[test]
    fn unique_insertion_matches_two_search_store() -> Result<(), SourceSetError> {
        for order in [
            [0, 1, 2, 3, 4, 5, 6, 7],
            [7, 6, 5, 4, 3, 2, 1, 0],
            [3, 0, 7, 2, 5, 1, 6, 4],
        ] {
            let sources = fixture_sources()?;
            let mut old = SourceStore::default();
            let mut new = SourceStore::default();
            let mut old_budget = budget();
            let mut new_budget = budget();
            for i in order {
                let source = &sources[i];
                assert!(
                    old.get_revision_with_budget(
                        &source.identity().source,
                        source.identity().revision,
                        &mut old_budget
                    )?
                    .is_none()
                );
                old.insert_ref_with_budget(source, &mut old_budget)?;
                new.insert_unique_declaration(source, &mut new_budget)?;
            }
            assert_eq!(old.snapshots(), new.snapshots());
            for source in &sources {
                assert_eq!(
                    old.get_revision_with_budget(
                        &source.identity().source,
                        source.identity().revision,
                        &mut budget()
                    )?,
                    new.get_revision_with_budget(
                        &source.identity().source,
                        source.identity().revision,
                        &mut budget()
                    )?
                );
            }
            assert!(new_budget.usage().work < old_budget.usage().work);
            assert_eq!(
                new_budget.usage().allocation_units,
                old_budget.usage().allocation_units
            );
            let before = new.snapshots().to_vec();
            assert_eq!(
                new.insert_unique_declaration(&sources[3], &mut budget()),
                Err(SourceSetError::Duplicate)
            );
            assert_eq!(new.snapshots(), before);
            // Public insertion remains idempotent for the same snapshot.
            new.insert_ref_with_budget(&sources[3], &mut budget())?;
            assert_eq!(new.snapshots(), before);
        }
        Ok(())
    }

    #[test]
    fn exact_local_sets_reuse_without_admitting_the_union() -> Result<(), SourceSetError> {
        let sources = fixture_sources()?;
        let mut admission = SourceAdmission::default();
        let mut cold = budget();
        admission.checked_source_set(&sources, &mut cold)?;
        let mut warm = budget();
        assert_eq!(
            admission
                .checked_source_set(&sources, &mut warm)?
                .snapshots(),
            sources.as_slice()
        );
        assert!(warm.usage().work < cold.usage().work);
        assert_eq!(warm.usage().allocation_units, 0);
        assert_eq!(warm.usage().source_bytes, 0);
        let independent = fixture_sources()?;
        assert_eq!(
            admission
                .checked_source_set(&independent, &mut budget())?
                .snapshots(),
            sources.as_slice()
        );
        let mut reordered = sources.clone();
        reordered.reverse();
        assert_eq!(
            admission
                .checked_source_set(&reordered, &mut budget())?
                .snapshots(),
            reordered.as_slice()
        );
        assert_eq!(
            admission
                .checked_source_set(&sources[..2], &mut budget())?
                .snapshots(),
            &sources[..2]
        );
        assert!(
            admission
                .checked_source_set(&[], &mut budget())?
                .snapshots()
                .is_empty()
        );
        assert!(
            admission
                .checked_source_set(&[], &mut budget())?
                .snapshots()
                .is_empty()
        );
        Ok(())
    }
    #[test]
    fn duplicate_conflict_and_failed_replacement_keep_prior_set() -> Result<(), SourceSetError> {
        let sources = fixture_sources()?;
        let mut admission = SourceAdmission::default();
        admission.checked_source_set(&sources, &mut budget())?;
        let mut duplicate = sources.clone();
        duplicate.push(sources[0].clone());
        assert!(matches!(
            admission.checked_source_set(&duplicate, &mut budget()),
            Err(SourceSetError::Duplicate)
        ));
        let conflicting = SourceSnapshot::new(
            SourceId("id-0".into()),
            1,
            "memory:other".into(),
            b"content-0".to_vec(),
            &mut budget(),
        )?;
        assert!(matches!(
            admission.checked_source_set(&[conflicting], &mut budget()),
            Err(SourceSetError::Source(_))
        ));
        let mut warm = budget();
        admission.checked_source_set(&sources, &mut warm)?;
        assert_eq!(warm.usage().allocation_units, 0);
        let mut calibration = SourceAdmission::default();
        calibration.checked_source_set(&sources, &mut budget())?;
        let mut replacement = budget();
        calibration.checked_source_set(&sources[..2], &mut replacement)?;
        for limit in [
            Limits {
                work: 0,
                ..budget().limits()
            },
            Limits {
                allocation_units: 0,
                ..budget().limits()
            },
            Limits {
                work: replacement.usage().work - 1,
                ..budget().limits()
            },
            Limits {
                allocation_units: replacement.usage().allocation_units - 1,
                ..budget().limits()
            },
        ] {
            let mut short = Budget::new(limit);
            assert!(
                admission
                    .checked_source_set(&sources[..2], &mut short)
                    .is_err()
            );
            assert!(
                admission
                    .checked_source_set(&sources[..2], &mut short)
                    .is_err()
            );
            let mut retained = budget();
            admission.checked_source_set(&sources, &mut retained)?;
            assert_eq!(retained.usage(), warm.usage());
        }
        Ok(())
    }
}
