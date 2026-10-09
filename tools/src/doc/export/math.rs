//! Explicit standalone-host Math policy. No KaTeX adapter is configured here;
//! preferred rendering therefore retains a typed capability notice and uses the
//! independent MathML backend. No error/stop is caught and disguised as fallback.
use crate::doc::{math::MathDisplayHost, source::err};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::model::{DocEmbed, EmbedKind, EmbedRef};
use nepl3_markup::{html::HtmlRequest, mathml::Display};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Renderer {
    #[default]
    KatexPreferred,
    MathmlOnly,
}
impl core::str::FromStr for Renderer {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "katex-preferred" => Ok(Self::KatexPreferred),
            "mathml-only" => Ok(Self::MathmlOnly),
            _ => Err("expected --math-renderer katex-preferred|mathml-only".into()),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fallback {
    KatexAdapterUnavailable,
}
#[derive(Debug, Serialize)]
pub struct Occurrence {
    pub guest_occurrence: u64,
    pub embed: u64,
    pub display: &'static str,
    pub representation: &'static str,
    pub fallback: Option<Fallback>,
}
/// Host export notices, not a received provider Report or remote identity proof.
#[derive(Debug, Serialize)]
pub struct Report {
    pub preference: Renderer,
    pub katex_adapter_available: bool,
    pub occurrences: Vec<Occurrence>,
}
impl Report {
    pub fn new(preference: Renderer) -> Self {
        Self {
            preference,
            katex_adapter_available: false,
            occurrences: Vec::new(),
        }
    }
    pub fn render<C: FoundationValueCodec>(
        &mut self,
        guest: &DocEmbed,
        embed: EmbedRef,
        occurrence: u64,
        host: &mut MathDisplayHost<'_, C>,
        b: &mut Budget,
    ) -> Result<HtmlRequest, String>
    where
        C::Error: core::fmt::Debug,
    {
        b.poll().map_err(err)?;
        let (display, display_name) = match guest.kind {
            EmbedKind::InlineMath => (Display::Inline, "inline"),
            EmbedKind::DisplayMath => (Display::Block, "block"),
            _ => return Err("MathKind".into()),
        };
        // Capability is known before rendering. MathmlOnly never requests TeX
        // generation; preferred mode cannot pretend an absent adapter executed.
        let fallback = (self.preference == Renderer::KatexPreferred)
            .then_some(Fallback::KatexAdapterUnavailable);
        let rendered = host
            .render(&guest.closure, display, b)
            .map_err(err)?
            .into_html(b)
            .map_err(err)?;
        if fallback.is_some() {
            b.charge(Resource::Diagnostics, 1).map_err(err)?;
        }
        self.push(
            Occurrence {
                guest_occurrence: occurrence,
                embed: embed.0,
                display: display_name,
                representation: "mathml",
                fallback,
            },
            b,
        )?;
        b.poll().map_err(err)?;
        Ok(rendered.markup)
    }
    fn push(&mut self, occurrence: Occurrence, b: &mut Budget) -> Result<(), String> {
        b.poll().map_err(err)?;
        b.charge(Resource::Work, 1).map_err(err)?;
        if self.occurrences.len() == self.occurrences.capacity() {
            let capacity = self
                .occurrences
                .capacity()
                .checked_mul(2)
                .map(|n| n.max(1))
                .ok_or_else(|| err(b.stop(StopReason::AllocationLimit)))?;
            let bytes = capacity
                .checked_mul(core::mem::size_of::<Occurrence>())
                .filter(|n| *n <= isize::MAX as usize)
                .ok_or_else(|| err(b.stop(StopReason::AllocationLimit)))?;
            b.charge(Resource::Work, self.occurrences.len() as u64)
                .map_err(err)?;
            b.charge(Resource::AllocationUnits, bytes as u64)
                .map_err(err)?;
            self.occurrences
                .try_reserve_exact(capacity - self.occurrences.len())
                .map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
        }
        self.occurrences.push(occurrence);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::source::{budget, compiled, with_input_route};
    use nepl3_core::source::{SourceAdmission, SourceStore};
    use nepl3_doc_core::{check::Category, lower};
    use nepl3_wire::foundation::FoundationCodec;

    fn occurrence(index: u64) -> Occurrence {
        Occurrence {
            guest_occurrence: index,
            embed: index,
            display: "inline",
            representation: "mathml",
            fallback: None,
        }
    }
    #[test]
    fn math_export_occurrence_growth_is_amortized_metered_and_failure_retains_prior_records()
    -> Result<(), String> {
        let mut report = Report::new(Renderer::MathmlOnly);
        let mut b = budget();
        for index in 0..17 {
            report.push(occurrence(index), &mut b)?;
        }
        assert_eq!(report.occurrences.len(), 17);
        // Capacities 1,2,4,8,16,32: requested storage63 records; copied31,
        // plus17 insertions. These are logical charges, not measured allocator RSS.
        assert_eq!(
            b.usage().allocation_units,
            63 * core::mem::size_of::<Occurrence>() as u64
        );
        assert_eq!(b.usage().work, 48);
        let mut limits = budget().limits();
        limits.allocation_units = 31 * core::mem::size_of::<Occurrence>() as u64;
        let mut b = Budget::new(limits);
        let mut report = Report::new(Renderer::MathmlOnly);
        for index in 0..16 {
            report.push(occurrence(index), &mut b)?;
        }
        assert!(report.push(occurrence(16), &mut b).is_err());
        assert_eq!(report.occurrences.len(), 16);
        assert_eq!(b.poll(), Err(StopReason::AllocationLimit));
        let used = b.usage();
        assert!(report.push(occurrence(16), &mut b).is_err());
        assert_eq!(b.usage(), used);
        Ok(())
    }
    #[test]
    fn math_export_notice_rendering_keeps_stops_and_explicit_mathml_avoids_notices()
    -> Result<(), String> {
        let compiled = compiled()?;
        with_input_route(
            true,
            &compiled,
            "article en \"Math\" body cons display Math frac 1 0 nil",
            "Article",
            |tree, profile, b, _| {
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(err)?;
                let doc = lower::document(
                    tree.syntax(),
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let guest = doc.value.embeds.first().ok_or("math embed")?;
                for reason in [
                    StopReason::Cancelled,
                    StopReason::WorkLimit,
                    StopReason::AllocationLimit,
                    StopReason::DiagnosticLimit,
                ] {
                    let mut limits = budget().limits();
                    match reason {
                        StopReason::WorkLimit => limits.work = 0,
                        StopReason::AllocationLimit => limits.allocation_units = 0,
                        StopReason::DiagnosticLimit => limits.diagnostics = 0,
                        _ => {}
                    }
                    let mut measured = Budget::new(limits);
                    if reason == StopReason::Cancelled {
                        measured.cancel();
                    }
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(profile.registry(), &store, &mut admission)
                            .map_err(err)?;
                    let mut host = MathDisplayHost {
                        registry: profile.registry(),
                        math_surface: &compiled.others[0].schema,
                        sentence_surface: Some(&compiled.others[3].schema),
                        doc_surface: Some(&compiled.doc.package.schema),
                        codec: &mut codec,
                    };
                    let mut report = Report::new(Renderer::KatexPreferred);
                    assert!(
                        report
                            .render(guest, EmbedRef(0), 0, &mut host, &mut measured)
                            .is_err()
                    );
                    assert_eq!(measured.poll(), Err(reason));
                    assert!(report.occurrences.is_empty());
                    let used = measured.usage();
                    assert!(
                        report
                            .render(guest, EmbedRef(0), 0, &mut host, &mut measured)
                            .is_err()
                    );
                    assert_eq!(measured.usage(), used);
                }
                let mut limits = budget().limits();
                limits.diagnostics = 0;
                let mut measured = Budget::new(limits);
                let mut report = Report::new(Renderer::MathmlOnly);
                let mut host = MathDisplayHost {
                    registry: profile.registry(),
                    math_surface: &compiled.others[0].schema,
                    sentence_surface: Some(&compiled.others[3].schema),
                    doc_surface: Some(&compiled.doc.package.schema),
                    codec: &mut codec,
                };
                report.render(guest, EmbedRef(0), 0, &mut host, &mut measured)?;
                assert_eq!(report.occurrences[0].fallback, None);
                assert_eq!(measured.usage().diagnostics, 0);
                Ok(())
            },
        )
    }
}
