//! Operation-level environment preparation; syntax nodes do not trigger NDF round trips.
use super::{
    build::{slot, span, text},
    model::LanguageEnvironment,
};
use crate::profile::{ProfileError, ResolvedParseProfile};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    origin::{Origin, OriginId},
    source::SourceStore,
    syntax::{Environment, EnvironmentBinding, EnvironmentEntry, NamespaceRef, ResourceContent},
    value_codec::FoundationValueCodec,
};
use nepl3_reader::{
    context::{CheckedReaderContext, ContextError},
    model::ReaderContext,
};

pub struct EnvironmentInput<'a> {
    pub alias: &'a str,
    pub context: &'a CheckedReaderContext<'a>,
}
pub struct ParseEnvironmentSet<'a> {
    pub(super) languages: Vec<LanguageEnvironment<'a>>,
    pub(super) origins: Vec<Origin>,
    pub(super) entries: Vec<EnvironmentEntry>,
}
#[derive(Debug)]
pub enum EnvironmentError<E> {
    Stopped(StopReason),
    Profile(ProfileError),
    Context(ContextError<E>),
    Projection(ContextError<core::convert::Infallible>),
    Boundary(E),
    MissingAlias,
    DuplicateAlias,
    Reference,
}
impl<E> From<StopReason> for EnvironmentError<E> {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl<E> From<ProfileError> for EnvironmentError<E> {
    fn from(v: ProfileError) -> Self {
        Self::Profile(v)
    }
}
fn offset(value: OriginId, base: u64) -> Result<OriginId, StopReason> {
    value
        .0
        .checked_add(base)
        .map(OriginId)
        .ok_or(StopReason::NodeLimit)
}
fn ids(values: &[OriginId], base: u64, budget: &mut Budget) -> Result<Vec<OriginId>, StopReason> {
    let mut out = Vec::new();
    for value in values {
        slot::<OriginId>(budget)?;
        budget.charge(Resource::Work, 1)?;
        out.push(offset(*value, base)?);
    }
    Ok(out)
}
pub(super) fn origins(
    values: &[Origin],
    base: u64,
    budget: &mut Budget,
) -> Result<Vec<Origin>, StopReason> {
    let mut out = Vec::new();
    for value in values {
        slot::<Origin>(budget)?;
        budget.charge(Resource::Work, 1)?;
        out.push(match value {
            Origin::Direct(v) => Origin::Direct(span(v, budget)?),
            Origin::Composite(v) => Origin::Composite(ids(v, base, budget)?),
            Origin::Synthetic { reason, anchor } => Origin::Synthetic {
                reason: text(reason, budget)?,
                anchor: anchor.as_ref().map(|v| span(v, budget)).transpose()?,
            },
            Origin::Generated {
                operation,
                callsite,
                inputs,
            } => {
                budget.charge(
                    Resource::AllocationUnits,
                    operation.schema.package.len() as u64 + operation.name.len() as u64,
                )?;
                Origin::Generated {
                    operation: operation.clone(),
                    callsite: callsite.as_ref().map(|v| span(v, budget)).transpose()?,
                    inputs: ids(inputs, base, budget)?,
                }
            }
        });
    }
    Ok(out)
}
fn environment(
    value: &Environment,
    base: u64,
    budget: &mut Budget,
) -> Result<Environment, StopReason> {
    let mut bindings = Vec::new();
    let mut resources = Vec::new();
    for binding in &value.bindings {
        slot::<EnvironmentBinding>(budget)?;
        budget.charge(
            Resource::AllocationUnits,
            binding.namespace.schema.package.len() as u64,
        )?;
        bindings.push(EnvironmentBinding {
            namespace: NamespaceRef {
                schema: binding.namespace.schema.clone(),
                name: text(&binding.namespace.name, budget)?,
            },
            name: text(&binding.name, budget)?,
            value: binding.value.clone_with_budget(budget)?,
            origin: binding.origin.map(|v| offset(v, base)).transpose()?,
        });
    }
    for resource in &value.resources {
        slot::<ResourceContent>(budget)?;
        budget.charge(Resource::AllocationUnits, resource.bytes.len() as u64)?;
        budget.charge(Resource::Work, resource.bytes.len() as u64)?;
        resources.push(ResourceContent {
            id: text(&resource.id, budget)?,
            digest: resource.digest,
            bytes: resource.bytes.clone(),
        });
    }
    Ok(Environment {
        bindings,
        resources,
    })
}
impl<'a> ParseEnvironmentSet<'a> {
    pub fn prepare<C: FoundationValueCodec>(
        profile: &ResolvedParseProfile<'_>,
        inputs: &[EnvironmentInput<'_>],
        sources: &'a SourceStore,
        codec: &mut C,
        budget: &mut Budget,
    ) -> Result<Self, EnvironmentError<C::Error>> {
        let mut table = Vec::new();
        let mut prepared = Vec::new();
        let mut ordered = Vec::new();
        for registration in &profile.profile().languages {
            slot::<&crate::profile::LanguageRegistration>(budget)?;
            ordered.push(registration);
        }
        budget.charge(
            Resource::Work,
            ordered.len() as u64
                * ordered
                    .iter()
                    .map(|v| v.alias.len() as u64 + 1)
                    .sum::<u64>(),
        )?;
        ordered.sort_unstable_by(|a, b| a.alias.cmp(&b.alias));
        for registration in ordered {
            budget.charge(
                Resource::Work,
                inputs.len() as u64 * (registration.alias.len() as u64 + 1),
            )?;
            let mut matches = inputs.iter().filter(|v| v.alias == registration.alias);
            let input = matches.next().ok_or(EnvironmentError::MissingAlias)?;
            if matches.next().is_some() {
                return Err(EnvironmentError::DuplicateAlias);
            }
            let base = table.len() as u64;
            let part = origins(&input.context.origins, base, budget)?;
            budget.charge(
                Resource::AllocationUnits,
                part.len() as u64 * core::mem::size_of::<Origin>() as u64,
            )?;
            table.extend(part);
            let value = environment(&input.context.environment.value, base, budget)?;
            let digest = codec
                .environment_digest(&value, budget)
                .map_err(EnvironmentError::Boundary)?;
            slot::<(String, EnvironmentEntry)>(budget)?;
            prepared.push((
                text(&registration.alias, budget)?,
                EnvironmentEntry {
                    id: prepared.len() as u64,
                    digest,
                    value,
                },
            ));
        }
        if inputs.len() != prepared.len() {
            return Err(EnvironmentError::MissingAlias);
        }
        let mut languages = Vec::new();
        let mut entries = Vec::new();
        for (alias, entry) in prepared {
            let selected = profile.entry(&alias, None, budget)?;
            let raw = ReaderContext {
                schema: selected.package.schema,
                category: selected.category,
                mode: selected.mode,
                origins: origins(&table, 0, budget)?,
                environment: entry,
            };
            let proof = raw
                .check(codec, sources, profile.registry(), budget)
                .map_err(EnvironmentError::Context)?;
            let owned = proof
                .retarget(
                    &raw.schema,
                    &raw.category,
                    &raw.mode,
                    sources,
                    profile.registry(),
                    budget,
                )
                .map_err(EnvironmentError::Projection)?;
            slot::<EnvironmentEntry>(budget)?;
            entries.push(EnvironmentEntry {
                id: raw.environment.id,
                digest: raw.environment.digest,
                value: environment(&raw.environment.value, 0, budget)?,
            });
            slot::<LanguageEnvironment<'a>>(budget)?;
            languages.push(LanguageEnvironment {
                alias,
                context: owned,
            });
        }
        Ok(Self {
            languages,
            origins: table,
            entries,
        })
    }
    pub fn languages(&self) -> &[LanguageEnvironment<'a>] {
        &self.languages
    }
}
