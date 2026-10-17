//! Opt-in v2 component identities paired with unchanged GroupArchive transport.
//! Identity preimages are private envelopes, not stand-alone native syntax.
use super::*;
use crate::source::archive::tables::Capture;
use core::cmp::Ordering;
use nepl3_core::source::{Digest, SourceSnapshot};

const SOURCE: &[u8] = b"NEPL3.Foreign.SourceContent.v2\0";
const PROVENANCE: &[u8] = b"NEPL3.Foreign.Provenance.v2\0";
const ENVIRONMENT: &[u8] = b"NEPL3.Foreign.Environment.v2\0";
const SYNTAX: &[u8] = b"NEPL3.Foreign.Syntax.v2\0";
const ENTRY: &[u8] = b"NEPL3.Foreign.Entry.v2\0";

struct CompactCapture<'a> {
    table: Option<Capture>,
    registry: &'a SchemaRegistry,
    cache: Vec<(SourceSnapshot, Digest)>,
    current: Vec<NdfValue>,
    provenances: Vec<NdfValue>,
    entries: Vec<NdfValue>,
}
impl<'a> CompactCapture<'a> {
    fn new(registry: &'a SchemaRegistry) -> Self {
        Self {
            table: Some(Capture::default()),
            registry,
            cache: Vec::new(),
            current: Vec::new(),
            provenances: Vec::new(),
            entries: Vec::new(),
        }
    }
    fn identity_only(registry: &'a SchemaRegistry) -> Self {
        Self {
            table: None,
            registry,
            cache: Vec::new(),
            current: Vec::new(),
            provenances: Vec::new(),
            entries: Vec::new(),
        }
    }
    fn source_digest(
        &mut self,
        source: &SourceSnapshot,
        schema: &SchemaRef,
        admission: &mut SourceAdmission,
        b: &mut Budget,
        mut next: Option<&mut usize>,
    ) -> Result<Digest, WireError> {
        // A hint belongs to one ordered local set only. It never substitutes for
        // identity comparison, exact snapshot equality, or source admission.
        if let Some(index) = next.as_deref_mut()
            && let Some((cached, digest)) = self.cache.get(*index)
            && cached
                .identity()
                .compare_with_budget(source.identity(), b)?
                == Ordering::Equal
        {
            if !cached.eq_with_budget(source, b)? {
                return Err(nepl3_core::source::SourceError::IdentityConflict.into());
            }
            *index += 1;
            b.charge(Resource::Work, 32)?;
            return Ok(*digest);
        }
        // Gaps use binary search, so sparse subsets do not scan the entire cache.
        let mut lo = 0;
        let mut hi = self.cache.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match self.cache[mid]
                .0
                .identity()
                .compare_with_budget(source.identity(), b)?
            {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => {
                    if !self.cache[mid].0.eq_with_budget(source, b)? {
                        return Err(nepl3_core::source::SourceError::IdentityConflict.into());
                    }
                    if let Some(index) = next {
                        *index = mid + 1;
                    }
                    b.charge(Resource::Work, 32)?;
                    return Ok(self.cache[mid].1);
                }
            }
        }
        // Admission has already occurred through the selected capture path. Still use the
        // normal typed adapter and registry check; a stored digest alone is no proof.
        let NdfValue::List(ref mut values) =
            crate::source::sources_value(core::slice::from_ref(source), schema, admission, b)?
        else {
            return Err(WireError::InvalidType);
        };
        let value = values.pop().ok_or(WireError::InvalidLength)?;
        b.charge(
            Resource::AllocationUnits,
            ("nepl3.foundation".len() + "SourceContent".len()) as u64,
        )?;
        self.registry
            .validate(&expected("SourceContent"), &value, b)?;
        let digest = crate::encode::digest(SOURCE, &value, b)?;
        b.charge(Resource::Work, (self.cache.len() - lo) as u64)?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<(SourceSnapshot, Digest)>() as u64,
        )?;
        let owned = source.clone_with_budget(b)?;
        self.cache.insert(lo, (owned, digest));
        if let Some(index) = next {
            *index = lo + 1;
        }
        Ok(digest)
    }
}
impl GroupSources for CompactCapture<'_> {
    fn take(
        &mut self,
        sources: &[SourceSnapshot],
        schema: &SchemaRef,
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        let archive = self.table.is_some();
        let empty = if let Some(table) = self.table.as_mut() {
            table.take(sources, admission, b)?
        } else {
            NdfValue::List(Vec::new())
        };
        let mut members = Vec::new();
        let mut next = 0;
        for (_, source) in crate::source::ordered_sources(sources, b)? {
            if !archive {
                admission.admit_existing(source, b)?;
            }
            let digest = self.source_digest(
                source,
                schema,
                admission,
                b,
                if archive { None } else { Some(&mut next) },
            )?;
            push(&mut members, digest_value(digest, b)?, b)?;
        }
        push(&mut self.current, NdfValue::List(members), b)?;
        Ok(empty)
    }
    fn finish_component(
        &mut self,
        component: SourceComponent,
        b: &mut Budget,
    ) -> Result<(), WireError> {
        b.charge(Resource::Work, 1)?;
        let members = NdfValue::List(core::mem::take(&mut self.current));
        let target = match component {
            SourceComponent::Provenance => &mut self.provenances,
            SourceComponent::Entry => &mut self.entries,
        };
        push(target, members, b)
    }
}
fn digest_value(digest: Digest, b: &mut Budget) -> Result<NdfValue, WireError> {
    b.charge(Resource::Work, 32)?;
    b.charge(Resource::AllocationUnits, 32)?;
    Ok(NdfValue::Bytes(digest.0.to_vec()))
}
fn component(
    domain: &[u8],
    skeleton: &NdfValue,
    members: NdfValue,
    b: &mut Budget,
) -> Result<Digest, WireError> {
    let mut fields = Vec::new();
    push(&mut fields, skeleton.clone_with_budget(b)?, b)?;
    push(&mut fields, members, b)?;
    crate::encode::digest(domain, &NdfValue::List(fields), b)
}

