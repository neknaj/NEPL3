//! Native atomic SVG rendering over owned prepared sources. Retries repeat
//! rendering, never parsing or rebuilding completed Code evidence. Publication,
//! interactive CLI prompts and durable restoration are not provided here.
mod receipt;
use super::{Frozen, Prepared};
use crate::doc::export::{
    MathRenderer,
    pages::{
        resources::{
            self, OperationLimits,
            job::{self, Attempt, Running},
        },
        svg::{generation, input::OwnedBinding},
    },
};
pub use job::preparation::rendering::RenderAttempt;
use job::preparation::{PageAttempt, owned::Owned};
use nepl3_core::budget::{Limits, StopReason, Usage};
use std::collections::BTreeMap;

struct Data<'c> {
    owned: Owned<'c>,
    assets: Vec<OwnedBinding>,
    renderer: MathRenderer,
    output: OperationLimits,
    renders: Vec<RenderAttempt>,
}
pub struct RenderJob<'c> {
    data: Box<Data<'c>>,
    account: Running,
}
pub struct PausedRender<'c> {
    data: Box<Data<'c>>,
    account: job::Paused,
}
pub struct RejectedRender<'c> {
    paused: PausedRender<'c>,
}
pub struct Rendered {
    pub files: BTreeMap<String, Vec<u8>>,
    pub manifest: String,
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
    pub accounting: job::Cancelled,
    pub preparation: Vec<PageAttempt>,
    pub renders: Vec<RenderAttempt>,
}
pub enum RenderProgress<'c> {
    Complete(Rendered),
    Paused {
        job: PausedRender<'c>,
        error: Option<String>,
    },
    Failed(RenderFailure),
    Cancelled(CancelledRender),
}
impl<'c> Prepared<'c> {
    /// Consume the exact source/Code checkpoint together with every frozen SVG
    /// binding. The manifest output cap is the initial render-attempt allowance.
    pub fn into_render(self) -> Result<RenderJob<'c>, Box<RenderFailure>> {
        let Self { inner, frozen } = self;
        let Frozen {
            assets,
            options,
            renderer,
            output,
        } = frozen;
        let (owned, account) = inner.into_owned(options).map_err(|failure| {
            Box::new(RenderFailure {
                error: failure.error,
                usage: failure.usage,
                preparation: failure.preparation,
                renders: Vec::new(),
            })
        })?;
        if !owned.aliases.is_empty() {
            return Err(Box::new(RenderFailure {
                error: "SvgAliasPreparationUnsupported".into(),
                usage: account.usage(),
                preparation: owned.preparation,
                renders: Vec::new(),
            }));
        }
        Ok(RenderJob {
            data: Box::new(Data {
                owned,
                assets,
                renderer,
                output,
                renders: Vec::new(),
            }),
            account,
        })
    }
}
impl<'c> RenderJob<'c> {
    pub fn run(self) -> RenderProgress<'c> {
        let Self { mut data, account } = self;
        if data.renders.try_reserve(1).is_err() {
            return RenderProgress::Failed(RenderFailure {
                error: "HostAllocationFailure".into(),
                usage: account.usage(),
                preparation: data.owned.preparation,
                renders: data.renders,
            });
        }
        let requested = data.output.budget().limits();
        let result = account.run_budget(requested, |b| {
            let effective = b.limits();
            let result = generation::render_prepared(generation::Context {
                compiled: data.owned.compiled, request: &data.owned.request,
                page_code: &data.owned.code, origins: &data.owned.origins,
                file_origins: &data.owned.file_origins, assets: &data.assets,
                preference: data.renderer, initial: b.usage(),
                scope: "Frozen source and page-bound Code checkpoints from cumulative parse/lower/Code preparation; output initial_usage starts this render attempt, not source preparation. Original package receipt serialization is measured. Capture I/O, host containers, outer job receipt bookkeeping and KaTeX internals are not measured. No durable restoration.",
            }, b);
            data.renders.push(RenderAttempt {
                requested, effective, usage: b.usage(), stop: b.poll().err(), failed: result.is_err(),
            });
            result
        });
        match result {
            Attempt::Complete { value, running } => {
                let limits = running.limits();
                let usage = running.usage();
                match receipt::finish(
                    &value.manifest,
                    &data.owned.evidence,
                    &data.renders,
                    limits,
                    usage,
                ) {
                    Ok(manifest) => RenderProgress::Complete(Rendered {
                        files: value.files,
                        manifest,
                        limits,
                        usage,
                        preparation: data.owned.preparation,
                        renders: data.renders,
                    }),
                    Err(error) => RenderProgress::Failed(RenderFailure {
                        error,
                        usage,
                        preparation: data.owned.preparation,
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
                preparation: data.owned.preparation,
                renders: data.renders,
            }),
            Attempt::Cancelled(accounting) => RenderProgress::Cancelled(CancelledRender {
                accounting,
                preparation: data.owned.preparation,
                renders: data.renders,
            }),
        }
    }
    pub fn cancel(self) -> CancelledRender {
        CancelledRender {
            accounting: self.account.cancel(),
            preparation: self.data.owned.preparation,
            renders: self.data.renders,
        }
    }
}
impl<'c> PausedRender<'c> {
    pub fn usage(&self) -> Usage {
        self.account.usage()
    }
    pub fn limits(&self) -> Limits {
        self.account.limits()
    }
    pub fn reason(&self) -> StopReason {
        self.account.reason()
    }
    pub fn preparation(&self) -> &[PageAttempt] {
        &self.data.owned.preparation
    }
    pub fn attempts(&self) -> &[RenderAttempt] {
        &self.data.renders
    }
    pub fn resume(
        self,
        limits: Limits,
        output: OperationLimits,
    ) -> Result<RenderJob<'c>, Box<RejectedRender<'c>>> {
        if !job::covers(output.budget().limits(), self.data.output.budget().limits())
            || !job::covers(limits, self.account.limits())
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
            preparation: self.data.owned.preparation,
            renders: self.data.renders,
        }
    }
}
impl<'c> RejectedRender<'c> {
    pub fn into_paused(self) -> PausedRender<'c> {
        self.paused
    }
}

#[cfg(test)]
mod tests;
