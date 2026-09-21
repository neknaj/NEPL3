use super::*;
use nepl3_core::{source::*, syntax::*, value::NdfValue};
use nepl3_engine::{parse::*, profile::*};
use nepl3_reader::model::ReaderContext;
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};

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
    let (package, registry) = language()?;
    let mut setup = budget();
    let identity = package
        .check(&registry, &mut setup)
        .map_err(error)?
        .semantic_identity(&mut setup)
        .map_err(error)?;
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?
        .clone();
    let profile = ParseProfile {
        id: "external-hello/1".into(),
        languages: vec![LanguageRegistration {
            alias: "Hello".into(),
            package: identity,
            default_category: "Greeting".into(),
        }],
        schemas: vec![
            package.schema.clone(),
            foundation.clone(),
            registry
                .selected("nepl3.reader", 1)
                .ok_or("reader")?
                .clone(),
        ],
        head_providers: vec![],
        category_modes: vec![],
        providers: vec![],
        allowlist: vec![],
        resources: vec![],
        limits: budget().limits(),
    };
    let packages = [&package];
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
        SourceId("hello-input".into()),
        7,
        "memory:hello".into(),
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
    let raw = ReaderContext {
        schema: package.schema.clone(),
        category: "Greeting".into(),
        mode: "Words".into(),
        origins: vec![],
        environment: EnvironmentEntry {
            id: 0,
            digest,
            value: environment,
        },
    };
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(error)?;
    let checked = raw
        .check(&mut codec, &sources, &registry, &mut setup)
        .map_err(error)?;
    let environments = ParseEnvironmentSet::prepare(
        &resolved,
        &[EnvironmentInput {
            alias: "Hello",
            context: &checked,
        }],
        &sources,
        &mut codec,
        &mut setup,
    )
    .map_err(error)?;
    let entry = resolved.entry("Hello", None, &mut setup).map_err(error)?;
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
                states: &[LanguageReaderState {
                    alias: "Hello".into(),
                    state: NdfValue::Unit,
                }],
            },
            &sources,
            &mut operation,
            &mut SourceAdmission::default(),
        )
        .map_err(error)?;
    inspect(reply, &resolved, &registry, &mut codec)
}
