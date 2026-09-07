use super::*;
use crate::binding::{BindingResolutionBatch, CanonicalBindingTarget, ResolutionTransition};
use crate::profile::ProviderRequirement;
use crate::recovery::ForeignStep;
use nepl3_core::{facts::OccurrenceId, source::Digest, syntax::NodeRef, value::OperationRef};

/// Checks retained structure and explicit grants, not the authenticity of the
/// historical host or the target's membership in a missing enclosing request.
pub(super) fn validate<E>(
    data: &Data<'_>,
    registry: &SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<(), PortableError<E>> {
    if data.history.is_empty() {
        return Ok(());
    }
    let facts = data.facts.ok_or(PortableError::Shape)?;
    let checked = facts
        .validate(registry, b, admission)
        .map_err(crate::facts::FactsError::from)?;
    checked
        .validate_resolutions(
            data.history
                .iter()
                .flat_map(|batch| batch.updates.iter())
                .flat_map(|u| [(u.occurrence, &u.before), (u.occurrence, &u.after)]),
            b,
            admission,
        )
        .map_err(crate::facts::FactsError::from)?;
    for (index, batch) in data.history.iter().enumerate() {
        // Authority.validate checks declared grants and ordered ranges, without
        // claiming that today's occupied IDs were free at historical issuance.
        batch
            .authority
            .validate(&checked, b, admission)
            .map_err(crate::facts::FactsError::from)?;
        b.charge(
            Resource::Work,
            (batch.provider.provider.len()
                + batch.provider.operation.name.len()
                + batch.provider.operation.schema.package.len()) as u64
                + 42,
        )?;
        if batch.provider.provider.is_empty() {
            return Err(PortableError::Shape);
        }
        let descriptor = registry
            .descriptor(&batch.provider.operation.schema)
            .ok_or(PortableError::Shape)?;
        b.charge(
            Resource::Work,
            (descriptor.operations.len() as u64)
                .saturating_mul(batch.provider.operation.name.len() as u64 + 1),
        )?;
        let signature = descriptor
            .operations
            .iter()
            .find(|v| v.name == batch.provider.operation.name)
            .ok_or(PortableError::Shape)?;
        if !crate::facts::signature(&signature.input, &signature.output, signature.pure) {
            return Err(PortableError::Shape);
        }
        for (ui, update) in batch.updates.iter().enumerate() {
            b.charge(
                Resource::Work,
                (batch.authority.resolution_updates.len() + facts.occurrences.len() + ui + 1)
                    as u64,
            )?;
            if !batch
                .authority
                .resolution_updates
                .contains(&update.occurrence)
                || batch.updates[..ui]
                    .iter()
                    .any(|v| v.occurrence == update.occurrence)
            {
                return Err(PortableError::Shape);
            }
            let occurrence = facts
                .occurrences
                .iter()
                .find(|v| v.id == update.occurrence)
                .ok_or(PortableError::Shape)?;
            let mut previous = None;
            for prior in &data.history[..index] {
                for v in &prior.updates {
                    b.charge(Resource::Work, 1)?;
                    if v.occurrence == update.occurrence {
                        previous = Some(&v.after);
                    }
                }
            }
            if let Some(previous) = previous
                && !resolution_equal(previous, &update.before, b)?
            {
                return Err(PortableError::Shape);
            }
            let mut later = false;
            for next in &data.history[index + 1..] {
                for v in &next.updates {
                    b.charge(Resource::Work, 1)?;
                    if v.occurrence == update.occurrence {
                        later = true;
                    }
                }
            }
            if !later && !resolution_equal(&update.after, &occurrence.resolution, b)? {
                return Err(PortableError::Shape);
            }
        }
    }
    Ok(())
}
fn resolution_equal<E>(
    a: &nepl3_core::facts::ReferenceResolution,
    c: &nepl3_core::facts::ReferenceResolution,
    b: &mut Budget,
) -> Result<bool, PortableError<E>> {
    use nepl3_core::{facts::ReferenceResolution as R, value::TypedValue};
    b.charge(Resource::Work, 1)?;
    Ok(match (a, c) {
        (R::Resolved(a), R::Resolved(c)) => a == c,
        (R::Unresolved(a), R::Unresolved(c)) => {
            b.charge(Resource::Work, (a.len() + c.len()) as u64)?;
            a == c
        }
        (R::Ambiguous(a), R::Ambiguous(c)) => {
            b.charge(Resource::Work, (a.len() + c.len()) as u64)?;
            a == c
        }
        (R::Deferred(a), R::Deferred(c)) => {
            if a.len() != c.len() {
                return Ok(false);
            }
            for (a, c) in a.iter().zip(c) {
                let (sa, na, va, fa) = match a {
                    TypedValue::Record(v) => (&v.schema, &v.kind, None, &v.fields),
                    TypedValue::Variant(v) => {
                        (&v.schema, &v.type_name, Some(&v.variant), &v.fields)
                    }
                };
                let (sc, nc, vc, fc) = match c {
                    TypedValue::Record(v) => (&v.schema, &v.kind, None, &v.fields),
                    TypedValue::Variant(v) => {
                        (&v.schema, &v.type_name, Some(&v.variant), &v.fields)
                    }
                };
                b.charge(
                    Resource::Work,
                    (sa.package.len()
                        + sc.package.len()
                        + na.len()
                        + nc.len()
                        + va.map_or(0, |v| v.len())
                        + vc.map_or(0, |v| v.len())) as u64
                        + 42,
                )?;
                if sa != sc || na != nc || va != vc || fa.len() != fc.len() {
                    return Ok(false);
                }
                for (a, c) in fa.iter().zip(fc) {
                    if !super::super::facts::compare::equal(a, c, b)? {
                        return Ok(false);
                    }
                }
            }
            true
        }
        _ => false,
    })
}

impl Value for ProviderRequirement {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        record(
            s.engine,
            "ProviderRequirement",
            [
                self.provider.value(s, c, b)?,
                self.revision.value(s, c, b)?,
                self.implementation_digest.value(s, c, b)?,
                self.operation.value(s, c, b)?,
            ],
            b,
        )
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let f = fields(v, s.engine, "ProviderRequirement", 4)?;
        Ok(Self {
            provider: alloc::string::String::read(&f[0], s, c, b)?,
            revision: u64::read(&f[1], s, c, b)?,
            implementation_digest: Digest::read(&f[2], s, c, b)?,
            operation: OperationRef::read(&f[3], s, c, b)?,
        })
    }
}
impl Value for CanonicalBindingTarget {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        record(
            s.engine,
            "CanonicalBindingTarget",
            [self.path.value(s, c, b)?, self.node.value(s, c, b)?],
            b,
        )
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let f = fields(v, s.engine, "CanonicalBindingTarget", 2)?;
        Ok(Self {
            path: Vec::<ForeignStep>::read(&f[0], s, c, b)?,
            node: NodeRef::read(&f[1], s, c, b)?,
        })
    }
}
impl Value for ResolutionTransition {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        record(
            s.engine,
            "ResolutionTransition",
            [
                self.occurrence.value(s, c, b)?,
                c.encode_reference_resolution(&self.before, b)
                    .map_err(boundary)?,
                c.encode_reference_resolution(&self.after, b)
                    .map_err(boundary)?,
            ],
            b,
        )
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let f = fields(v, s.engine, "ResolutionTransition", 3)?;
        Ok(Self {
            occurrence: OccurrenceId::read(&f[0], s, c, b)?,
            before: c.decode_reference_resolution(&f[1], b).map_err(boundary)?,
            after: c.decode_reference_resolution(&f[2], b).map_err(boundary)?,
        })
    }
}
impl Value for BindingResolutionBatch {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        record(
            s.engine,
            "BindingResolutionBatch",
            [
                self.provider.value(s, c, b)?,
                c.encode_fact_authority(&self.authority, b)
                    .map_err(boundary)?,
                self.target.value(s, c, b)?,
                self.updates.value(s, c, b)?,
            ],
            b,
        )
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let f = fields(v, s.engine, "BindingResolutionBatch", 4)?;
        Ok(Self {
            provider: ProviderRequirement::read(&f[0], s, c, b)?,
            authority: c.decode_fact_authority(&f[1], b).map_err(boundary)?,
            target: CanonicalBindingTarget::read(&f[2], s, c, b)?,
            updates: Vec::<ResolutionTransition>::read(&f[3], s, c, b)?,
        })
    }
}
