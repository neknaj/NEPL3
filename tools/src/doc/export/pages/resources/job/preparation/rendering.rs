//! Atomic in-memory render attempts over retained prepared documents.
//! A retry repeats rendering, never parsing completed pages. This entry supports
//! document pages only; no registered file or compatibility-alias input is accepted.
use super::*;
use crate::doc::export::{
    MathRenderer,
    pages::{self, render},
};
use nepl3_doc_core::pages::{PageDocument, PageRegistration, PageSet};
use nepl3_doc_html::{ParallelMode, RenderOptions, pages::PagesHtmlRequest};

struct RenderData<'a> {
    compiled: &'a Compiled,
    request: PagesHtmlRequest,
    code: Vec<PreparedPageCode>,
    origins: Vec<serde_json::Value>,
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
fn observation(value: &Observation) -> serde_json::Value {
    serde_json::json!({
        "stage":format!("{:?}", value.stage),
        "requested":pages::resources::limits(value.requested),
        "effective":pages::resources::limits(value.effective),
        "usage":pages::resources::usage(value.usage),
        "stop":value.stop.map(|s|format!("{s:?}")),
        "failed":value.failed,"accounting_rejected":value.accounting_rejected,
        "profile":value.profile.map(pages::digest_hex),
    })
}
impl<'a> PreparedPages<'a> {
    /// Freeze the renderer and own all retained document/Code evidence. Host
    /// bookkeeping remains outside the logical operation resource ledger.
    pub fn into_render(
        self,
        output: OperationLimits,
        renderer: MathRenderer,
    ) -> Result<RenderJob<'a>, Box<RenderFailure>> {
        let Self { mut data, account } = self;
        match build(&mut data, account.usage(), output, renderer) {
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
    renderer: MathRenderer,
) -> Result<RenderData<'a>, String> {
    let mut documents = Vec::new();
    let mut code = Vec::new();
    let mut origins = Vec::new();
    documents
        .try_reserve_exact(data.prepared.len())
        .map_err(|_| "HostAllocationFailure")?;
    code.try_reserve_exact(data.prepared.len())
        .map_err(|_| "HostAllocationFailure")?;
    origins
        .try_reserve_exact(data.prepared.len())
        .map_err(|_| "HostAllocationFailure")?;
    for (index, ((entry, source), page)) in std::mem::take(&mut data.inputs)
        .into_iter()
        .zip(std::mem::take(&mut data.prepared))
        .enumerate()
    {
        let successful = data
            .attempts
            .iter()
            .rev()
            .find(|a| a.page_index == index)
            .ok_or("PreparedEvidenceMissing")?;
        origins.push(serde_json::json!({
                "id":entry.id,"source":entry.source,"route":entry.route,"input":pages::input_path(&entry)?,
                "source_sha256":pages::digest_hex(Digest::of(source.as_bytes())),
                "operations":successful.stages.iter().map(observation).collect::<Vec<_>>(),
            }));
        documents.push(PageDocument {
            registration: PageRegistration {
                id: entry.id,
                source: entry.source,
                route: entry.route,
            },
            document: page.document,
        });
        code.push(page._code);
    }
    let evidence = serde_json::json!({
        "attempts":data.attempts.iter().map(|a|serde_json::json!({
            "page_index":a.page_index,"stages":a.stages.iter().map(observation).collect::<Vec<_>>()
        })).collect::<Vec<_>>(),

        "cumulative_usage":pages::resources::usage(usage),
        "scope":"Immutable owned documents and page-bound Code evidence; not whole-PageSet validity.",
    });
    Ok(RenderData {
        compiled: data.compiled,
        request: PagesHtmlRequest {
            set: PageSet {
                pages: documents,
                files: Vec::new(),
            },
            options: RenderOptions {
                parallel: ParallelMode::Rows,
            },
        },
        code,
        origins,
        evidence,
        preparation: std::mem::take(&mut data.attempts),
        renders: Vec::new(),
        renderer,
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
            let result = render::render(
                render::Context {
                    compiled: data.compiled,
                    request: &data.request,
                    page_code: &data.code,
                    profiles: &[],
                    retained_preparation: Some(&data.evidence),
                    origins: &data.origins,
                    file_origins: &[],
                    // Only the retained evidence identity is used for this route.
                    phases: pages::resources::PhaseLimits::default(),
                    initial_usage: meter.usage(),
                    renderer: data.renderer,
                    aliases: &[],
                },
                meter.budget,
            );
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
                let generated = value.finish(data.request, &[]);
                match generated {
                    Ok(mut generated) => {
                        let receipt = serde_json::json!({
                            "contract":"nepl3.local-doc-pages.retry-job/1",
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
                            let mut bytes =
                                Vec::from(b"nepl3.local-doc-pages.retry-job/1\0".as_slice());
                            bytes.extend_from_slice(
                                &serde_json::to_vec(&manifest["execution_identity"])
                                    .map_err(err)?,
                            );
                            bytes.extend_from_slice(
                                &serde_json::to_vec(&manifest["phase_execution"]).map_err(err)?,
                            );
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
