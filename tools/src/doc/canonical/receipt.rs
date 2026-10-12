//! Borrowed receipt serialization with a metered sizing pass and one output buffer.
//! The sizing pass uses maximum-width usage counters; the real counters can only
//! shorten the same JSON shape. No dependency Value trees are copied or cached.
use super::*;
use nepl3_core::budget::{Limits, Usage};
use serde::{Serialize, Serializer, ser::SerializeSeq};
use std::io::{self, Write};

#[derive(Serialize)]
struct Counters {
    allocation_units: u64,
    depth: u64,
    diagnostics: u64,
    events: u64,
    nodes: u64,
    output_bytes: u64,
    source_bytes: u64,
    work: u64,
}
impl From<Usage> for Counters {
    fn from(u: Usage) -> Self {
        Self {
            allocation_units: u.allocation_units,
            depth: u.depth,
            diagnostics: u.diagnostics,
            events: u.events,
            nodes: u.nodes,
            output_bytes: u.output_bytes,
            source_bytes: u.source_bytes,
            work: u.work,
        }
    }
}
impl From<Limits> for Counters {
    fn from(u: Limits) -> Self {
        Self {
            allocation_units: u.allocation_units,
            depth: u.depth,
            diagnostics: u.diagnostics,
            events: u.events,
            nodes: u.nodes,
            output_bytes: u.output_bytes,
            source_bytes: u.source_bytes,
            work: u.work,
        }
    }
}
struct Hex(Digest);
impl Serialize for Hex {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut bytes = [0u8; 64];
        for (i, byte) in self.0.0.iter().enumerate() {
            bytes[i * 2] = DIGITS[(byte >> 4) as usize];
            bytes[i * 2 + 1] = DIGITS[(byte & 15) as usize];
        }
        s.serialize_str(std::str::from_utf8(&bytes).map_err(serde::ser::Error::custom)?)
    }
}
#[derive(Serialize)]
struct Image<'a> {
    asset_id: &'a str,
    node: u64,
    route: &'a str,
    sha256: Hex,
}
struct Images<'a>(&'a [annotated::pages::ImageDependency]);
impl Serialize for Images<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        for i in self.0 {
            seq.serialize_element(&Image {
                asset_id: &i.asset_id,
                node: i.node,
                route: &i.route,
                sha256: Hex(i.digest),
            })?;
        }
        seq.end()
    }
}
#[derive(Serialize)]
struct Math {
    embed: u64,
    guest_sha256: Hex,
}
struct Maths<'a>(&'a [annotated::pages::MathDependency]);
impl Serialize for Maths<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        for m in self.0 {
            seq.serialize_element(&Math {
                embed: m.embed,
                guest_sha256: Hex(m.guest_digest),
            })?;
        }
        seq.end()
    }
}
#[derive(Serialize)]
struct Link<'a> {
    fragment: Option<&'a str>,
    node: u64,
    route: &'a str,
    target_id: &'a str,
    target_kind: &'a str,
}
struct Links<'a>(&'a [annotated::pages::LinkDependency]);
impl Serialize for Links<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        for link in self.0 {
            seq.serialize_element(&Link {
                fragment: link.fragment.as_deref(),
                node: link.node,
                route: &link.route,
                target_id: &link.target_id,
                target_kind: link.target_kind,
            })?;
        }
        seq.end()
    }
}
#[derive(Serialize)]
struct DependencyPage<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Images<'a>>,
    links: Links<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    math: Option<Maths<'a>>,
    path: &'a str,
}
struct Pages<'a> {
    inputs: &'a [Input],
    group: &'a annotated::pages::PagesArtifact,
    has_svg: bool,
}
impl Serialize for Pages<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.inputs.len()))?;
        for (((input, links), images), math) in self
            .inputs
            .iter()
            .zip(&self.group.dependencies)
            .zip(&self.group.image_dependencies)
            .zip(&self.group.math_dependencies)
        {
            seq.serialize_element(&DependencyPage {
                images: self.has_svg.then_some(Images(images)),
                links: Links(links),
                math: (!math.is_empty()).then_some(Maths(math)),
                path: &input.page.projection,
            })?;
        }
        seq.end()
    }
}
#[derive(Serialize)]
struct OutputBudget {
    initial_usage: Counters,
    limits: Counters,
    usage_before_receipt_serialization: Counters,
}
#[derive(Serialize)]
struct Receipt<'a> {
    files: &'a [serde_json::Value],
    format: &'static str,
    input_context: Option<Hex>,
    input_pageset: Option<Hex>,
    output_budget: OutputBudget,
    parse_limits: Counters,
    lower_limits: Counters,
    page_dependencies: Option<Pages<'a>>,
}

