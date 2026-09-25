//! Document-local owner tables. Native sharing only selects work to reuse;
//! portable order and references depend on checked contents and their digests.
use super::{PortableError, boundary, schema, value::*};
use crate::{check::StructureError, model::*};
use alloc::{boxed::Box, vec::Vec};
use core::cmp::Ordering;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::{Digest, SourceStore},
    syntax::{EnvironmentRef, ForeignClosure, ForeignSyntax, OwnerProvenance},
    value::{NdfValue, SchemaRef},
    value_codec::FoundationValueCodec,
};

pub const OWNER_DOMAIN: &[u8] = b"NEPL3.Doc.Owner.v1\0";

fn storage<T>(count: usize, b: &mut Budget) -> Result<Vec<T>, StopReason> {
    let bytes = count
        .checked_mul(core::mem::size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes)?;
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    Ok(out)
}

// In-place heapsort permits stopping before each comparison/swap. Portable
// sorting compares fixed-size digests; native grouping compares three slices.
fn sort<T>(
    items: &mut [T],
    compare: impl Fn(&T, &T) -> Ordering,
    b: &mut Budget,
) -> Result<(), StopReason> {
    fn sift<T>(
        items: &mut [T],
        mut root: usize,
        compare: &impl Fn(&T, &T) -> Ordering,
        b: &mut Budget,
    ) -> Result<(), StopReason> {
        while root < items.len() / 2 {
            b.charge(Resource::Work, 65)?;
            let mut child = root * 2 + 1;
            if child + 1 < items.len()
                && compare(&items[child], &items[child + 1]) == Ordering::Less
            {
                child += 1;
            }
            if compare(&items[root], &items[child]) != Ordering::Less {
                break;
            }
            b.charge(Resource::Work, 1)?;
            items.swap(root, child);
            root = child;
        }
        Ok(())
    }
    for root in (0..items.len() / 2).rev() {
        sift(items, root, &compare, b)?;
    }
    for end in (1..items.len()).rev() {
        b.charge(Resource::Work, 1)?;
        items.swap(0, end);
        sift(&mut items[..end], 0, &compare, b)?;
    }
    Ok(())
}

fn owner_key(owner: &OwnerProvenance) -> [(usize, usize); 3] {
    [
        (owner.origins().as_ptr() as usize, owner.origins().len()),
        (owner.sources().as_ptr() as usize, owner.sources().len()),
        (
            owner.source_maps().as_ptr() as usize,
            owner.source_maps().len(),
        ),
    ]
}

fn owner_value<C: FoundationValueCodec>(
    owner: &OwnerProvenance,
    s: &SchemaRef,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let sources = c.encode_sources(owner.sources(), b).map_err(boundary)?;
    let mut store = SourceStore::default();
    for source in owner.sources() {
        store
            .insert_with_budget(source.clone_with_budget(b)?, b)
            .map_err(StructureError::from)?;
    }
    let mut scoped = c.scoped_with_mappings(&store, owner.source_maps());
    let origins = scoped
        .encode_origins(owner.origins(), b)
        .map_err(boundary)?;
    let maps = scoped
        .encode_mappings(owner.source_maps(), b)
        .map_err(boundary)?;
    record(s, "DocOwner", [sources, origins, maps], b)
}

fn embed<C: FoundationValueCodec>(
    input: &DocEmbed,
    owner: Option<Digest>,
    syntax: Option<NdfValue>,
    s: &SchemaRef,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let content = match &input.content {
        DocContent::Value { value } => variant(s, "DocContent", "Value", [value.put(s, c, b)?], b)?,
        DocContent::Syntax { closure } => {
            let owner = owner.ok_or(PortableError::Shape)?.put(s, c, b)?;
            let identity = closure.syntax.schema.put(s, c, b)?;
            let category = closure.syntax.category.put(s, c, b)?;
            let syntax = syntax.ok_or(PortableError::Shape)?;
            let environment = c
                .encode_environment(&closure.owner_environment, b)
                .map_err(boundary)?;
            let closure = record(
                s,
                "DocClosure",
                [owner, identity, category, syntax, environment],
                b,
            )?;
            variant(s, "DocContent", "Syntax", [closure], b)?
        }
    };
    record(s, "DocEmbed", [input.kind.put(s, c, b)?, content], b)
}

fn syntax_parts<E>(
    mut value: NdfValue,
    schema: &SchemaRef,
) -> Result<(NdfValue, NdfValue, Vec<NdfValue>), PortableError<E>> {
    fields(&value, schema, "SyntaxBundleSet", 3)?;
    let NdfValue::Record(value) = &mut value else {
        return Err(PortableError::Shape);
    };
    let [sources, maps, mut members]: [NdfValue; 3] = core::mem::take(&mut value.fields)
        .try_into()
        .map_err(|_| PortableError::Shape)?;
    let NdfValue::List(members) = &mut members else {
        return Err(PortableError::Shape);
    };
    Ok((sources, maps, core::mem::take(members)))
}

