//! Native in-memory SVG source preparation. This owns captured inputs, not
//! admitted SVG, a validated PageSet, rendered files or a durable continuation.
//! Capture I/O and retained host containers are outside measured phase usage.
use super::input::{self, OwnedBinding};
use crate::doc::{
    export::{
        MathRenderer,
        pages::resources::{OperationLimits, job::preparation as base},
    },
    source::Compiled,
};
use nepl3_core::budget::{Limits, StopReason, Usage};
use nepl3_doc_core::model::DocumentSyntax;
use nepl3_doc_html::RenderOptions;
use std::path::Path;

/// Frozen selection and raw assets. These are not SVG validation evidence.
pub struct Frozen {
    assets: Vec<OwnedBinding>,
    options: RenderOptions,
    renderer: MathRenderer,
    output: OperationLimits,
}
/// An ordered page binding; an empty binding remains observable.
pub struct Binding<'a> {
    value: &'a OwnedBinding,
}
pub struct Asset<'a> {
    pub id: &'a str,
    pub source: &'a str,
    pub svg: &'a str,
}
impl Frozen {
    pub fn options(&self) -> &RenderOptions {
        &self.options
    }
    pub fn renderer(&self) -> MathRenderer {
        self.renderer
    }
    pub fn output_limits(&self) -> OperationLimits {
        self.output
    }
    pub fn bindings(&self) -> impl Iterator<Item = Binding<'_>> {
        self.assets.iter().map(|value| Binding { value })
    }
}
impl Binding<'_> {
    pub fn page_index(&self) -> usize {
        self.value.page
    }
    pub fn assets(&self) -> impl Iterator<Item = Asset<'_>> {
        self.value.assets.iter().map(|v| Asset {
            id: &v.id,
            source: &v.source,
            svg: &v.svg,
        })
    }
}
/// Linear preparation authority. Resumption cannot replace captured bytes.
pub struct Preparation<'c> {
    inner: base::Preparation<'c>,
    frozen: Frozen,
}
pub struct Paused<'c> {
    inner: base::PausedPreparation<'c>,
    frozen: Frozen,
}
pub struct Rejected<'c> {
    paused: Paused<'c>,
}
/// Exact prepared document/Code pairs are retained privately and inseparably.
/// No ordinary render escape is exposed that could discard the SVG bindings.
pub struct Prepared<'c> {
    inner: base::PreparedPages<'c>,
    frozen: Frozen,
}
pub enum Progress<'c> {
    Prepared(Prepared<'c>),
    Paused {
        job: Paused<'c>,
        error: Option<String>,
    },
    Failed {
        error: String,
        usage: Usage,
        attempts: Vec<base::PageAttempt>,
    },
    Cancelled(base::CancelledPreparation),
}
fn operation(v: Limits) -> OperationLimits {
    OperationLimits {
        source_bytes: v.source_bytes,
        work: v.work,
        depth: v.depth,
        nodes: v.nodes,
        allocation_units: v.allocation_units,
        output_bytes: v.output_bytes,
        diagnostics: v.diagnostics,
        events: v.events,
    }
}
impl<'c> Preparation<'c> {
    /// Capture once under the same finite setup limits as the SVG CLI. Parse
    /// and lower ceilings come from the manifest; Code has an explicit ceiling.
    /// Unlike ordinary export, all three stages consume one cumulative job.
    /// The manifest output limit is retained but is not spent by preparation.
    pub fn capture(
        compiled: &'c Compiled,
        manifest: &Path,
        renderer: MathRenderer,
        limits: Limits,
        code_limits: OperationLimits,
    ) -> Result<Self, String> {
        let captured = input::capture(manifest)?;
        let inner = base::Preparation::with_files(
            compiled,
            captured.inputs,
            captured.files,
            limits,
            base::PreparationLimits {
                parse_limits: operation(captured.phases.parse),
                lower_limits: operation(captured.phases.lower),
                code_limits,
            },
        )?;
        Ok(Self {
            inner,
            frozen: Frozen {
                assets: captured.assets,
                options: captured.options,
                renderer,
                output: captured.output,
            },
        })
    }
    pub fn frozen(&self) -> &Frozen {
        &self.frozen
    }
    pub fn run(self) -> Progress<'c> {
        let Self { inner, frozen } = self;
        match inner.run() {
            base::Progress::Prepared(inner) => Progress::Prepared(Prepared { inner, frozen }),
            base::Progress::Paused { job, error } => Progress::Paused {
                job: Paused { inner: job, frozen },
                error,
            },
            base::Progress::Failed {
                error,
                usage,
                attempts,
            } => Progress::Failed {
                error,
                usage,
                attempts,
            },
            base::Progress::Cancelled(value) => Progress::Cancelled(value),
        }
    }
    pub fn cancel(self) -> base::CancelledPreparation {
        self.inner.cancel()
    }
}
impl<'c> Paused<'c> {
    pub fn frozen(&self) -> &Frozen {
        &self.frozen
    }
    pub fn prepared_count(&self) -> usize {
        self.inner.prepared_count()
    }
    pub fn usage(&self) -> Usage {
        self.inner.usage()
    }
    pub fn reason(&self) -> StopReason {
        self.inner.reason()
    }
    pub fn attempts(&self) -> &[base::PageAttempt] {
        self.inner.attempts()
    }
    pub fn resume(
        self,
        limits: Limits,
        phases: base::PreparationLimits,
    ) -> Result<Preparation<'c>, Box<Rejected<'c>>> {
        let Self { inner, frozen } = self;
        match inner.resume(limits, phases) {
            Ok(inner) => Ok(Preparation { inner, frozen }),
            Err(rejected) => Err(Box::new(Rejected {
                paused: Self {
                    inner: rejected.into_paused(),
                    frozen,
                },
            })),
        }
    }
    pub fn cancel(self) -> base::CancelledPreparation {
        self.inner.cancel()
    }
}
impl<'c> Rejected<'c> {
    pub fn into_paused(self) -> Paused<'c> {
        self.paused
    }
}
impl Prepared<'_> {
    pub fn frozen(&self) -> &Frozen {
        &self.frozen
    }
    pub fn prepared_count(&self) -> usize {
        self.inner.prepared_count()
    }
    pub fn usage(&self) -> Usage {
        self.inner.usage()
    }
    pub fn attempts(&self) -> &[base::PageAttempt] {
        self.inner.attempts()
    }
    pub fn documents(&self) -> impl Iterator<Item = &DocumentSyntax> {
        self.inner.documents()
    }
}

#[cfg(test)]
mod tests;