struct SizingWriter<'a> {
    budget: &'a mut Budget,
    len: usize,
}
impl Write for SizingWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.budget
            .charge(Resource::Work, bytes.len() as u64)
            .map_err(|_| io::Error::other("receipt sizing stopped"))?;
        self.len = self
            .len
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("ContextLimit"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// This buffer cannot grow beyond its admitted capacity, even if the sizing
/// invariant regresses. A serialization failure never returns partial output.
struct Output {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("receipt size bound exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// serde scans unescaped string runs before calling Write. Admit those scans
// first, using lengths only; the writer's charges account for emitted bytes.
// Fixed per-record fees cover scalar formatting and structural dispatch.
fn admit_scans(
    inputs: &[Input],
    group: Option<&annotated::pages::PagesArtifact>,
    has_svg: bool,
    records: &[serde_json::Value],
    budget: &mut Budget,
) -> Result<()> {
    charge(budget, Resource::Work, 2048)?; // root keys, 40 counters, two digests
    for record in records {
        charge(budget, Resource::Work, 256)?;
        let fields = record
            .as_object()
            .ok_or("receipt file record is not an object")?;
        for (key, value) in fields {
            charge(budget, Resource::Work, 32)?;
            charge(budget, Resource::Work, key.len())?;
            match value {
                serde_json::Value::String(text) => charge(budget, Resource::Work, text.len())?,
                serde_json::Value::Number(number) if number.is_u64() => (),
                _ => return Err("invalid receipt file record field".into()),
            }
        }
    }
    if let Some(g) = group {
        for (i, input) in inputs.iter().enumerate() {
            charge(budget, Resource::Work, 256)?;
            charge(budget, Resource::Work, input.page.projection.len())?;
            for link in &g.dependencies[i] {
                charge(budget, Resource::Work, 256)?;
                for text in [link.target_kind, &link.target_id, &link.route] {
                    charge(budget, Resource::Work, text.len())?;
                }
                if let Some(fragment) = &link.fragment {
                    charge(budget, Resource::Work, fragment.len())?;
                }
            }
            if has_svg {
                for image in &g.image_dependencies[i] {
                    charge(budget, Resource::Work, 256)?;
                    charge(budget, Resource::Work, image.asset_id.len())?;
                    charge(budget, Resource::Work, image.route.len())?;
                }
            }
            for _ in &g.math_dependencies[i] {
                charge(budget, Resource::Work, 256)?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    inputs: &[Input],
    group: Option<&annotated::pages::PagesArtifact>,
    has_svg: bool,
    identity: Option<Digest>,
    records: &[serde_json::Value],
    initial: Usage,
    phases: PhaseLimits,
    budget: &mut Budget,
) -> Result<String> {
    budget.poll().map_err(err)?;
    if let Some(g) = group
        && [
            g.dependencies.len(),
            g.image_dependencies.len(),
            g.math_dependencies.len(),
        ]
        .iter()
        .any(|n| *n != inputs.len())
    {
        return Err("receipt dependency count mismatch".into());
    }
    admit_scans(inputs, group, has_svg, records, budget)?;
    let limits = budget.limits();
    let make = |usage| Receipt {
        files: records,
        parse_limits: phases.parse.into(),
        lower_limits: phases.lower.into(),
        format: "nepl3.canonical-markdown-stage/2",
        input_context: identity.map(Hex),
        input_pageset: group.map(|g| Hex(g.identity)),
        output_budget: OutputBudget {
            initial_usage: initial.into(),
            limits: limits.into(),
            usage_before_receipt_serialization: usage,
        },
        page_dependencies: group.map(|group| Pages {
            inputs,
            group,
            has_svg,
        }),
    };
    let maximum = Counters {
        allocation_units: u64::MAX,
        depth: u64::MAX,
        diagnostics: u64::MAX,
        events: u64::MAX,
        nodes: u64::MAX,
        output_bytes: u64::MAX,
        source_bytes: u64::MAX,
        work: u64::MAX,
    };
    let (measured, sized) = {
        let mut writer = SizingWriter { budget, len: 0 };
        let result = serde_json::to_writer_pretty(&mut writer, &make(maximum));
        (writer.len, result)
    };
    // Preserve typed sticky stops rather than returning only serde's I/O text.
    budget.poll().map_err(err)?;
    sized?;
    let capacity = measured.checked_add(1).ok_or("ContextLimit")?;
    // The same escaped JSON shape bounds both the final input scan and emit.
    charge(
        budget,
        Resource::Work,
        capacity.checked_mul(2).ok_or("ContextLimit")?,
    )?;
    charge(budget, Resource::AllocationUnits, capacity)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity)?;
    let mut output = Output {
        bytes,
        limit: capacity,
    };
    serde_json::to_writer_pretty(&mut output, &make(budget.usage().into()))?;
    output.write_all(b"\n")?;
    charge(budget, Resource::OutputBytes, output.bytes.len())?;
    charge(budget, Resource::Work, output.bytes.len())?;
    Ok(String::from_utf8(output.bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nepl3_core::budget::StopReason;
    use serde_json::{Value, json};

    fn budget() -> Budget {
        Budget::new(Limits {
            source_bytes: u64::MAX,
            work: u64::MAX,
            depth: u64::MAX,
            nodes: u64::MAX,
            allocation_units: u64::MAX,
            output_bytes: u64::MAX,
            diagnostics: u64::MAX,
            events: u64::MAX,
        })
    }
    fn inputs(n: usize, path: &str) -> Vec<Input> {
        (0..n)
            .map(|_| Input {
                page: Page {
                    id: "p".into(),
                    source: "p.nepld".into(),
                    projection: path.into(),
                    aliases: "aliases.json".into(),
                    route: "p.html".into(),
                    renderer: RENDERER.into(),
                },
                source: String::new(),
                aliases: Vec::new(),
            })
            .collect()
    }
    fn group() -> annotated::pages::PagesArtifact {
        annotated::pages::PagesArtifact {
            identity: Digest([0xab; 32]),
            pages: Vec::new(),
            dependencies: vec![
                vec![
                    annotated::pages::LinkDependency {
                        node: u64::MAX,
                        target_kind: "page",
                        target_id: "日本語\n\"\\".into(),
                        route: "nested/page.md".into(),
                        fragment: Some("\u{0000}\t\r\"\\あ".into()),
                    },
                    annotated::pages::LinkDependency {
                        node: 0,
                        target_kind: "file",
                        target_id: "file".into(),
                        route: "file.md".into(),
                        fragment: None,
                    },
                ],
                Vec::new(),
            ],
            image_dependencies: vec![
                vec![annotated::pages::ImageDependency {
                    node: u64::MAX,
                    asset_id: "svg".into(),
                    route: "image.svg".into(),
                    digest: Digest([0x10; 32]),
                }],
                Vec::new(),
            ],
            math_dependencies: vec![
                vec![annotated::pages::MathDependency {
                    embed: u64::MAX,
                    guest_digest: Digest([0xef; 32]),
                }],
                Vec::new(),
            ],
        }
    }
    #[test]
    fn receipt_preserves_all_fields_without_dependency_value_copies() -> Result<()> {
        let inputs = inputs(2, "dir/page.md");
        let group = group();
        let records = vec![
            json!({"path":"dir/page.md","sha256":"a".repeat(64),
            "document_digest":"b".repeat(64),"bytes":u64::MAX}),
            json!({"path":"image.svg","sha256":"c".repeat(64),"bytes":0,"kind":"static-svg"}),
        ];
        for grouped in [None, Some(&group)] {
            for has_svg in [false, true] {
                let mut b = budget();
                let text = render(
                    &inputs,
                    grouped,
                    has_svg,
                    Some(Digest([0x01; 32])),
                    &records,
                    Usage::default(),
                    PhaseLimits::default(),
                    &mut b,
                )?;
                let mut actual: Value = serde_json::from_str(&text)?;
                let metadata = actual
                    .as_object_mut()
                    .ok_or("receipt")?
                    .remove("output_budget")
                    .ok_or("budget")?;
                // Independent legacy shape, including null and omitted values.
                let expected = json!({"format":"nepl3.canonical-markdown-stage/2",
                    "input_context":hex(Digest([1;32])),"input_pageset":grouped.map(|g| hex(g.identity)),
                    "parse_limits": crate::doc::export::pages::resources::OperationLimits::default(),
                    "lower_limits": crate::doc::export::pages::resources::OperationLimits::default(),
                    "files":records,"page_dependencies":grouped.map(|g| inputs.iter().enumerate().map(|(index,input)| {
                        let mut p = json!({"path":input.page.projection,"links":g.dependencies[index]});
                        if has_svg { p["images"] = Value::Array(g.image_dependencies[index].iter().map(|i|
                            json!({"node":i.node,"asset_id":i.asset_id,"route":i.route,"sha256":hex(i.digest)})).collect()); }
                        if !g.math_dependencies[index].is_empty() { p["math"] = Value::Array(g.math_dependencies[index].iter().map(|m|
                            json!({"embed":m.embed,"guest_sha256":hex(m.guest_digest)})).collect()); }
                        p
                    }).collect::<Vec<_>>())});
                assert_eq!(actual, expected);
                assert!(text.ends_with('\n'));
                assert_eq!(metadata["limits"]["work"], u64::MAX);
                assert_eq!(metadata["initial_usage"]["work"], 0);
                assert_eq!(
                    metadata["usage_before_receipt_serialization"]["work"],
                    b.usage().work - text.len() as u64
                );
                assert_eq!(
                    metadata["usage_before_receipt_serialization"]["allocation_units"],
                    b.usage().allocation_units
                );
                assert_eq!(b.usage().output_bytes, text.len() as u64);
                // Only eight maximum-width usage counters can shorten between
                // passes: at most 19 digits each. The newline is reserved too.
                assert!(b.usage().allocation_units >= text.len() as u64);
                assert!(b.usage().allocation_units <= text.len() as u64 + 8 * 19);
                assert!(b.usage().work >= 3 * b.usage().allocation_units - 1 + text.len() as u64);
            }
        }
        Ok(())
    }
    #[test]
    fn receipt_short_paths_and_maximum_paths_use_actual_serialized_shape() -> Result<()> {
        for path in ["p.md".to_owned(), format!("{}.md", "a".repeat(4093))] {
            let inputs = inputs(128, &path);
            let group = annotated::pages::PagesArtifact {
                identity: Digest([0; 32]),
                pages: Vec::new(),
                dependencies: (0..128).map(|_| Vec::new()).collect(),
                image_dependencies: (0..128).map(|_| Vec::new()).collect(),
                math_dependencies: (0..128).map(|_| Vec::new()).collect(),
            };
            let records: Vec<_> = (0..128)
                .map(|_| {
                    json!({"path":path,"sha256":"0".repeat(64),
                "bytes":0,"document_digest":"1".repeat(64)})
                })
                .collect();
            let mut b = budget();
            let text = render(
                &inputs,
                Some(&group),
                true,
                None,
                &records,
                Usage::default(),
                PhaseLimits::default(),
                &mut b,
            )?;
            let actual: Value = serde_json::from_str(&text)?;
            assert_eq!(
                actual["page_dependencies"].as_array().ok_or("pages")?.len(),
                128
            );
            assert!(
                actual["page_dependencies"][0]["images"]
                    .as_array()
                    .ok_or("images")?
                    .is_empty()
            );
            assert!(actual["page_dependencies"][0].get("math").is_none());
            assert!(actual["input_context"].is_null());
            assert!(b.usage().allocation_units <= text.len() as u64 + 152);
            // Short paths no longer pay for hypothetical 4096-byte paths.
            if path.len() == 4 {
                assert!(b.usage().work < 500_000);
            }
        }
        Ok(())
    }
    #[test]
    fn receipt_limits_and_cancellation_are_sticky_without_partial_results() -> Result<()> {
        for reason in [
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::OutputLimit,
            StopReason::Cancelled,
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                StopReason::OutputLimit => limits.output_bytes = 0,
                _ => (),
            }
            let mut b = Budget::new(limits);
            if reason == StopReason::Cancelled {
                b.cancel();
            }
            assert!(
                render(
                    &[],
                    None,
                    false,
                    None,
                    &[],
                    Usage::default(),
                    PhaseLimits::default(),
                    &mut b
                )
                .is_err()
            );
            let used = b.usage();
            assert_eq!(b.poll(), Err(reason));
            assert!(
                render(
                    &[],
                    None,
                    false,
                    None,
                    &[],
                    Usage::default(),
                    PhaseLimits::default(),
                    &mut b
                )
                .is_err()
            );
            assert_eq!(b.usage(), used);
            if reason != StopReason::OutputLimit {
                assert_eq!(used.allocation_units, 0);
            }
        }
        Ok(())
    }
    #[test]
    fn receipt_positive_headroom_exact_fit_and_one_short_keep_limits_fixed() -> Result<()> {
        fn used(b: &Budget, resource: Resource) -> u64 {
            match resource {
                Resource::Work => b.usage().work,
                Resource::AllocationUnits => b.usage().allocation_units,
                Resource::OutputBytes => b.usage().output_bytes,
                _ => 0,
            }
        }
        for (resource, reason) in [
            (Resource::Work, StopReason::WorkLimit),
            (Resource::AllocationUnits, StopReason::AllocationLimit),
            (Resource::OutputBytes, StopReason::OutputLimit),
        ] {
            // These are separate operations with identical selected limits and
            // initial receipt metadata. All varied counters stay 20 digits.
            let seed = u64::MAX - 1_000_000;
            let mut baseline = budget();
            baseline.charge(resource, seed).map_err(err)?;
            render(
                &[],
                None,
                false,
                None,
                &[],
                Usage::default(),
                PhaseLimits::default(),
                &mut baseline,
            )?;
            let cost = used(&baseline, resource) - seed;
            assert!(cost > 0 && cost < 1_000_000);
            let mut exact = budget();
            exact.charge(resource, u64::MAX - cost).map_err(err)?;
            render(
                &[],
                None,
                false,
                None,
                &[],
                Usage::default(),
                PhaseLimits::default(),
                &mut exact,
            )?;
            assert_eq!(used(&exact, resource), u64::MAX);
            let mut short = budget();
            short.charge(resource, u64::MAX - cost + 1).map_err(err)?;
            assert!(
                render(
                    &[],
                    None,
                    false,
                    None,
                    &[],
                    Usage::default(),
                    PhaseLimits::default(),
                    &mut short
                )
                .is_err()
            );
            assert_eq!(short.poll(), Err(reason));
            let usage = short.usage();
            assert!(
                render(
                    &[],
                    None,
                    false,
                    None,
                    &[],
                    Usage::default(),
                    PhaseLimits::default(),
                    &mut short
                )
                .is_err()
            );
            assert_eq!(short.usage(), usage);
        }
        Ok(())
    }
    #[test]
    fn receipt_positive_work_exhaustion_at_sizing_and_final_admission() -> Result<()> {
        let mut baseline = budget();
        let text = render(
            &[],
            None,
            false,
            None,
            &[],
            Usage::default(),
            PhaseLimits::default(),
            &mut baseline,
        )?;
        let metadata: Value = serde_json::from_str(&text)?;
        let admitted = metadata["output_budget"]["usage_before_receipt_serialization"]["work"]
            .as_u64()
            .ok_or("work")?;
        let before_final = admitted - 2 * baseline.usage().allocation_units;
        // The first boundary pays only preflight; the second also pays sizing.
        for allowance in [2048, before_final] {
            let mut b = budget();
            b.charge(Resource::Work, u64::MAX - allowance)
                .map_err(err)?;
            assert!(
                render(
                    &[],
                    None,
                    false,
                    None,
                    &[],
                    Usage::default(),
                    PhaseLimits::default(),
                    &mut b
                )
                .is_err()
            );
            assert_eq!(b.poll(), Err(StopReason::WorkLimit));
            assert_eq!(b.usage().work, u64::MAX);
            assert_eq!(b.usage().allocation_units, 0);
            assert_eq!(b.usage().output_bytes, 0);
        }
        Ok(())
    }
    #[test]
    fn receipt_buffer_cannot_reallocate_and_sizing_overflow_is_rejected() -> Result<()> {
        let mut output = Output {
            bytes: Vec::with_capacity(3),
            limit: 3,
        };
        let capacity = output.bytes.capacity();
        output.write_all(b"abc")?;
        assert!(output.write_all(b"d").is_err());
        assert_eq!(output.bytes, b"abc");
        assert_eq!(output.bytes.capacity(), capacity);
        let mut b = budget();
        let mut writer = SizingWriter {
            budget: &mut b,
            len: usize::MAX,
        };
        assert!(writer.write_all(b"x").is_err());
        assert_eq!(writer.len, usize::MAX);
        let inputs = inputs(1, "p.md");
        let g = group();
        assert!(
            render(
                &inputs,
                Some(&g),
                false,
                None,
                &[],
                Usage::default(),
                PhaseLimits::default(),
                &mut b
            )
            .is_err()
        );
        Ok(())
    }
}
