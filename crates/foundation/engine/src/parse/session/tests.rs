use super::*;
use crate::{
    package::{EntryContext, PackageIdentity},
    profile::{ParseProfile, RuntimeCatalog},
};
use alloc::format;
use nepl3_core::{
    schema::{NamedType, SchemaDescriptor, SchemaRegistry, TypeShape},
    source::{Digest, SourceId},
    value::SchemaRef,
};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        allocation_units: 10_000_000,
        nodes: 100_000,
        source_bytes: 100_000,
        depth: 100,
        diagnostics: 100,
        ..Limits::default()
    })
}
#[test]
fn recovery_lookup_stops_preserve_complete_progress() -> Result<(), ParseError> {
    for padding in [0, 32] {
        let mut registry = SchemaRegistry::default();
        for i in 0..=padding {
            let d = SchemaDescriptor {
                package: if i == padding {
                    "nepl3.engine".into()
                } else {
                    format!("pad{i:02}")
                },
                revision: 1,
                types: if i == padding {
                    ["RecoveryUnparsed", "RecoveryMissing"]
                        .iter()
                        .map(|n| NamedType {
                            name: (*n).into(),
                            shape: TypeShape::Record { fields: vec![] },
                            constraints: vec![],
                        })
                        .collect()
                } else {
                    vec![]
                },
                operations: vec![],
            };
            registry.register(d.reference(&mut budget())?, d, &mut budget())?;
        }
        registry.finalize(&mut budget())?;
        let profile = ParseProfile {
            id: "lookup-test".into(),
            languages: vec![],
            schemas: vec![],
            category_modes: vec![],
            head_providers: vec![],
            providers: vec![],
            allowlist: vec![],
            resources: vec![],
            limits: budget().limits(),
        };
        let catalog = RuntimeCatalog {
            packages: &[],
            providers: &[],
            resources: &[],
        };
        let resolved = profile.resolve(&catalog, &registry, &mut budget())?;
        let environments = ParseEnvironmentSet {
            languages: vec![],
            origins: vec![],
            entries: vec![],
        };
        let session = ParseSession::new("test".into(), &resolved, &environments, &mut budget())?;
        let source = SourceSnapshot::new(
            SourceId("s".into()),
            0,
            "mem:s".into(),
            b"bad".to_vec(),
            &mut budget(),
        )?;
        let snapshot = source.reference();
        let mut sources = SourceStore::default();
        sources.insert(source.clone())?;
        let entry = EntryContext {
            package: PackageIdentity {
                schema: SchemaRef {
                    package: "fixture".into(),
                    revision: 1,
                    digest: Digest([0; 32]),
                },
                semantic_digest: Digest([1; 32]),
            },
            alias: "X".into(),
            category: "Node".into(),
            mode: "Code".into(),
        };
        let progress = ParseProgress {
            request: OwnedParseRequest {
                snapshot: snapshot.clone(),
                start: 0,
                limit: 3,
                final_input: true,
                entry: entry.clone(),
                states: vec![],
                environments: vec![],
                environment_entries: vec![],
                origins: vec![],
                sources: vec![source.clone()],
            },
            scope: TokenizationScope {
                operation_id: "op".into(),
                profile_digest: Digest([2; 32]),
                snapshot,
            },
            cursor: 0,
            states: vec![],
            arenas: vec![ParseArena {
                sources: vec![source],
                ..ParseArena::default()
            }],
            frames: vec![ParseFrame {
                entry,
                read: None,
                selection: None,
                node: None,
                arity: 1,
                children: vec![],
                next_child: 0,
                arena: 0,
                foreign: false,
            }],
            facts: vec![],
            recovery: vec![],
            contexts: vec![],
        };
        for missing in [false, true] {
            let name = if missing {
                "RecoveryMissing"
            } else {
                "RecoveryUnparsed"
            };
            let selection = 1 + padding as u64 * (5 + 12 + 9) + (12 + 12 + 9);
            let descriptor = selection + 12 + 41;
            // Two sorted types: first probe Unparsed; Missing then probes index 0.
            let probes = ("RecoveryUnparsed".len() + name.len() + 1) as u64
                + if missing {
                    (2 * name.len() + 1) as u64
                } else {
                    0
                };
            for work in [
                0,
                selection - 1,
                selection,
                selection + descriptor - 1,
                selection + descriptor,
                selection + descriptor + probes - 1,
            ] {
                let mut machine = Machine {
                    progress: progress.clone(),
                    accepted: None,
                    imported: vec![],
                };
                let mut b = Budget::new(Limits {
                    work,
                    ..budget().limits()
                });
                let result = session.recover(
                    &mut machine,
                    None,
                    missing,
                    &sources,
                    &mut b,
                    &mut SourceAdmission::default(),
                );
                assert_eq!(
                    result.err().and_then(|e| e.stop_reason()),
                    Some(StopReason::WorkLimit)
                );
                assert_eq!(machine.progress, progress);
                assert_eq!(b.usage().nodes, 0);
                let copied = &progress.frames[0].entry;
                let entry_allocation = (core::mem::size_of::<EntryContext>()
                    + copied.alias.len()
                    + copied.category.len()
                    + copied.mode.len()
                    + copied.package.schema.package.len())
                    as u64;
                assert_eq!(b.usage().allocation_units, entry_allocation);
            }
            let mut machine = Machine {
                progress: progress.clone(),
                accepted: None,
                imported: vec![],
            };
            let mut b = Budget::new(Limits {
                allocation_units: 0,
                work: 0,
                ..budget().limits()
            });
            assert_eq!(
                session.recover(
                    &mut machine,
                    None,
                    missing,
                    &sources,
                    &mut b,
                    &mut SourceAdmission::default()
                ),
                Err(ParseError::Stopped(StopReason::AllocationLimit))
            );
            assert_eq!(machine.progress, progress);
        }
    }
    Ok(())
}
