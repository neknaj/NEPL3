//! Borrowed identity equality, independent of registry resolution or authority.
use super::{OperationRef, SchemaRef};
use crate::{
    budget::{Budget, Resource, StopReason},
    lexical,
};
impl SchemaRef {
    /// Compare the complete identity without allocation or registry lookup.
    /// Fixed-size revision/digest rejection precedes variable UTF-8 names.
    pub fn equal_with_budget(&self, other: &Self, budget: &mut Budget) -> Result<bool, StopReason> {
        budget.charge(Resource::Work, 1)?;
        if self.revision != other.revision {
            return Ok(false);
        }
        budget.charge(Resource::Work, 32)?;
        if self.digest != other.digest {
            return Ok(false);
        }
        lexical::equal_with_budget(&self.package, &other.package, budget)
    }
}
impl OperationRef {
    /// Compare schema identity and operation spelling. This grants no execution
    /// authority and does not establish that the operation is registered.
    pub fn equal_with_budget(&self, other: &Self, budget: &mut Budget) -> Result<bool, StopReason> {
        if !self.schema.equal_with_budget(&other.schema, budget)? {
            return Ok(false);
        }
        lexical::equal_with_budget(&self.name, &other.name, budget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{budget::Limits, source::Digest};
    fn budget(work: u64) -> Budget {
        Budget::new(Limits {
            work,
            ..Default::default()
        })
    }
    fn operation() -> OperationRef {
        OperationRef {
            schema: SchemaRef {
                package: "界".into(),
                revision: 7,
                digest: Digest([9; 32]),
            },
            name: "run".into(),
        }
    }
    #[test]
    fn identity_comparisons_have_exact_limits_and_sticky_stops() -> Result<(), StopReason> {
        let a = operation();
        // Revision1, digest32, two string setups, and three Work per byte pair.
        let expected = 1 + 32 + 2 + 3 * (a.schema.package.len() + a.name.len()) as u64;
        let mut exact = budget(expected);
        assert!(a.equal_with_budget(&a.clone(), &mut exact)?);
        assert_eq!(exact.usage().work, expected);
        for work in [0, 1, 32, 33, expected - 1] {
            let mut short = budget(work);
            assert_eq!(
                a.equal_with_budget(&a.clone(), &mut short),
                Err(StopReason::WorkLimit)
            );
            let used = short.usage();
            assert_eq!(
                a.equal_with_budget(&operation(), &mut short),
                Err(StopReason::WorkLimit)
            );
            assert_eq!(short.usage(), used);
        }
        for reason in [
            StopReason::Cancelled,
            StopReason::SourceLimit,
            StopReason::WorkLimit,
            StopReason::DepthLimit,
            StopReason::NodeLimit,
            StopReason::AllocationLimit,
            StopReason::OutputLimit,
            StopReason::DiagnosticLimit,
            StopReason::EventLimit,
        ] {
            let mut b = budget(expected);
            b.stop(reason);
            let used = b.usage();
            assert_eq!(a.equal_with_budget(&operation(), &mut b), Err(reason));
            assert_eq!(b.usage(), used);
        }
        Ok(())
    }
    #[test]
    fn identity_rejects_each_component_without_unnecessary_suffix_scans() -> Result<(), StopReason>
    {
        let mut a = operation();
        a.schema.package = "界".repeat(4096);
        a.name = "a".repeat(4096);
        for component in 0..4 {
            let mut other = a.clone();
            match component {
                0 => other.schema.revision += 1,
                1 => other.schema.digest.0[31] ^= 1,
                2 => other.schema.package.push('x'),
                _ => other.name.push('x'),
            }
            let work = match component {
                0 => 1,
                1 => 33,
                2 => 34,
                _ => 35 + 3 * a.schema.package.len() as u64,
            };
            let mut b = budget(work);
            assert!(!a.equal_with_budget(&other, &mut b)?);
            assert_eq!(b.usage().work, work);
        }
        Ok(())
    }
}
