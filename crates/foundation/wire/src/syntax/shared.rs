//! Explicit root-bundle source/map sharing. Nested foreign bundles retain their
//! own ordinary envelope. A pool is storage, never an ambient declaration scope.
use super::*;
use nepl3_core::source::Digest;
mod pool;

const SOURCE_DOMAIN: &[u8] = b"NEPL3.SyntaxBundleSet.Source.v1\0";
const MAP_DOMAIN: &[u8] = b"NEPL3.SyntaxBundleSet.Mapping.v1\0";

struct Entry {
    digest: Digest,
    value: NdfValue,
    used: bool,
}

fn locate(
    entries: &[Entry],
    digest: Digest,
    b: &mut Budget,
) -> Result<Result<usize, usize>, WireError> {
    let (mut low, mut high) = (0, entries.len());
    while low < high {
        b.charge(Resource::Work, 33)?;
        let mid = low + (high - low) / 2;
        match entries[mid].digest.cmp(&digest) {
            core::cmp::Ordering::Less => low = mid + 1,
            core::cmp::Ordering::Greater => high = mid,
            core::cmp::Ordering::Equal => return Ok(Ok(mid)),
        }
    }
    Ok(Err(low))
}

fn table(mut entries: Vec<Entry>, b: &mut Budget) -> Result<NdfValue, WireError> {
    fn sift(entries: &mut [Entry], mut root: usize, b: &mut Budget) -> Result<(), WireError> {
        while root < entries.len() / 2 {
            b.charge(Resource::Work, 65)?;
            let mut child = root * 2 + 1;
            if child + 1 < entries.len() && entries[child].digest < entries[child + 1].digest {
                child += 1;
            }
            if entries[root].digest >= entries[child].digest {
                break;
            }
            b.charge(Resource::Work, 1)?;
            entries.swap(root, child);
            root = child;
        }
        Ok(())
    }
    for root in (0..entries.len() / 2).rev() {
        sift(&mut entries, root, b)?;
    }
    for end in (1..entries.len()).rev() {
        b.charge(Resource::Work, 1)?;
        entries.swap(0, end);
        sift(&mut entries[..end], 0, b)?;
    }
    let mut values = Vec::new();
    let mut prior = None;
    for entry in entries {
        b.charge(Resource::Work, 33)?;
        if prior == Some(entry.digest) {
            let previous: &NdfValue = values.last().ok_or(WireError::InvalidType)?;
            if !previous.equal_with_budget(&entry.value, b)? {
                return Err(WireError::InvalidType);
            }
            continue;
        }
        prior = Some(entry.digest);
        push(&mut values, entry.value, b)?;
    }
    Ok(NdfValue::List(values))
}

/// Encode a self-contained set. Pool addresses are hashes of canonical content;
/// each root retains its exact source membership and mapping order. Native
/// declarations are indexed before encoding each distinct table entry once.
pub fn encode(
    bundles: &[SyntaxBundle],
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    let value = value(bundles.iter(), schema, registry, admission, b)?;
    crate::encode_checked(&value, &expected("SyntaxBundleSet"), registry, b)
}

pub(crate) fn value<'a>(
    bundles: impl IntoIterator<Item = &'a SyntaxBundle>,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    let (mut sources, mut maps, mut inputs) = (Vec::new(), Vec::new(), Vec::new());
    for bundle in bundles {
        b.charge(Resource::Work, 1)?;
        bundle.validate_with_sources(registry, b, admission)?;
        for source in &bundle.sources {
            push(&mut sources, source, b)?;
        }
        for mapping in &bundle.source_maps {
            push(&mut maps, mapping, b)?;
        }
        push(&mut inputs, bundle, b)?;
    }
    let sources = pool::unique_source_storage(sources, b)?;
    let sources = pool::Pool::new(sources, SOURCE_DOMAIN, schema, admission, b)?;
    let positions = pool::SourcePositions::new(&sources, b)?;
    let mut indexed_maps = Vec::new();
    for mapping in maps {
        push(&mut indexed_maps, positions.mapping(mapping, b)?, b)?;
    }
    let mut maps = Vec::new();
    for mapping in &indexed_maps {
        push(&mut maps, mapping, b)?;
    }
    let maps = pool::Pool::new(maps, MAP_DOMAIN, schema, admission, b)?;
    let mut remaining_maps = indexed_maps.as_slice();
    let mut members = Vec::new();
    for bundle in inputs {
        let source_refs = positions.references(&bundle.sources, b)?;
        let (selected, remaining) = remaining_maps
            .split_at_checked(bundle.source_maps.len())
            .ok_or(WireError::InvalidType)?;
        remaining_maps = remaining;
        let map_refs = maps.references(selected, false, b)?;
        let body = bundle_body_value(bundle, schema, registry, admission, b)?;
        let member = record(
            schema,
            "SharedSyntaxBundle",
            [source_refs, map_refs, body],
            b,
        )?;
        push(&mut members, member, b)?;
    }
    if !remaining_maps.is_empty() {
        return Err(WireError::InvalidType);
    }
    record(
        schema,
        "SyntaxBundleSet",
        [
            table(sources.values, b)?,
            table(maps.values, b)?,
            NdfValue::List(members),
        ],
        b,
    )
}

