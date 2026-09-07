//! Persistent static selections and recovery retain their bundle-local owners
//! through the same canonical node mapping used by the core syntax codec.
mod canonical;
mod selection;
use super::{PortableError, boundary, value::*};
use crate::{profile::ResolvedParseProfile, recovery::*, selection::*};
use alloc::vec::Vec;
use canonical::Mappings;
use nepl3_core::{
    budget::{Budget, Resource},
    source::SourceStore,
    value::NdfValue,
    value_codec::FoundationValueCodec,
};

pub fn to_value<C: FoundationValueCodec>(
    tree: &ParseTree,
    profile: &ResolvedParseProfile<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    validate(tree, profile, codec, b)?;
    let s = Schemas::new(profile.registry())?;
    let mappings = Mappings::new(&tree.bundle, b)?;
    let mut contexts = Vec::new();
    let mut recoveries = Vec::new();
    for mapping in &mappings.entries {
        let bundle = mapping.bundle();
        let mut context = None;
        for item in &tree.contexts {
            b.charge(Resource::Work, 1)?;
            if core::ptr::eq(
                crate::tree::path(&tree.bundle, &item.path, profile.registry(), b)?,
                bundle,
            ) {
                context = Some(item);
                break;
            }
        }
        let context = context.ok_or(PortableError::Shape)?;
        let (path, _) = mappings.path_value(&tree.bundle, &context.path, &s, profile, codec, b)?;
        let mut nodes = Vec::new();
        for old in mapping.order() {
            let mut selected = None;
            for item in &context.nodes {
                b.charge(Resource::Work, 1)?;
                if item.node.0 == *old as u64 {
                    selected = Some(item);
                    break;
                }
            }
            let item = selected.ok_or(PortableError::Shape)?;
            push(
                &mut nodes,
                record(
                    s.engine,
                    "NodeSelection",
                    [
                        mapping.mapped(item.node)?.value(&s, codec, b)?,
                        item.entry.value(&s, codec, b)?,
                        item.execution_digest.value(&s, codec, b)?,
                        item.shape.value(&s, codec, b)?,
                    ],
                    b,
                )?,
                b,
            )?;
        }
        push(
            &mut contexts,
            record(s.engine, "BundleContext", [path, NdfValue::List(nodes)], b)?,
            b,
        )?;
        for recovery in &tree.recovery {
            b.charge(Resource::Work, 1)?;
            if !core::ptr::eq(
                crate::tree::path(&tree.bundle, &recovery.path, profile.registry(), b)?,
                bundle,
            ) {
                continue;
            }
            let (path, _) =
                mappings.path_value(&tree.bundle, &recovery.path, &s, profile, codec, b)?;
            let sources = declared(&bundle.sources, b)?;
            let mut local = codec.scoped(&sources);
            let mut entries = Vec::new();
            for old in mapping.order() {
                for item in &recovery.entries {
                    b.charge(Resource::Work, 1)?;
                    if item.node.0 == *old as u64 {
                        push(
                            &mut entries,
                            record(
                                s.engine,
                                "RecoveryEntry",
                                [
                                    mapping.mapped(item.node)?.value(&s, &mut local, b)?,
                                    item.kind.value(&s, &mut local, b)?,
                                ],
                                b,
                            )?,
                            b,
                        )?;
                    }
                }
            }
            push(
                &mut recoveries,
                record(
                    s.engine,
                    "BundleRecovery",
                    [path, NdfValue::List(entries)],
                    b,
                )?,
                b,
            )?;
        }
    }
    let value = record(
        s.engine,
        "ParseTree",
        [
            tree.profile_digest.value(&s, codec, b)?,
            codec.encode_syntax(&tree.bundle, b).map_err(boundary)?,
            NdfValue::List(recoveries),
            NdfValue::List(contexts),
        ],
        b,
    )?;
    profile
        .registry()
        .validate(&expected("ParseTree", b)?, &value, b)?;
    Ok(value)
}

pub fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    profile: &ResolvedParseProfile<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<ParseTree, PortableError<C::Error>> {
    profile
        .registry()
        .validate(&expected("ParseTree", b)?, value, b)?;
    let s = Schemas::new(profile.registry())?;
    let f = fields(value, s.engine, "ParseTree", 4)?;
    let bundle = codec.decode_syntax(&f[1], b).map_err(boundary)?;
    let mut contexts = Vec::new();
    let mut recovery = Vec::new();
    for value in list(&f[3])? {
        let f = fields(value, s.engine, "BundleContext", 2)?;
        push(
            &mut contexts,
            BundleContext {
                path: Value::read(&f[0], &s, codec, b)?,
                nodes: Value::read(&f[1], &s, codec, b)?,
            },
            b,
        )?;
    }
    for value in list(&f[2])? {
        let f = fields(value, s.engine, "BundleRecovery", 2)?;
        let path = Vec::<ForeignStep>::read(&f[0], &s, codec, b)?;
        let owner = crate::tree::path(&bundle, &path, profile.registry(), b)?;
        let sources = declared(&owner.sources, b)?;
        let mut local = codec.scoped(&sources);
        push(
            &mut recovery,
            BundleRecovery {
                path,
                entries: Value::read(&f[1], &s, &mut local, b)?,
            },
            b,
        )?;
    }
    let tree = ParseTree {
        profile_digest: Value::read(&f[0], &s, codec, b)?,
        bundle,
        recovery,
        contexts,
    };
    validate(&tree, profile, codec, b)?;
    Mappings::new(&tree.bundle, b)?.check_tables(&tree, profile, b)?;
    Ok(tree)
}
fn validate<C: FoundationValueCodec>(
    tree: &ParseTree,
    profile: &ResolvedParseProfile<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<(), PortableError<C::Error>> {
    tree.validate(profile, b, codec.source_admission())
        .map(|_| ())
        .map_err(|error| {
            // Native validators retain some nested error wrappers. The same
            // operation's sticky Budget is authoritative for a resource stop.
            match b.poll() {
                Err(reason) => PortableError::Stopped(reason),
                Ok(()) => error.into(),
            }
        })
}
pub(super) fn declared<E>(
    values: &[nepl3_core::source::SourceSnapshot],
    b: &mut Budget,
) -> Result<SourceStore, PortableError<E>> {
    let mut out = SourceStore::default();
    for source in values {
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<nepl3_core::source::SourceSnapshot>() as u64,
        )?;
        out.insert(source.clone_with_budget(b)?)?;
    }
    Ok(out)
}
