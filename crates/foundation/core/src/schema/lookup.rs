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

pub(super) use crate::lexical::compare_with_budget as compare;

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