/// Compact identity input. Syntax owners are bound by content digest; decoding
/// such an embed requires its enclosing DocValue owner table.
pub(super) fn embed_value<C: FoundationValueCodec>(
    input: &DocEmbed,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let s = schema(r)?;
    let owner = match &input.content {
        DocContent::Syntax { closure } => {
            closure
                .validate(r, b, c.source_admission())
                .map_err(StructureError::from)?;
            let value = owner_value(&closure.provenance, s, c, b)?;
            Some(
                c.canonical_value_digest(OWNER_DOMAIN, &value, b)
                    .map_err(boundary)?,
            )
        }
        DocContent::Value { value } => {
            r.validate_typed(value, b)?;
            None
        }
    };
    let syntax = match &input.content {
        DocContent::Syntax { closure } => {
            let value = c
                .encode_syntax_set(&[&closure.syntax.bundle], b)
                .map_err(boundary)?;
            let (_, _, mut members) = syntax_parts(value, c.foundation_schema())?;
            if members.len() != 1 {
                return Err(PortableError::Shape);
            }
            members.pop()
        }
        DocContent::Value { .. } => None,
    };
    embed(input, owner, syntax, s, c, b)
}

pub(super) fn put<C: FoundationValueCodec>(
    input: &DocValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let s = schema(r)?;
    let mut closures = storage(input.embeds.len(), b)?;
    let mut references = storage(input.embeds.len(), b)?;
    for (index, input) in input.embeds.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        references.push(None);
        if let DocContent::Syntax { closure } = &input.content {
            closures.push((index, closure));
        }
    }
    sort(
        &mut closures,
        |a, z| owner_key(&a.1.provenance).cmp(&owner_key(&z.1.provenance)),
        b,
    )?;
    let mut owners = storage(closures.len(), b)?;
    let mut at = 0;
    while at < closures.len() {
        let owner = &closures[at].1.provenance;
        let checked = owner
            .validate(r, b, c.source_admission())
            .map_err(StructureError::from)?;
        let value = owner_value(owner, s, c, b)?;
        let digest = c
            .canonical_value_digest(OWNER_DOMAIN, &value, b)
            .map_err(boundary)?;
        let key = owner_key(owner);
        while at < closures.len() {
            b.charge(Resource::Work, 7)?;
            let (index, closure) = closures[at];
            if owner_key(&closure.provenance) != key {
                break;
            }
            checked
                .validate_closure(closure, b, c.source_admission())
                .map_err(StructureError::from)?;
            references[index] = Some(digest);
            at += 1;
        }
        owners.push((digest, value));
    }
    sort(&mut owners, |a, z| a.0.cmp(&z.0), b)?;
    let mut owner_values: Vec<NdfValue> = storage(owners.len(), b)?;
    let mut prior = None;
    for (digest, value) in owners {
        b.charge(Resource::Work, 33)?;
        if prior == Some(digest) {
            if !owner_values
                .last()
                .ok_or(PortableError::Shape)?
                .equal_with_budget(&value, b)?
            {
                return Err(PortableError::Shape);
            }
        } else {
            owner_values.push(value);
            prior = Some(digest);
        }
    }
    // Keep the original embed order, independently of owner grouping above.
    let mut bundles = storage(closures.len(), b)?;
    for input in &input.embeds {
        b.charge(Resource::Work, 1)?;
        if let DocContent::Syntax { closure } = &input.content {
            bundles.push(&closure.syntax.bundle);
        }
    }
    let syntax = c.encode_syntax_set(&bundles, b).map_err(boundary)?;
    let (syntax_sources, syntax_maps, members) = syntax_parts(syntax, c.foundation_schema())?;
    let mut members = members.into_iter();
    let mut embeds = storage(input.embeds.len(), b)?;
    for (input, owner) in input.embeds.iter().zip(references) {
        b.charge(Resource::Work, 1)?;
        let syntax = match &input.content {
            DocContent::Syntax { .. } => Some(members.next().ok_or(PortableError::Shape)?),
            DocContent::Value { .. } => None,
        };
        embeds.push(embed(input, owner, syntax, s, c, b)?);
    }
    if members.next().is_some() {
        return Err(PortableError::Shape);
    }
    record(
        s,
        "DocValue",
        [
            input.root.put(s, c, b)?,
            input.nodes.put(s, c, b)?,
            NdfValue::List(embeds),
            NdfValue::List(owner_values),
            syntax_sources,
            syntax_maps,
        ],
        b,
    )
}

fn list<E>(value: &NdfValue) -> Result<&[NdfValue], PortableError<E>> {
    match value {
        NdfValue::List(values) => Ok(values),
        _ => Err(PortableError::Shape),
    }
}