/// Encode unchanged GroupArchive bytes and explicit v2 entry identities together.
/// No v1 codec or identity API selects this operation implicitly. All validation,
/// transport generation, and hashes share the supplied admission and Budget.
/// A stop or malformed component returns no successful bytes/digest pair.
pub fn encode(
    group: &ForeignClosureGroup,
    group_archive: &SchemaRef,
    source_archive: &SchemaRef,
    foundation: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<(Vec<u8>, Vec<Digest>), WireError> {
    encode_inner(
        group,
        group_archive,
        source_archive,
        foundation,
        registry,
        admission,
        b,
    )
    .map_err(|error| {
        match nepl3_core::value_codec::FoundationCodecError::stop_reason(&error) {
            Some(reason) => WireError::Stopped(reason),
            None => error,
        }
    })
}
fn encode_inner(
    group: &ForeignClosureGroup,
    group_archive: &SchemaRef,
    source_archive: &SchemaRef,
    foundation: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<(Vec<u8>, Vec<Digest>), WireError> {
    crate::source::archive::fields::check(foundation, registry, true, b)?;
    let mut capture = CompactCapture::new(registry);
    let skeleton = to_value_with_sources(group, foundation, registry, admission, b, &mut capture)?;
    finish_encoded(
        group,
        skeleton,
        capture,
        Boundary {
            group_archive,
            source_archive,
            foundation,
            registry,
        },
        admission,
        b,
    )
}

struct Boundary<'a> {
    group_archive: &'a SchemaRef,
    source_archive: &'a SchemaRef,
    foundation: &'a SchemaRef,
    registry: &'a SchemaRegistry,
}
fn finish_encoded(
    group: &ForeignClosureGroup,
    skeleton: NdfValue,
    capture: CompactCapture<'_>,
    boundary: Boundary<'_>,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<(Vec<u8>, Vec<Digest>), WireError> {
    let Boundary {
        group_archive,
        source_archive,
        foundation,
        registry,
    } = boundary;
    // Root and unchanged payload validation also run for an empty group.
    let bytes = crate::encode_checked(&skeleton, &expected("ForeignClosureGroup"), registry, b)?;
    if !capture.current.is_empty() {
        return Err(WireError::InvalidLength);
    }
    let sources = capture.table.ok_or(WireError::InvalidLength)?.finish(
        source_archive,
        foundation,
        admission,
        b,
    )?;
    let value = record(
        group_archive,
        nepl3_core::schema::archive::group::ROOT,
        [sources, NdfValue::Bytes(bytes)],
        b,
    )?;
    let transport = crate::encode_checked(&value, &super::archive::archive_type(), registry, b)?;
    let result = entry_commitments(
        group,
        &skeleton,
        capture.provenances,
        capture.entries,
        foundation,
        b,
    )?;
    Ok((transport, result))
}
fn entry_commitments(
    group: &ForeignClosureGroup,
    skeleton: &NdfValue,
    provenances: Vec<NdfValue>,
    entries: Vec<NdfValue>,
    foundation: &SchemaRef,
    b: &mut Budget,
) -> Result<Vec<Digest>, WireError> {
    let f = fields(skeleton, foundation, "ForeignClosureGroup", 3)?;
    let ps = list(&f[0])?;
    let es = list(&f[1])?;
    let xs = list(&f[2])?;
    if ps.len() != provenances.len() || xs.len() != entries.len() {
        return Err(WireError::InvalidLength);
    }
    let mut pd = Vec::new();
    for (p, members) in ps.iter().zip(provenances) {
        b.charge(Resource::Work, 1)?;
        push(&mut pd, component(PROVENANCE, p, members, b)?, b)?;
    }
    let mut ed = Vec::new();
    for e in es {
        b.charge(Resource::Work, 1)?;
        let f = fields(e, foundation, "ForeignOwnerEnvironment", 2)?;
        push(&mut ed, crate::encode::digest(ENVIRONMENT, &f[1], b)?, b)?;
    }
    let mut result = Vec::new();
    for ((entry, x), members) in group.entries.iter().zip(xs).zip(entries) {
        b.charge(Resource::Work, 1)?;
        let owner_index = usize::try_from(entry.owner.0).map_err(|_| WireError::InvalidLength)?;
        let owner = group
            .environments
            .get(owner_index)
            .ok_or(WireError::InvalidLength)?;
        let provenance_index =
            usize::try_from(owner.provenance.0).map_err(|_| WireError::InvalidLength)?;
        let provenance = *pd.get(provenance_index).ok_or(WireError::InvalidLength)?;
        let environment = *ed.get(owner_index).ok_or(WireError::InvalidLength)?;
        let f = fields(x, foundation, "ForeignGroupEntry", 2)?;
        let syntax = component(SYNTAX, &f[1], members, b)?;
        let mut values = Vec::new();
        for digest in [provenance, environment, syntax] {
            push(&mut values, digest_value(digest, b)?, b)?;
        }
        push(
            &mut result,
            crate::encode::digest(ENTRY, &NdfValue::List(values), b)?,
            b,
        )?;
    }
    b.poll()?;
    Ok(result)
}

#[cfg(test)]
mod tests;

/// Native consuming-language occurrence depths and their registry authority.
/// This is an input, not a validation receipt. Each call validates it afresh.
pub struct GroupAtDepths<'a> {
    pub group: &'a ForeignClosureGroup,
    pub depths: &'a [u64],
    pub native_registry: &'a SchemaRegistry,
}

