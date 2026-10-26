//! Opt-in SVG CLI continuation. Only allowance files are reread at prompts.
//! Doc/file/SVG bytes and renderer/display selection stay captured in memory.
use crate::doc::{
    export::{
        MathRenderer,
        pages::{
            self,
            interactive::{RetryLimits, next_limits},
            svg::preparation::{
                Preparation, Progress,
                rendering::{RenderProgress, Rendered},
            },
        },
    },
    source::{Compiled, err},
};
use nepl3_core::budget::{StopReason, Usage};
use std::{
    io::{BufRead, Write},
    path::Path,
};

pub fn write(
    manifest: &Path,
    limits: &Path,
    output: &Path,
    renderer: MathRenderer,
) -> crate::Result<()> {
    let mut input = std::io::stdin().lock();
    let mut messages = std::io::stderr().lock();
    write_with_io(
        manifest,
        limits,
        output,
        renderer,
        &mut input,
        &mut messages,
    )
}
fn write_with_io(
    manifest: &Path,
    limits: &Path,
    output: &Path,
    renderer: MathRenderer,
    input: &mut impl BufRead,
    messages: &mut impl Write,
) -> crate::Result<()> {
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let selected = RetryLimits::read(limits)?;
    let compiled = crate::doc::source::compiled()?;
    let done = run(
        &compiled,
        manifest,
        selected,
        renderer,
        |previous, stage, reason, usage| {
            writeln!(
                messages,
                "Paused {stage}: {reason:?}; cumulative usage={usage:?}"
            )
            .map_err(err)?;
            next_limits(previous, input, messages)
        },
    )?;
    // As in ordinary export, files precede the completion receipt. Filesystem
    // failure after Complete may leave an incomplete directory; no rollback claim.
    pages::write_checked_files(done.files, done.manifest, output)
}
fn run(
    compiled: &Compiled,
    manifest: &Path,
    mut limits: RetryLimits,
    renderer: MathRenderer,
    mut choose: impl FnMut(RetryLimits, &str, StopReason, Usage) -> Result<Option<RetryLimits>, String>,
) -> Result<Rendered, String> {
    let mut preparing = Preparation::capture_selected(
        compiled,
        manifest,
        renderer,
        limits.job_limits.budget().limits(),
        limits.preparation(),
        limits.output_limits,
    )?;
    let prepared = loop {
        match preparing.run() {
            Progress::Prepared(ready) => break ready,
            Progress::Paused { mut job, .. } => loop {
                let Some(next) =
                    choose(limits, "preparation", job.reason(), job.usage()).map_err(|error| {
                        format!(
                            "{error}; usage={:?}; attempts={:?}",
                            job.usage(),
                            job.attempts()
                        )
                    })?
                else {
                    let cancelled = job.cancel();
                    return Err(format!(
                        "Cancelled: {:?}; attempts={:?}",
                        cancelled.accounting, cancelled.attempts
                    ));
                };
                if !next.covers(limits) {
                    continue;
                }
                match job.resume(next.job_limits.budget().limits(), next.preparation()) {
                    Ok(next_job) => {
                        limits = next;
                        preparing = next_job;
                        break;
                    }
                    Err(rejected) => job = rejected.into_paused(),
                }
            },
            Progress::Failed {
                error,
                usage,
                attempts,
            } => return Err(format!("{error}; usage={usage:?}; attempts={attempts:?}")),
            Progress::Cancelled(cancelled) => {
                return Err(format!(
                    "Cancelled: {:?}; attempts={:?}",
                    cancelled.accounting, cancelled.attempts
                ));
            }
        }
    };
    let mut rendering = prepared
        .into_render_selected(limits.output_limits)
        .map_err(err)?;
    loop {
        match rendering.run() {
            RenderProgress::Complete(done) => return Ok(done),
            RenderProgress::Paused { mut job, .. } => loop {
                let Some(next) =
                    choose(limits, "render", job.reason(), job.usage()).map_err(|error| {
                        format!(
                            "{error}; usage={:?}; preparation={:?}; renders={:?}",
                            job.usage(),
                            job.preparation(),
                            job.attempts()
                        )
                    })?
                else {
                    let cancelled = job.cancel();
                    return Err(format!(
                        "Cancelled: {:?}; preparation={:?}; renders={:?}",
                        cancelled.accounting, cancelled.preparation, cancelled.renders
                    ));
                };
                if !next.covers(limits) {
                    continue;
                }
                match job.resume(next.job_limits.budget().limits(), next.output_limits) {
                    Ok(next_job) => {
                        limits = next;
                        rendering = next_job;
                        break;
                    }
                    Err(rejected) => job = rejected.into_paused(),
                }
            },
            RenderProgress::Failed(failure) => return Err(err(failure)),
            RenderProgress::Cancelled(cancelled) => {
                return Err(format!(
                    "Cancelled: {:?}; preparation={:?}; renders={:?}",
                    cancelled.accounting, cancelled.preparation, cancelled.renders
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests;
