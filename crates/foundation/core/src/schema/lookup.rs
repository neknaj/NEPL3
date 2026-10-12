//! Per-validation borrowed name resolution, never a value-validation proof.
use super::*;

#[derive(Clone, Copy)]
struct Entry<'r, 't> {
    requested: &'t TypeRef,
    schema: &'r SchemaRef,
    definition: &'r NamedType,
}

/// The registry is fixed for the lifetime of this local cache. A pointer key
/// refers to an immutable expected descriptor, not an incoming value's name.
/// Only eight definitions are retained; eviction performs no allocation.
pub(super) struct Definitions<'r, 't> {
    registry: &'r SchemaRegistry,
    entries: [Option<Entry<'r, 't>>; 8],
    next: usize,
    len: usize,
}

impl<'r, 't> Definitions<'r, 't> {
    pub(super) fn new(registry: &'r SchemaRegistry) -> Self {
        Self {
            registry,
            entries: [None; 8],
            next: 0,
            len: 0,
        }
    }

    pub(super) fn resolve(
        &mut self,
        requested: &'t TypeRef,
        budget: &mut Budget,
    ) -> Result<(&'r SchemaRef, &'r NamedType), SchemaError> {
        for entry in self.entries[..self.len].iter().flatten() {
            budget.charge(Resource::Work, 1)?;
            if core::ptr::eq(entry.requested, requested) {
                return Ok((entry.schema, entry.definition));
            }
        }
        let (schema, descriptor) = self
            .registry
            .selected_descriptor_with_budget(&requested.package, requested.revision, budget)?
            .ok_or(SchemaError::UnknownSchema)?;
        let definition = SchemaRegistry::named_definition(descriptor, &requested.name, budget)?;
        budget.charge(Resource::Work, 1)?;
        self.entries[self.next] = Some(Entry {
            requested,
            schema,
            definition,
        });
        self.next = (self.next + 1) % self.entries.len();
        self.len = (self.len + 1).min(self.entries.len());
        Ok((schema, definition))
    }
}

/// Lexicographic UTF-8 byte order equals Rust str order. Admit each byte pair
/// before reading it; mismatching prefixes do not scan the untouched suffix.
pub(super) fn compare(
    left: &str,
    right: &str,
    budget: &mut Budget,
) -> Result<core::cmp::Ordering, StopReason> {
    budget.charge(Resource::Work, 1)?;
    for index in 0..left.len().min(right.len()) {
        budget.charge(Resource::Work, 3)?;
        let order = left.as_bytes()[index].cmp(&right.as_bytes()[index]);
        if order != core::cmp::Ordering::Equal {
            return Ok(order);
        }
    }
    Ok(left.len().cmp(&right.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::ToString, vec};
    fn budget(work: u64) -> Budget {
        Budget::new(crate::budget::Limits {
            work,
            allocation_units: 1_000_000,
            nodes: 1000,
            depth: 100,
            ..Default::default()
        })
    }
    #[test]
    fn comparison_admits_only_visited_pairs_with_exact_sticky_boundaries() -> Result<(), StopReason>
    {
        let huge = "界".repeat(1024);
        let tail = alloc::format!("{huge}x");
        let mismatch = alloc::format!("{huge}y");
        for (a, b, pairs) in [
            ("", "", 0),
            ("", huge.as_str(), 0),
            ("a", "ab", 1),
            ("x", huge.as_str(), 1),
            (huge.as_str(), huge.as_str(), huge.len()),
            (tail.as_str(), mismatch.as_str(), huge.len() + 1),
        ] {
            let work = 1 + 3 * pairs as u64;
            let mut exact = budget(work);
            assert_eq!(compare(a, b, &mut exact)?, a.cmp(b));
            assert_eq!(exact.usage().work, work);
            for limit in [0, work - 1] {
                let mut short = budget(limit);
                assert_eq!(compare(a, b, &mut short), Err(StopReason::WorkLimit));
                assert_eq!(
                    short.usage().work,
                    if limit == 0 {
                        0
                    } else {
                        1 + 3 * ((limit - 1) / 3)
                    }
                );
                let used = short.usage();
                assert_eq!(compare("", "", &mut short), Err(StopReason::WorkLimit));
                assert_eq!(short.usage(), used);
            }
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
            let mut b = budget(100);
            b.charge(Resource::Work, 7)?;
            b.stop(reason);
            let before = b.usage();
            assert_eq!(compare("a", "b", &mut b), Err(reason));
            assert_eq!(b.usage(), before);
        }
        Ok(())
    }
    #[test]
    fn resolution_cache_uses_exact_borrows_and_has_bounded_eviction() -> Result<(), SchemaError> {
        let descriptor = SchemaDescriptor {
            package: "p".into(),
            revision: 1,
            types: vec![NamedType {
                name: "T".into(),
                shape: TypeShape::Record { fields: vec![] },
                constraints: vec![],
            }],
            operations: vec![],
        };
        let mut r = SchemaRegistry::default();
        let schema = descriptor.reference(&mut budget(1_000_000))?;
        r.register(schema, descriptor, &mut budget(1_000_000))?;
        let requests: Vec<_> = (0..9)
            .map(|_| TypeRef {
                package: "p".to_string(),
                revision: 1,
                name: "T".to_string(),
            })
            .collect();
        let mut cache = Definitions::new(&r);
        for (i, request) in requests.iter().enumerate() {
            // i cached pointer comparisons (capped at8), one schema scan12,
            // one exact type comparison4, and one insertion1.
            let cost = i.min(8) as u64 + 17;
            let mut b = budget(cost);
            cache.resolve(request, &mut b)?;
            assert_eq!(b.usage().work, cost);
            assert_eq!(b.usage().allocation_units, 0);
        }
        assert_eq!(cache.len, 8);
        let mut warm = budget(1);
        cache.resolve(&requests[8], &mut warm)?;
        assert_eq!(warm.usage().work, 1);
        let mut short = budget(0);
        assert!(matches!(
            cache.resolve(&requests[8], &mut short),
            Err(SchemaError::Stopped(StopReason::WorkLimit))
        ));
        let mut evicted = budget(25);
        cache.resolve(&requests[0], &mut evicted)?;
        assert_eq!(evicted.usage().work, 25);
        // A fresh context is cold, even for the exact previously resolved ref.
        let mut fresh = Definitions::new(&r);
        let mut b = budget(17);
        fresh.resolve(&requests[0], &mut b)?;
        assert_eq!(b.usage().work, 17);
        Ok(())
    }
}
