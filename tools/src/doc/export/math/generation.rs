//! Generation-only views of retained formula occurrences. Owner paths identify
//! exact display occurrences, not globally meaningful embed numbers. This is
//! not an admission API for untrusted provider output.
use super::err;
use super::{Fallback, Occurrence as Direct};
use crate::doc::{
    annotations::{ForeignRecord, MathRecord, article::render::SentenceRecord},
    math::display::{Preference, TexPreparation, prepare_tex},
};
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_markup::mathml::Display;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum OwnerStep {
    Doc {
        placement: usize,
        embed: u64,
    },
    Sentence {
        placement: usize,
        occurrence: usize,
        owner: usize,
    },
    SentenceForeign {
        occurrence: usize,
        embed: u64,
    },
    DocumentMath {
        occurrence: usize,
        embed: u64,
    },
    MathAnnotation {
        occurrence: usize,
        embed: u64,
    },
}
pub struct Occurrence {
    pub root: u64,
    pub display: Display,
    pub tex: Arc<TexPreparation>,
    pub fallback: Option<Fallback>,
    pub fallback_detail: Option<String>,
    pub owner: Vec<OwnerStep>,
    direct: Option<usize>,
}
fn push<T>(items: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), String> {
    b.charge(Resource::Work, 1).map_err(err)?;
    b.charge(
        Resource::AllocationUnits,
        (core::mem::size_of::<T>() as u64).saturating_mul(2),
    )
    .map_err(err)?;
    items
        .try_reserve(1)
        .map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
    items.push(value);
    Ok(())
}
fn text(value: &Option<String>, b: &mut Budget) -> Result<Option<String>, String> {
    value
        .as_ref()
        .map(|value| {
            b.charge(Resource::Work, value.len() as u64).map_err(err)?;
            b.charge(Resource::AllocationUnits, value.len() as u64)
                .map_err(err)?;
            Ok(value.clone())
        })
        .transpose()
}
struct Collector {
    preference: Preference,
    output: Vec<Occurrence>,
    path: Vec<OwnerStep>,
    visual_ancestor: bool,
}
impl Collector {
    fn math(
        &mut self,
        record: &MathRecord,
        direct: Option<(usize, &Direct)>,
        b: &mut Budget,
    ) -> Result<(), String> {
        b.with_depth(|b| Ok::<_, StopReason>(self.math_inner(record, direct, b)))
            .map_err(err)?
    }
    fn math_inner(
        &mut self,
        record: &MathRecord,
        direct: Option<(usize, &Direct)>,
        b: &mut Budget,
    ) -> Result<(), String> {
        let (tex, fallback, detail) = if let Some((_, direct)) = direct {
            (
                Arc::clone(&direct.tex),
                direct.fallback,
                text(&direct.fallback_detail, b)?,
            )
        } else {
            let tex =
                prepare_tex::<core::convert::Infallible>(&record.syntax.value, self.preference, b)
                    .map_err(err)?;
            let fallback = match &tex {
                TexPreparation::MathMLOnly => None,
                TexPreparation::Unsupported { node, reason } => Some(Fallback::UnsupportedTex {
                    node: *node,
                    reason: *reason,
                }),
                TexPreparation::Ready { .. } => Some(Fallback::HostCapabilityUnavailable),
            };
            if fallback.is_some() {
                b.charge(Resource::Diagnostics, 1).map_err(err)?;
            }
            b.charge(
                Resource::AllocationUnits,
                (core::mem::size_of::<TexPreparation>() + 2 * core::mem::size_of::<usize>()) as u64,
            )
            .map_err(err)?;
            (Arc::new(tex), fallback, None)
        };
        let ready = matches!(&*tex, TexPreparation::Ready { .. });
        if ready && self.visual_ancestor {
            return Err("NestedKaTeXAncestorConflict".into());
        }
        let previous_ancestor = self.visual_ancestor;
        self.visual_ancestor |= ready;
        let mut owner = Vec::new();
        for step in &self.path {
            push(&mut owner, *step, b)?;
        }
        push(
            &mut self.output,
            Occurrence {
                root: record.root,
                display: record.display,
                tex,
                fallback,
                fallback_detail: detail,
                owner,
                direct: direct.map(|(index, _)| index),
            },
            b,
        )?;
        for (index, annotation) in record.annotations.iter().enumerate() {
            push(
                &mut self.path,
                OwnerStep::MathAnnotation {
                    occurrence: index,
                    embed: annotation.embed,
                },
                b,
            )?;
            self.foreign(&annotation.foreign, b)?;
            self.path.pop();
        }
        self.visual_ancestor = previous_ancestor;
        Ok(())
    }
    fn foreign(&mut self, records: &[ForeignRecord], b: &mut Budget) -> Result<(), String> {
        for (index, record) in records.iter().enumerate() {
            push(
                &mut self.path,
                OwnerStep::SentenceForeign {
                    occurrence: index,
                    embed: record.embed().0,
                },
                b,
            )?;
            match record {
                ForeignRecord::Math(record) => self.math(&record.output, None, b)?,
                ForeignRecord::Document(record) => {
                    for (index, nested) in record.foreign.iter().enumerate() {
                        push(
                            &mut self.path,
                            OwnerStep::DocumentMath {
                                occurrence: index,
                                embed: nested.embed.0,
                            },
                            b,
                        )?;
                        self.math(&nested.output, None, b)?;
                        self.path.pop();
                    }
                }
            }
            self.path.pop();
        }
        Ok(())
    }
}
/// Inputs have already been remapped into the final Article/page arena. Do not
/// offset nested records a second time. Retained source owners remain intact.
pub fn collect(
    direct: &[Direct],
    sentences: &[SentenceRecord],
    preference: Preference,
    b: &mut Budget,
) -> Result<Vec<Occurrence>, String> {
    b.poll().map_err(err)?;
    let mut collector = Collector {
        preference,
        output: Vec::new(),
        path: Vec::new(),
        visual_ancestor: false,
    };
    for (index, record) in direct.iter().enumerate() {
        if record.preference != preference {
            return Err("MathPreferenceMismatch".into());
        }
        if record.root != record.output.root || record.display != record.output.display {
            return Err("MathRootIdentity".into());
        }
        push(
            &mut collector.path,
            OwnerStep::Doc {
                placement: record.placement.ok_or("MathUncomposed")?,
                embed: record.embed.0,
            },
            b,
        )?;
        collector.math(&record.output, Some((index, record)), b)?;
        collector.path.pop();
    }
    for record in sentences {
        push(
            &mut collector.path,
            OwnerStep::Sentence {
                placement: record.placement,
                occurrence: record.occurrence.0,
                owner: record.owner.0,
            },
            b,
        )?;
        collector.foreign(&record.foreign, b)?;
        collector.path.pop();
    }
    // Formula roots are unique occurrences in the final arena. Keep diagnostics
    // and generation requests in that arena order, across direct and nested Math.
    let count = collector.output.len() as u64;
    let levels = u64::BITS - count.leading_zeros();
    b.charge(
        Resource::Work,
        count
            .saturating_mul(u64::from(levels) + 1)
            .saturating_mul(4),
    )
    .map_err(err)?;
    collector.output.sort_unstable_by_key(|record| record.root);
    for pair in collector.output.windows(2) {
        b.charge(Resource::Work, 1).map_err(err)?;
        if pair[0].root == pair[1].root {
            return Err("DuplicateMathRoot".into());
        }
    }
    b.poll().map_err(err)?;
    Ok(collector.output)
}
/// Preserve the existing direct-record diagnostic state after generation.
pub fn settle(
    direct: &mut [Direct],
    generated: &[Occurrence],
    b: &mut Budget,
) -> Result<(), String> {
    for record in generated {
        b.charge(Resource::Work, 1).map_err(err)?;
        if let Some(index) = record.direct {
            let target = direct.get_mut(index).ok_or("MathOccurrence")?;
            target.fallback = record.fallback;
            target.fallback_detail = text(&record.fallback_detail, b)?;
        }
    }
    Ok(())
}
pub fn diagnostics(records: &[Occurrence]) -> Vec<serde_json::Value> {
    records.iter().enumerate().filter_map(|(occurrence,record)|record.fallback.map(|reason|{
        let embed=match record.owner.as_slice(){[OwnerStep::Doc{embed,..}]=>Some(*embed),_=>None};
        serde_json::json!({"occurrence":occurrence,"embed":embed,"owner_path":record.owner,"renderer":"MathML","host_detail":record.fallback_detail,
            "detail":match reason{Fallback::UnsupportedTex{node,reason}=>Some(serde_json::json!({"node":node,"reason":format!("{reason:?}")})),_=>None},
            "reason":match reason{Fallback::HostCapabilityUnavailable=>"KaTeXHostCapabilityUnavailable",Fallback::ResourceUnavailable=>"KaTeXResourceUnavailable",Fallback::RendererParseError=>"KaTeXRendererParseError",Fallback::UnsupportedTex{..}=>"FaithfulTexUnsupported"}})
    })).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{
        export,
        source::{budget, compiled},
    };

    #[test]
    fn collection_rejects_uncomposed_duplicate_roots_preferences_and_visual_ancestors()
    -> Result<(), String> {
        let compiled = compiled()?;
        let mut result = export::generate_with_renderer(
            &compiled,
            r#"article en "Math" body cons display Math frac 1 2 cons display Math frac 2 3 nil"#,
            export::CssMode::Inline,
            Preference::MathMLOnly,
        )?;
        assert_eq!(
            collect(&result.math, &[], Preference::KaTeXPreferred, &mut budget())
                .err()
                .as_deref(),
            Some("MathPreferenceMismatch")
        );
        result.math[0].placement = None;
        assert_eq!(
            collect(&result.math, &[], Preference::MathMLOnly, &mut budget())
                .err()
                .as_deref(),
            Some("MathUncomposed")
        );
        result.math[0].placement = Some(0);
        result.math[1].root = result.math[0].root;
        result.math[1].output.root = result.math[0].root;
        assert_eq!(
            collect(&result.math, &[], Preference::MathMLOnly, &mut budget())
                .err()
                .as_deref(),
            Some("DuplicateMathRoot")
        );
        let mut nested = export::generate_with_renderer(
            &compiled,
            r#"article en "Math" body cons display Math label x Sentence sentence cons math frac 1 2 nil nil"#,
            export::CssMode::Inline,
            Preference::MathMLOnly,
        )?;
        assert_eq!(nested.math.len(), 1);
        // Model a future converter admitting the parent. A nested eligible
        // formula may not silently become a visual inside a hidden subtree.
        nested.math[0].preference = Preference::KaTeXPreferred;
        nested.math[0].tex = Arc::new(TexPreparation::Ready {
            tex: "x".into(),
            occurrences: Vec::new(),
        });
        assert_eq!(
            collect(&nested.math, &[], Preference::KaTeXPreferred, &mut budget())
                .err()
                .as_deref(),
            Some("NestedKaTeXAncestorConflict")
        );
        for reason in [
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::Cancelled,
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                _ => {}
            }
            let mut operation = Budget::new(limits);
            if reason == StopReason::Cancelled {
                operation.cancel();
            }
            assert!(
                collect(
                    &nested.math,
                    &[],
                    Preference::KaTeXPreferred,
                    &mut operation
                )
                .is_err()
            );
            assert_eq!(operation.poll(), Err(reason));
        }
        Ok(())
    }
}
