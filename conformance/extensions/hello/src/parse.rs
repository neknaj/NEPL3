use super::*;
use nepl3_core::{source::*, syntax::*, value::NdfValue};
use nepl3_engine::{parse::*, profile::*};
use nepl3_reader::model::ReaderContext;
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};

pub(crate) struct Languages<'a> {
    pub packages: Vec<(&'a str, LanguagePackage)>,
    pub registry: SchemaRegistry,
}

/// Parse a Hello input using the example's public foundation dependencies.
/// `final_input` distinguishes a complete input from an unfinished input buffer.
/// Recovery, suspension and resource stops remain explicit in `ParseReply`.
pub fn parse(input: &str, final_input: bool) -> Result<ParseReply, String> {
    with_reply(input, final_input, false, |reply, _, _, _| Ok(reply))
}

// The test callback checks portable exchange while the resolved profile and
// codec still borrow their inputs. The observer uses the same parse operation.
pub(crate) fn with_reply<T>(
    input: &str,
    final_input: bool,
    stopped: bool,
    inspect: impl FnOnce(
        ParseReply,
        &ResolvedParseProfile<'_>,
        &SchemaRegistry,
        &mut FoundationCodec<'_>,
    ) -> Result<T, String>,
) -> Result<T, String> {
    with_language(
        input,
        final_input,
        stopped,
        language()?,
        ("Hello", "hello"),
        inspect,
    )
}

pub(crate) fn with_language<T>(
    input: &str,
    final_input: bool,
    stopped: bool,
    language: (LanguagePackage, SchemaRegistry),
    identity: (&str, &str),
    inspect: impl FnOnce(
        ParseReply,
        &ResolvedParseProfile<'_>,
        &SchemaRegistry,
        &mut FoundationCodec<'_>,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let (package, registry) = language;
    with_languages(
        input,
        final_input,
        stopped,
        Languages {
            packages: vec![(identity.0, package)],
            registry,
        },
        identity,
        inspect,
    )
}

pub(crate) fn with_languages<T>(
    input: &str,
    final_input: bool,
    stopped: bool,
    languages: Languages<'_>,
    identity: (&str, &str),
    inspect: impl FnOnce(
        ParseReply,
        &ResolvedParseProfile<'_>,
        &SchemaRegistry,
        &mut FoundationCodec<'_>,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let Languages {
        packages: languages,
        registry,
    } = languages;
    let (alias, source_name) = identity;
    let mut setup = budget();
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?
        .clone();
    let profile = ParseProfile {
        id: format!("external-{source_name}/1"),
        languages: languages
            .iter()
            .map(|(alias, package)| {
                Ok(LanguageRegistration {
                    alias: (*alias).into(),
                    package: package
                        .check(&registry, &mut setup)
                        .map_err(error)?
                        .semantic_identity(&mut setup)
                        .map_err(error)?,
                    default_category: package.root.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
        schemas: languages
            .iter()
            .map(|(_, package)| package.schema.clone())
            .chain([
                foundation.clone(),
                registry
                    .selected("nepl3.reader", 1)
                    .ok_or("reader")?
                    .clone(),
            ])
            .collect(),
        head_providers: vec![],
        category_modes: vec![],
        providers: vec![],
        allowlist: vec![],
        resources: vec![],
        limits: budget().limits(),
    };
    let packages = languages.iter().map(|(_, p)| p).collect::<Vec<_>>();
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &[],
                resources: &[],
            },
            &registry,
            &mut setup,
        )
        .map_err(error)?;
    let source = SourceSnapshot::new(
        SourceId(format!("{source_name}-input")),
        7,
        format!("memory:{source_name}"),
        input.as_bytes().to_vec(),
        &mut setup,
    )
    .map_err(error)?;
    let mut sources = SourceStore::default();
    sources.insert(source.clone()).map_err(error)?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest =
        environment_digest(&environment, &foundation, &registry, &mut setup).map_err(error)?;
    let raw = languages
        .iter()
        .map(|(_, package)| {
            let mode = package
                .categories
                .iter()
                .find(|c| c.name == package.root)
                .ok_or("missing root category")?
                .mode
                .clone();
            Ok(ReaderContext {
                schema: package.schema.clone(),
                category: package.root.clone(),
                mode,
                origins: vec![],
                environment: EnvironmentEntry {
                    id: 0,
                    digest,
                    value: environment.clone(),
                },
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(error)?;
    let checked = raw
        .iter()
        .map(|raw| {
            raw.check(&mut codec, &sources, &registry, &mut setup)
                .map_err(error)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let inputs = languages
        .iter()
        .zip(&checked)
        .map(|((alias, _), context)| EnvironmentInput { alias, context })
        .collect::<Vec<_>>();
    let environments =
        ParseEnvironmentSet::prepare(&resolved, &inputs, &sources, &mut codec, &mut setup)
            .map_err(error)?;
    let entry = resolved.entry(alias, None, &mut setup).map_err(error)?;
    let mut session = ParseSession::new(
        "external-parse".into(),
        &resolved,
        &environments,
        &mut setup,
    )
    .map_err(error)?;
    let mut operation = budget();
    if stopped {
        operation.cancel();
    }
    let reply = session
        .read(
            ParseRequest {
                snapshot: &source,
                start: 0,
                limit: input.len() as u64,
                final_input,
                entry: &entry,
                states: &languages
                    .iter()
                    .map(|(alias, _)| LanguageReaderState {
                        alias: (*alias).into(),
                        state: NdfValue::Unit,
                    })
                    .collect::<Vec<_>>(),
            },
            &sources,
            &mut operation,
            &mut SourceAdmission::default(),
        )
        .map_err(error)?;
    inspect(reply, &resolved, &registry, &mut codec)
}
