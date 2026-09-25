//! Shared development-host parsing of an explicit resolved Profile.
use nepl3_core::{
    budget::*,
    source::*,
    syntax::{Environment, EnvironmentEntry},
    value::NdfValue,
};
use nepl3_engine::{parse::*, profile::*};
use nepl3_reader::model::{ProviderCall, ReaderContext};
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};
fn err(v: impl std::fmt::Debug) -> String {
    format!("{v:?}")
}

pub fn reservation_prefix(source: &SourceSnapshot, b: &mut Budget) -> Result<String, String> {
    // One allocator namespace per source identity. A counter alone collides
    // when independently parsed pages enter the same source bundle.
    let identity = source.identity();
    let prefix_bound = identity.source.0.len() as u64 + 43;
    b.charge(Resource::Work, prefix_bound).map_err(err)?;
    b.charge(Resource::AllocationUnits, prefix_bound)
        .map_err(err)?;
    let prefix = format!(
        "{}:{}:{}:",
        identity.source.0.len(),
        identity.source.0,
        identity.revision
    );
    Ok(prefix)
}

#[allow(clippy::too_many_arguments)]
pub fn parse(
    source: &SourceSnapshot,
    resolved: &ResolvedParseProfile<'_>,
    alias: &str,
    category: &str,
    b: &mut Budget,
    a: &mut SourceAdmission,
    native: bool,
    session_id: &str,
    prefix: &str,
    host: &mut impl ParseHost,
) -> Result<nepl3_engine::recovery::ParseTree, String> {
    parse_validated(
        source, resolved, alias, category, b, a, native, session_id, prefix, host,
    )
    .map(nepl3_engine::tree::OwnedValidatedParseTree::into_inner)
}

#[allow(clippy::too_many_arguments)]
pub fn parse_validated<'p>(
    source: &SourceSnapshot,
    resolved: &'p ResolvedParseProfile<'p>,
    alias: &str,
    category: &str,
    b: &mut Budget,
    a: &mut SourceAdmission,
    native: bool,
    session_id: &str,
    prefix: &str,
    host: &mut impl ParseHost,
) -> Result<nepl3_engine::tree::OwnedValidatedParseTree<'p>, String> {
    let r = resolved.registry();
    let foundation = r.selected("nepl3.foundation", 1).ok_or("foundation")?;
    let mut store = SourceStore::default();
    store.insert(source.clone()).map_err(err)?;
    store.prepare_scope(b).map_err(err)?;
    let value = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = environment_digest(&value, foundation, r, b).map_err(err)?;
    let mut contexts = Vec::new();
    for language in &resolved.profile().languages {
        let entry = resolved.entry(&language.alias, None, b).map_err(err)?;
        contexts.push(ReaderContext {
            schema: entry.package.schema,
            category: entry.category,
            mode: entry.mode,
            origins: vec![],
            environment: EnvironmentEntry {
                id: 0,
                digest,
                value: value.clone(),
            },
        });
    }
    let environments = {
        let mut codec = FoundationCodec::new(r, &store, a).map_err(err)?;
        let checked = contexts
            .iter()
            .map(|raw| raw.check(&mut codec, &store, r, b).map_err(err))
            .collect::<Result<Vec<_>, String>>()?;
        let inputs = checked
            .iter()
            .zip(&resolved.profile().languages)
            .map(|(context, language)| EnvironmentInput {
                alias: &language.alias,
                context,
            })
            .collect::<Vec<_>>();
        ParseEnvironmentSet::prepare(resolved, &inputs, &store, &mut codec, b)
            .map_err(|e| format!("environments: {e:?}"))?
    };
    let entry = resolved.entry(alias, Some(category), b).map_err(err)?;
    let states = resolved
        .profile()
        .languages
        .iter()
        .map(|language| LanguageReaderState {
            alias: language.alias.clone(),
            state: NdfValue::Unit,
        })
        .collect::<Vec<_>>();
    let mut parser =
        ParseSession::new(session_id.into(), resolved, &environments, b).map_err(err)?;
    let request = ParseRequest {
        snapshot: source,
        start: 0,
        limit: source.text().len() as u64,
        final_input: true,
        entry: &entry,
        states: &states,
    };
    let mut result = if native {
        let reply = parser
            .read_with_host_validated(request, &store, b, a, host)
            .map_err(err)?;
        if let Some(error) = reply.host_error {
            let cursor = match &reply.reply.outcome {
                ParseOutcome::Stopped { progress, .. } => progress.as_ref().map(|p| p.cursor),
                _ => None,
            };
            return Err(format!(
                "native host: {error:?}; cursor={cursor:?}; usage={:?}",
                b.usage()
            ));
        }
        reply.reply
    } else {
        parser.read_validated(request, &store, b, a).map_err(err)?
    };
    let mut reservations = 0;
    loop {
        match result.outcome {
            ParseOutcome::Await { call, continuation } => {
                let ProviderCall::Read {
                    operation,
                    depth_base,
                    ..
                } = call.as_ref()
                else {
                    return Err("unexpected provider".into());
                };
                let requirement = resolved.provider(operation, b).map_err(err)?;
                let terminal = b
                    .with_depth_at_least(*depth_base, |b| host.provider(&call, requirement, b, a))
                    .map_err(err)?
                    .ok_or("unavailable reader provider")?;
                result = parser
                    .resume_validated(&continuation, terminal, &store, b, a)
                    .map_err(err)?;
            }
            ParseOutcome::Reserve { continuation, .. } => {
                reservations += 1;
                let reserved = SourceReservation {
                    source_id: SourceId(format!("{prefix}{reservations}")),
                    revision: 0,
                    uri: format!("memory:{prefix}{reservations}"),
                };
                result = parser
                    .reserve_validated(&continuation, &reserved, &store, b, a)
                    .map_err(err)?;
            }
            ParseOutcome::Complete { tree, cursor, .. } => {
                // A prefix parse ends after its root, before trailing source
                // whitespace. Keep the original snapshot, including file LF.
                let tail = usize::try_from(cursor)
                    .ok()
                    .and_then(|offset| source.text().get(offset..))
                    .ok_or("invalid completed source cursor")?;
                if !tail
                    .bytes()
                    .all(|c| matches!(c, b' ' | b'\t' | b'\r' | b'\n'))
                {
                    return Err(format!("unexpected trailing input at byte {cursor}"));
                }
                return Ok(tree);
            }
            ParseOutcome::Stopped { reason, progress } => {
                return Err(format!(
                    "candidate stopped: {reason:?}; cursor={:?}; usage={:?}",
                    progress.as_ref().map(|p| p.cursor),
                    b.usage()
                ));
            }
            other => return Err(format!("candidate: {other:?}")),
        }
    }
}
