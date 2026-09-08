//! Derived lookup only: portable form order and indices remain unchanged.
use super::Form;
use alloc::vec::Vec;
use core::cmp::Ordering;
use nepl3_core::budget::{Budget, Resource, StopReason};

pub(super) struct FormIndex(Vec<usize>);

fn compare(
    form: &Form,
    category: &str,
    spelling: &str,
    budget: &mut Budget,
) -> Result<Ordering, StopReason> {
    budget.charge(
        Resource::Work,
        category.len().min(form.category.len()) as u64 + 1,
    )?;
    let order = form.category.as_str().cmp(category);
    if order != Ordering::Equal {
        return Ok(order);
    }
    budget.charge(
        Resource::Work,
        spelling.len().min(form.spelling.len()) as u64 + 1,
    )?;
    Ok(form.spelling.as_str().cmp(spelling))
}

impl FormIndex {
    pub(super) fn new(forms: &[Form], budget: &mut Budget) -> Result<Self, StopReason> {
        let bytes = forms
            .len()
            .checked_mul(core::mem::size_of::<usize>())
            .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
        budget.charge(Resource::AllocationUnits, bytes as u64)?;
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(forms.len())
            .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
        for (index, form) in forms.iter().enumerate() {
            let mut low = 0;
            let mut high = indices.len();
            while low < high {
                let mid = low + (high - low) / 2;
                if compare(&forms[indices[mid]], &form.category, &form.spelling, budget)?
                    == Ordering::Less
                {
                    low = mid + 1;
                } else {
                    high = mid;
                }
            }
            budget.charge(Resource::Work, (indices.len() - low) as u64 + 1)?;
            indices.insert(low, index);
        }
        Ok(Self(indices))
    }

    pub(super) fn find(
        &self,
        forms: &[Form],
        category: &str,
        spelling: &str,
        budget: &mut Budget,
    ) -> Result<Option<usize>, StopReason> {
        budget.poll()?;
        let mut low = 0;
        let mut high = self.0.len();
        while low < high {
            let mid = low + (high - low) / 2;
            let index = self.0[mid];
            match compare(&forms[index], category, spelling, budget)? {
                Ordering::Less => low = mid + 1,
                Ordering::Greater => high = mid,
                Ordering::Equal => return Ok(Some(index)),
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::BindingId;
    use nepl3_core::{
        budget::Limits,
        source::Digest,
        value::{KindRef, SchemaRef},
    };

    fn forms() -> Vec<Form> {
        [("Z", "x"), ("Expr", "日本語"), ("Expr", "x"), ("Expr", "a")]
            .into_iter()
            .map(|(category, spelling)| Form {
                category: category.into(),
                spelling: spelling.into(),
                kind: KindRef {
                    schema: SchemaRef {
                        package: "test".into(),
                        revision: 1,
                        digest: Digest([0; 32]),
                    },
                    local_kind: 0,
                },
                fields: Vec::new(),
                binding: BindingId(0),
                styles: Vec::new(),
                selection_rules: Vec::new(),
            })
            .collect()
    }
    fn budget() -> Budget {
        Budget::new(Limits {
            work: 10_000,
            allocation_units: 10_000,
            ..Limits::default()
        })
    }

    #[test]
    fn exact_category_and_spelling_preserve_original_indices() -> Result<(), StopReason> {
        let forms = forms();
        let index = FormIndex::new(&forms, &mut budget())?;
        for (expected, form) in forms.iter().enumerate() {
            assert_eq!(
                index.find(&forms, &form.category, &form.spelling, &mut budget())?,
                Some(expected)
            );
        }
        for (category, spelling) in [
            ("", "x"),
            ("Expr", ""),
            ("Expr", "日本"),
            ("Expr", "xx"),
            ("Z", "a"),
        ] {
            assert_eq!(index.find(&forms, category, spelling, &mut budget())?, None);
        }
        Ok(())
    }

    #[test]
    fn index_build_and_lookup_keep_cumulative_stops() -> Result<(), StopReason> {
        let forms = forms();
        let mut complete = budget();
        let index = FormIndex::new(&forms, &mut complete)?;
        let measured = complete.usage();
        for cap in 0..measured.work {
            let mut limited = Budget::new(Limits {
                work: cap,
                ..budget().limits()
            });
            assert!(matches!(
                FormIndex::new(&forms, &mut limited),
                Err(StopReason::WorkLimit)
            ));
            assert_eq!(limited.poll(), Err(StopReason::WorkLimit));
        }
        let mut allocation = Budget::new(Limits {
            allocation_units: measured.allocation_units - 1,
            ..budget().limits()
        });
        assert!(matches!(
            FormIndex::new(&forms, &mut allocation),
            Err(StopReason::AllocationLimit)
        ));
        assert_eq!(allocation.poll(), Err(StopReason::AllocationLimit));
        let mut lookup = Budget::new(Limits {
            work: 0,
            ..budget().limits()
        });
        assert_eq!(
            index.find(&forms, "Expr", "x", &mut lookup),
            Err(StopReason::WorkLimit)
        );
        assert_eq!(lookup.poll(), Err(StopReason::WorkLimit));
        let empty = FormIndex::new(&[], &mut budget())?;
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            FormIndex::new(&[], &mut cancelled),
            Err(StopReason::Cancelled)
        ));
        assert_eq!(
            empty.find(&[], "Expr", "x", &mut cancelled),
            Err(StopReason::Cancelled)
        );
        Ok(())
    }
}
