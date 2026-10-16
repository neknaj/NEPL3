//! Operation-local reuse of an identical canonical ForeignSyntax source prefix.
//! Only the checked native conversion caller can supply this proof; this is not
//! a generic NDF memoization API and does not replace validation or source data.
use super::{Sink, emit};
use crate::{WireError, source::fields};
use nepl3_core::{
    budget::{Budget, Resource},
    source::Digest,
    syntax::ForeignSyntax,
    value::{NdfValue, SchemaRef},
};
use sha2::{Digest as _, Sha256};

struct Saved<'a> {
    syntax: &'a ForeignSyntax,
    hash: Sha256,
    depth: u64,
}
pub(crate) struct SyntaxPrefixCache<'a> {
    schema: &'a SchemaRef,
    domain: &'a [u8],
    saved: Option<Saved<'a>>,
}
fn copy_hash(hash: &Sha256, b: &mut Budget) -> Result<Sha256, WireError> {
    let size = core::mem::size_of::<Sha256>() as u64;
    b.charge(Resource::Work, size)?;
    b.charge(Resource::AllocationUnits, size)?;
    Ok(hash.clone())
}
fn same_prefix(a: &ForeignSyntax, z: &ForeignSyntax, b: &mut Budget) -> Result<bool, WireError> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::Work,
        (a.schema.package.len() as u64)
            .saturating_add(z.schema.package.len() as u64)
            .saturating_add(a.category.len() as u64)
            .saturating_add(z.category.len() as u64)
            .saturating_add(66),
    )?;
    if a.schema != z.schema
        || a.category != z.category
        || a.bundle.sources.len() != z.bundle.sources.len()
    {
        return Ok(false);
    }
    for (a, z) in a.bundle.sources.iter().zip(&z.bundle.sources) {
        if !a.eq_with_budget(z, b)? {
            return Ok(false);
        }
    }
    Ok(true)
}
struct PrefixHash<'a> {
    hash: Sha256,
    source: &'a NdfValue,
    warm: Option<&'a Saved<'a>>,
    capturing: bool,
    max_depth: u64,
    checkpoint: Option<Sha256>,
}
impl Sink for PrefixHash<'_> {
    fn shortcut(&mut self, item: &NdfValue, depth: u64, b: &mut Budget) -> Result<bool, WireError> {
        b.charge(Resource::Work, 1)?;
        if self.capturing {
            self.max_depth = self.max_depth.max(depth);
        }
        if core::ptr::eq(item, self.source) {
            if let Some(saved) = self.warm {
                b.observe_depth(saved.depth)?;
                self.hash = copy_hash(&saved.hash, b)?;
                return Ok(true);
            }
            self.max_depth = depth;
            self.capturing = true;
        }
        Ok(false)
    }
    fn enter(&mut self, item: &NdfValue, _: &mut Budget) -> Result<bool, WireError> {
        Ok(self.warm.is_none() && core::ptr::eq(item, self.source))
    }
    fn leave(&mut self, b: &mut Budget) -> Result<(), WireError> {
        self.checkpoint = Some(copy_hash(&self.hash, b)?);
        self.capturing = false;
        Ok(())
    }
    fn write(&mut self, bytes: &[u8], b: &mut Budget) -> Result<(), WireError> {
        Sink::write(&mut self.hash, bytes, b)
    }
}
impl<'a> SyntaxPrefixCache<'a> {
    pub(crate) fn new(schema: &'a SchemaRef, domain: &'a [u8]) -> Self {
        Self {
            schema,
            domain,
            saved: None,
        }
    }
    pub(crate) fn digest(
        &mut self,
        syntax: &'a ForeignSyntax,
        encoded: &NdfValue,
        b: &mut Budget,
    ) -> Result<Digest, WireError> {
        b.poll()?;
        let foreign = fields(encoded, self.schema, "ForeignSyntax", 5)?;
        let bundle = fields(&foreign[3], self.schema, "SyntaxBundle", 7)?;
        let hit = match &self.saved {
            Some(saved) => same_prefix(saved.syntax, syntax, b)?,
            None => false,
        };
        b.charge(Resource::OutputBytes, 32)?;
        b.charge(Resource::Work, self.domain.len() as u64)?;
        let mut out = PrefixHash {
            hash: Sha256::new(),
            source: &bundle[0],
            warm: if hit { self.saved.as_ref() } else { None },
            capturing: false,
            max_depth: 0,
            checkpoint: None,
        };
        out.hash.update(self.domain);
        emit(encoded, &mut out, b)?;
        let checkpoint = out.checkpoint.take();
        let depth = out.max_depth;
        let result = Digest(out.hash.finalize().into());
        if let Some(hash) = checkpoint {
            b.charge(Resource::Work, 1)?;
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Saved<'_>>() as u64,
            )?;
            self.saved = Some(Saved {
                syntax,
                hash,
                depth,
            });
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        source::{record, sources_value},
        view::schema_value,
    };
    use alloc::vec;
    use nepl3_core::{
        budget::Limits,
        source::{SourceAdmission, SourceId, SourceSnapshot},
        syntax::{EnvironmentRef, NodeRef, SyntaxBundle},
    };
    fn budget() -> Budget {
        Budget::new(Limits {
            source_bytes: 1_000_000,
            work: 10_000_000,
            depth: 100,
            nodes: 1_000_000,
            allocation_units: 10_000_000,
            output_bytes: 1_000_000,
            diagnostics: 100,
            events: 100,
        })
    }
    fn native(schema: &SchemaRef, uri: &str) -> Result<ForeignSyntax, WireError> {
        Ok(ForeignSyntax {
            schema: schema.clone(),
            category: "Node".into(),
            root: NodeRef(0),
            environment: EnvironmentRef {
                id: 0,
                digest: Digest([0; 32]),
            },
            bundle: SyntaxBundle {
                sources: vec![SourceSnapshot::new(
                    SourceId("test".into()),
                    1,
                    uri.into(),
                    vec![b'x'; 4096],
                    &mut budget(),
                )?],
                nodes: vec![],
                origins: vec![],
                root: NodeRef(0),
                environments: vec![],
                tokens: vec![],
                source_maps: vec![],
            },
        })
    }
    // Prefix fixtures intentionally contain no syntax nodes. Public integration
    // tests separately exercise real checked conversion and malformed inputs.
    fn value(n: &ForeignSyntax, s: &SchemaRef) -> Result<NdfValue, WireError> {
        let b = &mut budget();
        let bundle = record(
            s,
            "SyntaxBundle",
            [
                sources_value(&n.bundle.sources, s, &mut SourceAdmission::default(), b)?,
                NdfValue::List(vec![]),
                NdfValue::List(vec![]),
                record(s, "NodeRef", [NdfValue::U64(0)], b)?,
                NdfValue::List(vec![]),
                NdfValue::List(vec![]),
                NdfValue::List(vec![]),
            ],
            b,
        )?;
        record(
            s,
            "ForeignSyntax",
            [
                schema_value(&n.schema, s, b)?,
                NdfValue::Text(n.category.clone()),
                record(s, "NodeRef", [NdfValue::U64(0)], b)?,
                bundle,
                record(
                    s,
                    "EnvironmentRef",
                    [
                        NdfValue::U64(n.environment.id),
                        NdfValue::Bytes(n.environment.digest.0.to_vec()),
                    ],
                    b,
                )?,
            ],
            b,
        )
    }
    #[test]
    fn keys_suffix_replacement_and_independent_storage_preserve_bytes() -> Result<(), WireError> {
        let s = SchemaRef {
            package: "foundation-fixture".into(),
            revision: 1,
            digest: Digest([1; 32]),
        };
        let a = native(&s, "memory:a")?;
        let mut suffix = a.clone();
        suffix.environment.id = 7;
        let mut category = a.clone();
        category.category = "Other".into();
        let mut schema = a.clone();
        schema.schema.digest.0[0] ^= 1;
        let uri = native(&s, "memory:b")?;
        let independent = native(&s, "memory:a")?;
        let mut empty = a.clone();
        empty.bundle.sources.clear();
        let mut two = a.clone();
        two.bundle.sources.push(SourceSnapshot::new(
            SourceId("other".into()),
            1,
            "memory:other".into(),
            vec![b'y'; 17],
            &mut budget(),
        )?);
        let mut reordered = two.clone();
        reordered.bundle.sources.reverse();
        let mut changed = a.clone();
        changed.bundle.sources[0] = SourceSnapshot::new(
            SourceId("test".into()),
            1,
            "memory:a".into(),
            vec![b'z'; 4096],
            &mut budget(),
        )?;
        let mut cache = SyntaxPrefixCache::new(&s, b"test-domain");
        for n in [
            &a,
            &suffix,
            &category,
            &schema,
            &uri,
            &a,
            &independent,
            &empty,
            &empty,
            &two,
            &reordered,
            &changed,
            &a,
        ] {
            let v = value(n, &s)?;
            assert_eq!(
                cache.digest(n, &v, &mut budget())?,
                super::super::digest(b"test-domain", &v, &mut budget())?
            );
        }
        Ok(())
    }
    #[test]
    fn warm_limits_reobserve_depth_and_exact_resource_boundaries() -> Result<(), WireError> {
        let s = SchemaRef {
            package: "foundation-fixture".into(),
            revision: 1,
            digest: Digest([1; 32]),
        };
        let a = native(&s, "memory:a")?;
        let v = value(&a, &s)?;
        let mut cache = SyntaxPrefixCache::new(&s, b"test-domain");
        cache.digest(&a, &v, &mut budget())?;
        let mut measured = budget();
        cache.digest(&a, &v, &mut measured)?;
        let usage = measured.usage();
        let exact = Limits {
            work: usage.work,
            nodes: usage.nodes,
            allocation_units: usage.allocation_units,
            output_bytes: usage.output_bytes,
            depth: usage.depth,
            ..budget().limits()
        };
        cache.digest(&a, &v, &mut Budget::new(exact))?;
        for short in [
            Limits {
                work: exact.work - 1,
                ..exact
            },
            Limits {
                allocation_units: exact.allocation_units - 1,
                ..exact
            },
            Limits {
                depth: exact.depth - 1,
                ..exact
            },
            Limits {
                nodes: exact.nodes - 1,
                ..exact
            },
            Limits {
                output_bytes: 31,
                ..exact
            },
        ] {
            let mut b = Budget::new(short);
            assert!(cache.digest(&a, &v, &mut b).is_err());
            assert!(cache.digest(&a, &v, &mut b).is_err());
        }
        // A failed replacement must leave the last successful A checkpoint
        // intact, even when B's complete hash was already computed.
        let mut other = a.clone();
        other.category = "Other".into();
        let other_value = value(&other, &s)?;
        let mut calibration = SyntaxPrefixCache::new(&s, b"test-domain");
        calibration.digest(&a, &v, &mut budget())?;
        let mut replacement = budget();
        calibration.digest(&other, &other_value, &mut replacement)?;
        for (limit, reason) in [
            (
                Limits {
                    work: replacement.usage().work - 1,
                    ..budget().limits()
                },
                nepl3_core::budget::StopReason::WorkLimit,
            ),
            (
                Limits {
                    allocation_units: replacement.usage().allocation_units - 1,
                    ..budget().limits()
                },
                nepl3_core::budget::StopReason::AllocationLimit,
            ),
        ] {
            let mut cache = SyntaxPrefixCache::new(&s, b"test-domain");
            let expected = cache.digest(&a, &v, &mut budget())?;
            assert_eq!(
                cache.digest(&other, &other_value, &mut Budget::new(limit)),
                Err(WireError::Stopped(reason))
            );
            let mut retained = budget();
            assert_eq!(cache.digest(&a, &v, &mut retained)?, expected);
            assert_eq!(retained.usage(), usage);
        }
        let mut raw = budget();
        super::super::digest(b"test-domain", &v, &mut raw)?;
        assert!(usage.work < raw.usage().work);
        assert!(usage.allocation_units < raw.usage().allocation_units);
        Ok(())
    }
}
