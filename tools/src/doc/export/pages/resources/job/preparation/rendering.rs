//! Atomic in-memory render attempts over retained prepared documents.
//! A retry repeats rendering, never parsing completed pages. This entry supports
//! document pages, frozen passive files and explicit native compatibility aliases.
use super::*;
use crate::doc::export::{
    MathRenderer,
    pages::{self, render},
};
use nepl3_doc_html::pages::PagesHtmlRequest;

struct RenderData<'a> {
    compiled: &'a Compiled,
    request: PagesHtmlRequest,
    code: Vec<PreparedPageCode>,
    origins: Vec<serde_json::Value>,
    file_origins: Vec<serde_json::Value>,
    aliases: Vec<pages::aliases::PageAliases>,
    evidence: serde_json::Value,
    preparation: Vec<PageAttempt>,
    renders: Vec<RenderAttempt>,
    renderer: MathRenderer,
    output: OperationLimits,
}
#[derive(Debug)]
pub struct RenderAttempt {
    pub requested: Limits,
    pub effective: Limits,
    pub usage: Usage,
    pub stop: Option<StopReason>,
    pub failed: bool,
}
pub struct RenderJob<'a> {
    data: RenderData<'a>,
    account: Running,
}
pub struct PausedRender<'a> {
    data: RenderData<'a>,
    account: Paused,
}
pub struct RejectedRender<'a> {
    paused: PausedRender<'a>,
}
pub struct RenderedJob {
    pub generated: pages::GeneratedPages,
    pub limits: Limits,
    pub usage: Usage,
    pub preparation: Vec<PageAttempt>,
    pub renders: Vec<RenderAttempt>,
}
#[derive(Debug)]
pub struct RenderFailure {
    pub error: String,
    pub usage: Usage,
    pub preparation: Vec<PageAttempt>,
    pub renders: Vec<RenderAttempt>,
}
pub struct CancelledRender {
    pub accounting: Cancelled,
    pub preparation: Vec<PageAttempt>,
    pub renders: Vec<RenderAttempt>,
}
pub enum RenderProgress<'a> {
    Complete(RenderedJob),
    Paused {
        job: PausedRender<'a>,
        error: Option<String>,
    },
    Failed(RenderFailure),
    Cancelled(CancelledRender),
}
impl<'a> PreparedPages<'a> {
    /// Freeze the renderer and own all retained document/Code evidence. Host
    /// bookkeeping remains outside the logical operation resource ledger.
    pub fn into_render(
        self,
        output: OperationLimits,
        renderer: MathRenderer,
    ) -> Result<RenderJob<'a>, Box<RenderFailure>> {
        self.into_render_with_display(
            output,
            pages::DisplayOptions {
                math_renderer: renderer,
                ..Default::default()
            },
        )
    }
    /// Freeze display selection together with the renderer for every retry.
    /// Resuming a paused render can change allowance, never the owned request.
    pub fn into_render_with_display(
        self,
        output: OperationLimits,
        options: pages::DisplayOptions,
    ) -> Result<RenderJob<'a>, Box<RenderFailure>> {
        let Self { mut data, account } = self;
        match build(&mut data, account.usage(), output, options) {
            Ok(data) => Ok(RenderJob { data, account }),
            Err(error) => Err(Box::new(RenderFailure {
                error,
                usage: account.usage(),
                preparation: data.attempts,
                renders: Vec::new(),
            })),
        }
    }
}
fn build<'a>(
    data: &mut Data<'a>,
    usage: Usage,
    output: OperationLimits,
    options: pages::DisplayOptions,
) -> Result<RenderData<'a>, String> {
    let owned = super::owned::build(data, usage, options.document)?;
    Ok(RenderData {
        compiled: owned.compiled,
        request: owned.request,
        code: owned.code,
        origins: owned.origins,
        file_origins: owned.file_origins,
        aliases: owned.aliases,
        evidence: owned.evidence,
        preparation: owned.preparation,
        renders: Vec::new(),
        renderer: options.math_renderer,
        output,
    })
}
impl<'a> RenderJob<'a> {
    pub fn run(self) -> RenderProgress<'a> {
        let Self { mut data, account } = self;
        if data.renders.try_reserve(1).is_err() {
            return RenderProgress::Failed(RenderFailure {
                error: "HostAllocationFailure".into(),
                usage: account.usage(),
                preparation: data.preparation,
                renders: data.renders,
            });
        }
        let requested = data.output.budget().limits();
        let result = account.run(requested, |meter| {
            let effective = meter.limits();
            let result =
                pages::aliases::check_set_binding(&data.request.set, &data.aliases, meter.budget)
                    .and_then(|()| {
                        render::render(
                            render::Context {
                                compiled: data.compiled,
                                request: &data.request,
                                page_code: &data.code,
                                profiles: &[],
                                retained_preparation: Some(&data.evidence),
                                origins: &data.origins,
                                file_origins: &data.file_origins,
                                // Only the retained evidence identity is used for this route.
                                phases: pages::resources::PhaseLimits::default(),
                                initial_usage: meter.usage(),
                                renderer: data.renderer,
                                aliases: &data.aliases,
                            },
                            meter.budget,
                        )
                    });
            data.renders.push(RenderAttempt {
                requested,
                effective,
                usage: meter.usage(),
                stop: meter.poll().err(),
                failed: result.is_err(),
            });
            result
        });
        match result {
            Attempt::Complete { value, running } => {
                let usage = running.usage();
                let limits = running.limits();
                let generated = value.finish(data.request, &data.aliases);
                match generated {
                    Ok(mut generated) => {
                        let alias_job = !data.aliases.is_empty();
                        let contract = if alias_job {
                            "nepl3.local-doc-pages.retry-alias-job/1"
                        } else {
                            "nepl3.local-doc-pages.retry-job/1"
                        };
                        let receipt = serde_json::json!({
                            "contract":contract,
                            "limits":pages::resources::limits(limits),
                            "usage":pages::resources::usage(usage),
                            "render_attempts":data.renders.iter().map(|a|serde_json::json!({
                                "requested":pages::resources::limits(a.requested),"effective":pages::resources::limits(a.effective),
                                "usage":pages::resources::usage(a.usage),"stop":a.stop.map(|s|format!("{s:?}")),"failed":a.failed
                            })).collect::<Vec<_>>(),
                            "budget_scope":"Cumulative measured preparation and render attempts; host bookkeeping, receipts, KaTeX internals and I/O are not measured.",
                        });
                        let encoded = (|| -> Result<String, String> {
                            let mut manifest: serde_json::Value =
                                serde_json::from_str(&generated.manifest).map_err(err)?;
                            let mut bytes = Vec::from(contract.as_bytes());
                            bytes.push(0);
                            bytes.extend_from_slice(
                                &serde_json::to_vec(&manifest["execution_identity"])
                                    .map_err(err)?,
                            );
                            bytes.extend_from_slice(
                                &serde_json::to_vec(&manifest["phase_execution"]).map_err(err)?,
                            );
                            if alias_job {
                                let compatibility = manifest["compatibility"]["execution_identity"]
                                    .as_str()
                                    .ok_or("MissingCompatibilityExecutionIdentity")?;
                                bytes.extend_from_slice(
                                    &serde_json::to_vec(compatibility).map_err(err)?,
                                );
                            }
                            bytes.extend_from_slice(&serde_json::to_vec(&receipt).map_err(err)?);
                            manifest["job_execution"] = receipt;
                            manifest["job_execution"]["identity"] =
                                pages::digest_hex(Digest::of(&bytes)).into();
                            Ok(serde_json::to_string_pretty(&manifest).map_err(err)? + "\n")
                        })();
                        match encoded {
                            Ok(manifest) => {
                                generated.manifest = manifest;
                                RenderProgress::Complete(RenderedJob {
                                    generated,
                                    limits,
                                    usage,
                                    preparation: data.preparation,
                                    renders: data.renders,
                                })
                            }
                            Err(error) => RenderProgress::Failed(RenderFailure {
                                error,
                                usage,
                                preparation: data.preparation,
                                renders: data.renders,
                            }),
                        }
                    }
                    Err(error) => RenderProgress::Failed(RenderFailure {
                        error,
                        usage,
                        preparation: data.preparation,
                        renders: data.renders,
                    }),
                }
            }
            Attempt::Paused { paused, error } => RenderProgress::Paused {
                job: PausedRender {
                    data,
                    account: paused,
                },
                error,
            },
            Attempt::Failed { error, usage, .. } => RenderProgress::Failed(RenderFailure {
                error,
                usage,
                preparation: data.preparation,
                renders: data.renders,
            }),
            Attempt::Cancelled(accounting) => RenderProgress::Cancelled(CancelledRender {
                accounting,
                preparation: data.preparation,
                renders: data.renders,
            }),
        }
    }
    pub fn cancel(self) -> CancelledRender {
        CancelledRender {
            accounting: self.account.cancel(),
            preparation: self.data.preparation,
            renders: self.data.renders,
        }
    }
}
impl<'a> PausedRender<'a> {
    pub fn usage(&self) -> Usage {
        self.account.usage()
    }
    pub fn reason(&self) -> StopReason {
        self.account.reason()
    }
    pub fn preparation(&self) -> &[PageAttempt] {
        &self.data.preparation
    }
    pub fn attempts(&self) -> &[RenderAttempt] {
        &self.data.renders
    }
    pub fn resume(
        self,
        limits: Limits,
        output: OperationLimits,
    ) -> Result<RenderJob<'a>, Box<RejectedRender<'a>>> {
        if !covers(limits, self.account.limits())
            || !covers(output.budget().limits(), self.data.output.budget().limits())
        {
            return Err(Box::new(RejectedRender { paused: self }));
        }
        let Self { mut data, account } = self;
        match account.resume(limits) {
            Ok(account) => {
                data.output = output;
                Ok(RenderJob { data, account })
            }
            Err(rejected) => Err(Box::new(RejectedRender {
                paused: Self {
                    data,
                    account: rejected.into_paused(),
                },
            })),
        }
    }
    pub fn cancel(self) -> CancelledRender {
        CancelledRender {
            accounting: self.account.cancel(),
            preparation: self.data.preparation,
            renders: self.data.renders,
        }
    }
}
impl<'a> RejectedRender<'a> {
    pub fn into_paused(self) -> PausedRender<'a> {
        self.paused
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod aliases;