fn read_table(values: &NdfValue, domain: &[u8], b: &mut Budget) -> Result<Vec<Entry>, WireError> {
    let mut entries: Vec<Entry> = Vec::new();
    for value in list(values)? {
        b.charge(Resource::Work, 33)?;
        let digest = crate::encode::digest(domain, value, b)?;
        if entries.last().is_some_and(|entry| entry.digest >= digest) {
            return Err(WireError::NonCanonical);
        }
        let value = value.clone_with_budget(b)?;
        push(
            &mut entries,
            Entry {
                digest,
                value,
                used: false,
            },
            b,
        )?;
    }
    Ok(entries)
}

fn expand(refs: &NdfValue, entries: &mut [Entry], b: &mut Budget) -> Result<NdfValue, WireError> {
    let mut values = Vec::new();
    for reference in list(refs)? {
        b.charge(Resource::Work, 1)?;
        let at = locate(entries, as_digest(reference)?, b)?.map_err(|_| WireError::InvalidType)?;
        let entry = &mut entries[at];
        let value = entry.value.clone_with_budget(b)?;
        push(&mut values, value, b)?;
        entry.used = true;
    }
    Ok(NdfValue::List(values))
}

/// Decode without ambient sources. Only each member's selected declarations
/// reach its ordinary bundle decoder and graph/view validation. No partial
/// bundle set is returned on failure; admission and resource charges persist.
pub fn decode(
    input: &[u8],
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<SyntaxBundle>, WireError> {
    let input = crate::decode_checked(input, &expected("SyntaxBundleSet"), registry, b)?;
    from_value(input.value(), schema, registry, admission, b)
}

pub(crate) fn from_value(
    input: &NdfValue,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<SyntaxBundle>, WireError> {
    let fields = fields(input, schema, "SyntaxBundleSet", 3)?;
    let mut sources = read_table(&fields[0], SOURCE_DOMAIN, b)?;
    let mut maps = read_table(&fields[1], MAP_DOMAIN, b)?;
    let mut bundles = Vec::new();
    for member in list(&fields[2])? {
        b.charge(Resource::Work, 1)?;
        let f = super::fields(member, schema, "SharedSyntaxBundle", 3)?;
        let body = super::fields(&f[2], schema, "SyntaxBody", 5)?;
        let value = record(
            schema,
            "SyntaxBundle",
            [
                expand(&f[0], &mut sources, b)?,
                body[0].clone_with_budget(b)?,
                body[1].clone_with_budget(b)?,
                body[2].clone_with_budget(b)?,
                body[3].clone_with_budget(b)?,
                body[4].clone_with_budget(b)?,
                expand(&f[1], &mut maps, b)?,
            ],
            b,
        )?;
        let bundle = bundle_from(&value, schema, registry, admission, b)?;
        bundle.validate_with_sources(registry, b, admission)?;
        push(&mut bundles, bundle, b)?;
    }
    for entry in sources.iter().chain(&maps) {
        b.charge(Resource::Work, 1)?;
        if !entry.used {
            return Err(WireError::NonCanonical);
        }
    }
    Ok(bundles)
}