fn find<E>(
    owners: &[(Digest, OwnerProvenance)],
    wanted: Digest,
    b: &mut Budget,
) -> Result<usize, PortableError<E>> {
    let (mut low, mut high) = (0, owners.len());
    while low < high {
        b.charge(Resource::Work, 33)?;
        let middle = low + (high - low) / 2;
        match owners[middle].0.cmp(&wanted) {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => return Ok(middle),
        }
    }
    Err(PortableError::Shape)
}

pub(super) fn read<C: FoundationValueCodec>(
    input: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<DocValue, PortableError<C::Error>> {
    let s = schema(r)?;
    let f = fields(input, s, "DocValue", 6)?;
    let mut members = storage(list(&f[2])?.len(), b)?;
    for embed in list(&f[2])? {
        b.charge(Resource::Work, 1)?;
        let fields = fields(embed, s, "DocEmbed", 2)?;
        let (tag, parts) = case(&fields[1], s, "DocContent")?;
        let [part] = parts else {
            return Err(PortableError::Shape);
        };
        match tag {
            "Syntax" => {
                let fields = super::value::fields(part, s, "DocClosure", 5)?;
                members.push(fields[3].clone_with_budget(b)?);
            }
            "Value" => (),
            _ => return Err(PortableError::Shape),
        }
    }
    let set = record(
        c.foundation_schema(),
        "SyntaxBundleSet",
        [
            f[4].clone_with_budget(b)?,
            f[5].clone_with_budget(b)?,
            NdfValue::List(members),
        ],
        b,
    )?;
    let mut bundles = c.decode_syntax_set(&set, b).map_err(boundary)?.into_iter();
    let values = list(&f[3])?;
    let mut owners: Vec<(Digest, OwnerProvenance)> = storage(values.len(), b)?;
    let mut used = storage(values.len(), b)?;
    for value in values {
        b.charge(Resource::Work, 33)?;
        let digest = c
            .canonical_value_digest(OWNER_DOMAIN, value, b)
            .map_err(boundary)?;
        if owners.last().is_some_and(|(prior, _)| *prior >= digest) {
            return Err(PortableError::Shape);
        }
        let f = fields(value, s, "DocOwner", 3)?;
        let sources = c.decode_sources(&f[0], b).map_err(boundary)?;
        let mut store = SourceStore::default();
        for source in &sources {
            store
                .insert_with_budget(source.clone_with_budget(b)?, b)
                .map_err(StructureError::from)?;
        }
        let mut scoped = c.scoped(&store);
        let origins = scoped.decode_origins(&f[1], b).map_err(boundary)?;
        let maps = scoped.decode_mappings(&f[2], b).map_err(boundary)?;
        let owner = OwnerProvenance::new(origins, sources, maps, b)?;
        owner
            .validate(r, b, scoped.source_admission())
            .map_err(StructureError::from)?;
        owners.push((digest, owner));
        used.push(false);
    }
    let values = list(&f[2])?;
    let mut embeds = storage(values.len(), b)?;
    for value in values {
        b.charge(Resource::Work, 1)?;
        let f = fields(value, s, "DocEmbed", 2)?;
        let kind = Value::read(&f[0], s, c, b)?;
        let (tag, parts) = case(&f[1], s, "DocContent")?;
        let [part] = parts else {
            return Err(PortableError::Shape);
        };
        let content = match tag {
            "Value" => DocContent::Value {
                value: Value::read(part, s, c, b)?,
            },
            "Syntax" => {
                let f = fields(part, s, "DocClosure", 5)?;
                let index = find(&owners, Digest::read(&f[0], s, c, b)?, b)?;
                let schema = Value::read(&f[1], s, c, b)?;
                let category = Value::read(&f[2], s, c, b)?;
                let bundle = bundles.next().ok_or(PortableError::Shape)?;
                let owner_environment = c.decode_environment(&f[4], b).map_err(boundary)?;
                let syntax = ForeignSyntax {
                    schema,
                    category,
                    root: bundle.root,
                    bundle,
                    environment: EnvironmentRef {
                        id: owner_environment.id,
                        digest: owner_environment.digest,
                    },
                };
                used[index] = true;
                b.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<ForeignClosure>() as u64,
                )?;
                DocContent::Syntax {
                    closure: Box::new(ForeignClosure {
                        syntax,
                        owner_environment,
                        provenance: owners[index].1.clone_with_budget(b)?,
                    }),
                }
            }
            _ => return Err(PortableError::Shape),
        };
        embeds.push(DocEmbed { kind, content });
    }
    for used in used {
        b.charge(Resource::Work, 1)?;
        if !used {
            return Err(PortableError::Shape);
        }
    }
    if bundles.next().is_some() {
        return Err(PortableError::Shape);
    }
    Ok(DocValue {
        root: Value::read(&f[0], s, c, b)?,
        nodes: Value::read(&f[1], s, c, b)?,
        embeds,
    })
}