/// Preserve per-entry semantic depths, including an already-active outer depth.
/// Caller structure failures remain separate from codec-registry/encoding errors.
pub fn encode_at_depths(
    input: GroupAtDepths<'_>,
    group_archive: &SchemaRef,
    source_archive: &SchemaRef,
    foundation: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<(Vec<u8>, Vec<Digest>), nepl3_core::value_codec::GroupEncodingError<WireError>> {
    use nepl3_core::value_codec::GroupEncodingError;
    validate_for_encoding_at_depths(
        input.group,
        input.depths,
        input.native_registry,
        registry,
        admission,
        b,
    )
    .map_err(|error| match error {
        GroupEncodingError::Structure(error) => GroupEncodingError::Structure(error),
        GroupEncodingError::Codec(error) => GroupEncodingError::Codec(
            match nepl3_core::value_codec::FoundationCodecError::stop_reason(&error) {
                Some(reason) => WireError::Stopped(reason),
                None => error,
            },
        ),
    })?;
    let encoded = (|| {
        crate::source::archive::fields::check(foundation, registry, true, b)?;
        let mut capture = CompactCapture::new(registry);
        let skeleton = encode_validated(
            input.group,
            foundation,
            registry,
            admission,
            b,
            &mut capture,
        )?;
        finish_encoded(
            input.group,
            skeleton,
            capture,
            Boundary {
                group_archive,
                source_archive,
                foundation,
                registry,
            },
            admission,
            b,
        )
    })();
    encoded.map_err(|error| {
        GroupEncodingError::Codec(
            match nepl3_core::value_codec::FoundationCodecError::stop_reason(&error) {
                Some(reason) => WireError::Stopped(reason),
                None => error,
            },
        )
    })
}

/// Identity-only operation. The complete skeleton and position-preserving local
/// memberships bind the ordered group without serializing either archive layer.
pub fn commit_at_depths(
    input: GroupAtDepths<'_>,
    foundation: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<
    nepl3_core::value_codec::commitment::GroupCommitments,
    nepl3_core::value_codec::GroupEncodingError<WireError>,
> {
    use nepl3_core::value_codec::{
        FoundationCodecError, GroupEncodingError, commitment::GroupCommitments,
    };
    let normalize = |error: WireError| match error.stop_reason() {
        Some(reason) => WireError::Stopped(reason),
        None => error,
    };
    validate_for_encoding_at_depths(
        input.group,
        input.depths,
        input.native_registry,
        registry,
        admission,
        b,
    )
    .map_err(|e| match e {
        GroupEncodingError::Structure(e) => GroupEncodingError::Structure(e),
        GroupEncodingError::Codec(e) => GroupEncodingError::Codec(normalize(e)),
    })?;
    let result = (|| {
        crate::source::archive::fields::check(foundation, registry, true, b)?;
        let mut capture = CompactCapture::identity_only(registry);
        let skeleton = encode_validated(
            input.group,
            foundation,
            registry,
            admission,
            b,
            &mut capture,
        )?;
        b.charge(
            Resource::AllocationUnits,
            ("nepl3.foundation".len() + "ForeignClosureGroup".len()) as u64,
        )?;
        registry.validate(&expected("ForeignClosureGroup"), &skeleton, b)?;
        if !capture.current.is_empty() {
            return Err(WireError::InvalidLength);
        }
        let mut provenance_members = Vec::new();
        for value in &capture.provenances {
            push(&mut provenance_members, value.clone_with_budget(b)?, b)?;
        }
        let mut entry_members = Vec::new();
        for value in &capture.entries {
            push(&mut entry_members, value.clone_with_budget(b)?, b)?;
        }
        let entry_digests = entry_commitments(
            input.group,
            &skeleton,
            capture.provenances,
            capture.entries,
            foundation,
            b,
        )?;
        let mut fields = Vec::new();
        push(&mut fields, skeleton, b)?;
        push(&mut fields, NdfValue::List(provenance_members), b)?;
        push(&mut fields, NdfValue::List(entry_members), b)?;
        let group_digest =
            crate::encode::digest(b"NEPL3.Foreign.Group.v2\0", &NdfValue::List(fields), b)?;
        b.poll()?;
        Ok(GroupCommitments {
            group_digest,
            entry_digests,
        })
    })();
    result.map_err(|e| GroupEncodingError::Codec(normalize(e)))
}
