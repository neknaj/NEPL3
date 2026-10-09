//! Explicit trusted-host standalone export. This is not portable renderer
//! identity, browser qualification, or an OS process-tree isolation boundary.
use super::*;
use crate::doc::math::{self as math_core, MathDisplayHost, document, occurrence};
use math_core::display::{
    Preference, TexPreparation,
    generation::{
        Attempt, Setup,
        composite::{CompletionPolicy, FallbackReason, Representation},
    },
    process::{
        self,
        supervisor::{RunControl, Supervisor},
    },
    request,
};
use nepl3_core::budget::Resource;
use nepl3_doc_html::guests;
mod host;
mod publish;

/// Native mode currently preserves complete external CSS/font resource paths.
/// MathMLOnly uses the independent existing exporter without reading host.json.
/// All host paths must be absolute; modules_url is an explicit local directory
/// URL. Selected host code is trusted and can itself perform arbitrary I/O.
pub fn write(
    input: &Path,
    output: &Path,
    host_path: &Path,
    css: CssMode,
    renderer: math::Renderer,
) -> crate::Result<()> {
    if renderer == math::Renderer::MathmlOnly {
        return super::write_with_options(input, output, css, renderer);
    }
    if css != CssMode::External {
        return Err("NativeMathRequiresExternalCss".into());
    }
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let source = super::read_source(input)?;
    let (host, host_digest) = host::Host::read(host_path)?;
    let compiled = compiled()?;
    let mut supervisor = Supervisor::default();
    let result = with_input_route(
        true,
        &compiled,
        &source,
        "Article",
        |tree, profile, _, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let mut b = budget();
            let doc = lower::document(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut b,
                &mut codec,
            )
            .map_err(err)?;
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            let prepared = guests::prepare(&doc, &options, profile.registry(), &mut codec, &mut b)
                .map_err(err)?;
            let assets = math_core::assets::loader::load_installation(
                &host.katex_installation,
                16_000_000,
                &mut b,
            )
            .map_err(err)?;
            let closed = document::render(
                &prepared,
                1000,
                &mut |context, b| {
                    // Decimal ordinal has at most 20 digits. Scope bytes are copied
                    // by generation under the same ledger after this host formatting.
                    b.charge(Resource::AllocationUnits, 30).map_err(err)?;
                    b.charge(Resource::Work, 30).map_err(err)?;
                    let scope = format!("nepl-math-{}", context.ordinal());
                    let mut display = MathDisplayHost {
                        registry: profile.registry(),
                        math_surface: &compiled.others[0].schema,
                        sentence_surface: Some(&compiled.others[3].schema),
                        doc_surface: Some(&compiled.doc.package.schema),
                        codec: &mut codec,
                    };
                    let generated = occurrence::generate(
                        context,
                        &mut display,
                        Preference::KaTeXPreferred,
                        Setup {
                            controls: host::controls(),
                            request_cap: 65536,
                            config: host.config(),
                            assets: &assets,
                            scope: &scope,
                        },
                        |request, config, b| {
                            supervisor.run(request, config, b, &mut || {
                                std::thread::sleep(Duration::from_millis(2));
                                RunControl::Continue
                            })
                        },
                        b,
                    )
                    .map_err(err)?;
                    match generated
                        .into_composite_with_policy(CompletionPolicy::OrdinaryMathmlFallback, b)
                        .map_err(err)?
                    {
                        occurrence::Composition::Ready(composed) => Ok(composed),
                        occurrence::Composition::Deferred(generated) => {
                            Err(deferred(generated.generation().attempt()))
                        }
                    }
                },
                &mut |context, b| super::code::render(context.embed(), tree, profile, b),
                &mut b,
            )
            .map_err(err)?;
            if supervisor.has_pending_cleanup() {
                return Err("NativeCleanupStillPending".into());
            }
            closed.check_math_termination(&mut b).map_err(err)?;
            let bundle = closed.materialize_bundle(&mut b).map_err(err)?;
            publish::write(output, &bundle, &source, host_digest, profile.digest(), &b)
        },
    );
    // A failed generation must not drop the native resources at the callback
    // boundary. This may wait on inherited pipes; no hard deadline is promised.
    let mut cleanup_error = None;
    while supervisor.has_pending_cleanup() {
        if let Some(reclaimed) = supervisor.poll_cleanup()
            && let process::CleanupPoll::Pending {
                termination_error: Some(error),
                ..
            } = reclaimed.status
        {
            cleanup_error.get_or_insert(error);
        }
        if supervisor.has_pending_cleanup() {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    match (result, cleanup_error) {
        (Err(error), Some(cleanup)) => {
            Err(format!("{error}; native cleanup reported: {cleanup:?}").into())
        }
        (result, _) => result.map_err(Into::into),
    }
}
fn deferred(attempt: &Attempt<'_, process::supervisor::RunError>) -> String {
    match attempt {
        Attempt::RequestRejected(e) => format!("NativeRequestRejected: {e:?}"),
        Attempt::DriverFailed(e) => format!("NativeDriverFailed: {e:?}"),
        Attempt::AssociationRejected(e) => format!("NativeAssociationRejected: {e:?}"),
        Attempt::ValidationFailed(e) => format!("NativeValidationFailed: {e:?}"),
        Attempt::Other(process::reply::Outcome::InvalidRequest(cause)) => format!(
            "NativeInvalidRequest: {}",
            cause.as_ref().map_or("unspecified", |c| c.code())
        ),
        Attempt::Other(process::reply::Outcome::Stopped(cause)) => {
            format!("NativeRendererStopped: {}", cause.code())
        }
        Attempt::Other(process::reply::Outcome::Violation(cause)) => {
            format!("NativeProviderViolation: {}", cause.code())
        }
        _ => "NativeMathDeferred: no completed export representation".into(),
    }
}
