//! Opt-in in-process CLI continuation. Only the allowance file is reread.
//! Source documents, compiled definitions, output destination, display selection and renderer stay fixed.
use super::{
    Entry, GeneratedPages,
    resources::{
        self, OperationLimits,
        job::{
            self,
            preparation::{Preparation, PreparationLimits, Progress, rendering::RenderProgress},
        },
    },
};
use crate::doc::{
    export::MathRenderer,
    source::{Compiled, err},
};
use nepl3_core::budget::{StopReason, Usage};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, Write},
    path::Path,
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetryLimits {
    #[serde(default)]
    pub job_limits: OperationLimits,
    #[serde(default)]
    pub parse_limits: OperationLimits,
    #[serde(default)]
    pub lower_limits: OperationLimits,
    #[serde(default)]
    pub code_limits: OperationLimits,
    #[serde(default)]
    pub output_limits: OperationLimits,
}
impl RetryLimits {
    pub fn read(path: &Path) -> crate::Result<Self> {
        resources::read_configuration(path)
    }
    fn preparation(self) -> PreparationLimits {
        PreparationLimits {
            parse_limits: self.parse_limits,
            lower_limits: self.lower_limits,
            code_limits: self.code_limits,
        }
    }
    fn covers(self, old: Self) -> bool {
        [
            self.job_limits,
            self.parse_limits,
            self.lower_limits,
            self.code_limits,
            self.output_limits,
        ]
        .into_iter()
        .zip([
            old.job_limits,
            old.parse_limits,
            old.lower_limits,
            old.code_limits,
            old.output_limits,
        ])
        .all(|(a, b)| job::covers(a.budget().limits(), b.budget().limits()))
    }
}
pub fn defaults() -> crate::Result<()> {
    println!("{}", serde_json::to_string_pretty(&RetryLimits::default())?);
    Ok(())
}
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
    let loaded = super::load(manifest)?;
    if !loaded.files.is_empty() {
        return Err("interactive continuation does not support registered files".into());
    }
    let compiled = crate::doc::source::compiled()?;
    let generated = run(
        &compiled,
        loaded.inputs,
        selected,
        super::DisplayOptions {
            document: loaded.display,
            math_renderer: renderer,
        },
        |previous, stage, reason, usage| {
            writeln!(
                messages,
                "Paused {stage}: {reason:?}; cumulative usage={usage:?}"
            )
            .map_err(err)?;
            next_limits(previous, input, messages)
        },
    )?;
    super::write_generated(generated, output)
}
fn next_limits(
    previous: RetryLimits,
    input: &mut impl BufRead,
    messages: &mut impl Write,
) -> Result<Option<RetryLimits>, String> {
    loop {
        writeln!(messages, "resume <limits.json> or cancel (EOF cancels):").map_err(err)?;
        messages.flush().map_err(err)?;
        let mut line = String::new();
        // Bound the line before allocating arbitrary stdin input.
        let count = std::io::Read::take(&mut *input, 4097)
            .read_line(&mut line)
            .map_err(err)?;
        if line.len() > 4096 {
            return Err("ResumeCommandLimit".into());
        }
        if count == 0 || line.trim() == "cancel" {
            return Ok(None);
        }
        let Some(path) = line
            .trim()
            .strip_prefix("resume ")
            .filter(|s| !s.trim().is_empty())
        else {
            writeln!(messages, "Expected resume <limits.json> or cancel.").map_err(err)?;
            continue;
        };
        match RetryLimits::read(Path::new(path.trim())) {
            Ok(next) if next.covers(previous) => return Ok(Some(next)),
            Ok(_) => writeln!(
                messages,
                "Rejected: all job and stage limits must be nondecreasing."
            )
            .map_err(err)?,
            Err(error) => writeln!(messages, "Cannot read limits: {error}").map_err(err)?,
        }
    }
}
fn run(
    compiled: &Compiled,
    inputs: Vec<(Entry, String)>,
    mut limits: RetryLimits,
    options: super::DisplayOptions,
    mut choose: impl FnMut(RetryLimits, &str, StopReason, Usage) -> Result<Option<RetryLimits>, String>,
) -> Result<GeneratedPages, String> {
    let mut preparing = Preparation::new(
        compiled,
        inputs,
        limits.job_limits.budget().limits(),
        limits.preparation(),
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
        .into_render_with_display(limits.output_limits, options)
        .map_err(err)?;
    loop {
        match rendering.run() {
            RenderProgress::Complete(done) => return Ok(done.generated),
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
mod tests {
    use super::*;
    #[test]
    fn command_input_is_bounded_and_eof_cancels() -> Result<(), String> {
        let mut messages = Vec::new();
        assert!(
            next_limits(
                RetryLimits::default(),
                &mut std::io::Cursor::new(Vec::<u8>::new()),
                &mut messages
            )?
            .is_none()
        );
        let input = vec![b'x'; 4097];
        assert_eq!(
            next_limits(
                RetryLimits::default(),
                &mut std::io::Cursor::new(input),
                &mut messages
            )
            .err()
            .as_deref(),
            Some("ResumeCommandLimit")
        );
        Ok(())
    }
    #[test]
    fn configuration_and_monotonic_grants_are_explicit() -> Result<(), String> {
        let defaults = RetryLimits::default();
        assert!(defaults.covers(defaults));
        let mut lower = defaults;
        lower.code_limits.events -= 1;
        assert!(!lower.covers(defaults));
        assert!(serde_json::from_str::<RetryLimits>(r#"{"unknown":1}"#).is_err());
        assert!(serde_json::from_str::<RetryLimits>(r#"{"job_limits":{"work":1}}"#).is_err());
        assert!(serde_json::from_str::<RetryLimits>(r#"{"job_limits":null}"#).is_err());
        let text = serde_json::to_string(&defaults).map_err(err)?;
        let roundtrip: RetryLimits = serde_json::from_str(&text).map_err(err)?;
        assert!(roundtrip.covers(defaults) && defaults.covers(roundtrip));
        Ok(())
    }
}

#[cfg(test)]
#[path = "interactive/selection.rs"]
mod selection_tests;
