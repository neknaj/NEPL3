//! Preferred-renderer evidence observes successful visuals, not negative host calls.
use super::*;
use crate::doc::export::math::{Fallback, generation::Occurrence};
use nepl3_doc_core::pages::PageSet;
const CONTEXT: &str = "nepl3.local-doc-pages.svg-katex-context/1";
pub(crate) struct Prepared {
    pub pages: Vec<Option<export::katex::Generated>>,
    pub contexts: Vec<String>,
}
fn context(source: Digest, page: usize, id: &str, b: &mut Budget) -> Result<String, String> {
    let size = (CONTEXT.len() + 1 + 32 + 8 + 8 + 1)
        .checked_add(id.len())
        .ok_or_else(|| err(b.stop(StopReason::AllocationLimit)))?;
    let work = size
        .checked_mul(2)
        .and_then(|n| n.checked_add(256))
        .ok_or_else(|| err(b.stop(StopReason::WorkLimit)))?;
    let allocation = size
        .checked_add(256)
        .ok_or_else(|| err(b.stop(StopReason::AllocationLimit)))?;
    metadata_charge(work, allocation, b)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
    bytes.extend_from_slice(CONTEXT.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&source.0);
    bytes.extend_from_slice(&(page as u64).to_le_bytes());
    bytes.extend_from_slice(&(id.len() as u64).to_le_bytes());
    bytes.extend_from_slice(id.as_bytes());
    bytes.push(1); // Explicit KaTeXPreferred tag; MathMLOnly never calls this path.
    Ok(export::digest_hex(Digest::of(&bytes)))
}
pub(super) fn prepare(
    records: &mut [Vec<Occurrence>],
    direct: &mut [Vec<export::math::Occurrence>],
    set: &PageSet,
    source: Digest,
    b: &mut Budget,
) -> Result<Prepared, String> {
    if records.len() != set.pages.len() || direct.len() != records.len() {
        return Err("NativePageCountMismatch".into());
    }
    let mut pages: Vec<Option<export::katex::Generated>> = Vec::new();
    let mut contexts = Vec::new();
    for (page, ((records, direct), document)) in
        records.iter_mut().zip(direct).zip(&set.pages).enumerate()
    {
        let context = context(source, page, &document.registration.id, b)?;
        // Account before finding the most recent successful immutable font cache.
        b.charge(Resource::Work, pages.len() as u64).map_err(err)?;
        let cached = pages.iter().rev().find_map(Option::as_ref);
        let result = export::katex::generate_cached(records, &context, cached, b)?;
        export::math::generation::settle(direct, records, b)?;
        for record in records.iter() {
            b.charge(Resource::Work, 1).map_err(err)?;
            let mut visual = false;
            if let Some(native) = &result {
                for item in &native.visuals {
                    b.charge(Resource::Work, 1).map_err(err)?;
                    visual |= item.root == record.root;
                }
            }
            if visual != record.fallback.is_none() {
                return Err("NativeVisualRecordMismatch".into());
            }
        }
        push(&mut pages, result, b)?;
        push(&mut contexts, context, b)?;
    }
    Ok(Prepared { pages, contexts })
}
impl Prepared {
    pub(super) fn evidence<'a>(
        &'a self,
        records: &'a [Vec<Occurrence>],
        set: &'a PageSet,
        b: &mut Budget,
    ) -> Result<output::NativeEvidence<'a>, String> {
        b.poll().map_err(err)?;
        b.charge(Resource::Work, 1).map_err(err)?;
        if self.pages.len() != records.len()
            || records.len() != set.pages.len()
            || self.contexts.len() != records.len()
        {
            return Err("NativeEvidencePageCountMismatch".into());
        }
        let mut pages = Vec::new();
        for (page, ((native, records), document)) in
            self.pages.iter().zip(records).zip(&set.pages).enumerate()
        {
            let mut files = Vec::new();
            let mut visuals = Vec::new();
            let mut formulas = Vec::new();
            if let Some(native) = native {
                for file in native.files.iter() {
                    metadata_charge(file.bytes.len() + 256, 256, b)?;
                    push(
                        &mut files,
                        output::NativeFile {
                            path: &file.path,
                            mime: &file.mime,
                            bytes: file.bytes.len(),
                            sha256: export::digest_hex(Digest::of(&file.bytes)),
                        },
                        b,
                    )?;
                }
                for visual in &native.visuals {
                    push(
                        &mut visuals,
                        output::NativeVisual {
                            root: visual.root,
                            scope: &visual.scope,
                        },
                        b,
                    )?;
                }
            }
            for record in records {
                let unsupported =
                    if let Some(Fallback::UnsupportedTex { node, reason }) = record.fallback {
                        metadata_charge(256, 256, b)?;
                        Some(output::UnsupportedEvidence {
                            node,
                            reason: format!("{reason:?}"),
                        })
                    } else {
                        None
                    };
                push(
                    &mut formulas,
                    output::NativeFormula {
                        root: record.root,
                        owner_path: &record.owner,
                        fallback_detail: record.fallback_detail.as_deref(),
                        unsupported,
                    },
                    b,
                )?;
            }
            push(
                &mut pages,
                output::NativePage {
                    page: &document.registration.id,
                    request_context: self.contexts.get(page).ok_or("NativeContextMissing")?,
                    node_version: native.as_ref().map(|n| n.node_version.as_str()),
                    visuals,
                    files,
                    formulas,
                },
                b,
            )?;
        }
        Ok(output::NativeEvidence {
            contract: CONTEXT,
            requested_renderer: "katex-preferred",
            pinned_version: "0.18.7",
            host_invocation_recorded: false,
            host_internal_work: "unobserved",
            host_internal_allocation: "unobserved",
            pages,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferred_context_is_length_delimited_and_budgeted() -> Result<(), String> {
        let source = Digest([7; 32]);
        let mut b = crate::doc::source::budget();
        b.charge(Resource::Work, 11).map_err(err)?;
        let actual = context(source, 3, "a:b", &mut b)?;
        let mut bytes = b"nepl3.local-doc-pages.svg-katex-context/1\0".to_vec();
        bytes.extend_from_slice(&[7; 32]);
        bytes.extend_from_slice(&3u64.to_le_bytes());
        bytes.extend_from_slice(&3u64.to_le_bytes());
        bytes.extend_from_slice(b"a:b");
        bytes.push(1);
        assert_eq!(actual, export::digest_hex(Digest::of(&bytes)));
        assert_ne!(
            actual,
            context(source, 4, "a:b", &mut crate::doc::source::budget())?
        );
        assert_ne!(
            actual,
            context(source, 3, "a", &mut crate::doc::source::budget())?
        );
        let mut limits = b.limits();
        limits.work = b.usage().work;
        limits.allocation_units = b.usage().allocation_units;
        let mut exact = Budget::new(limits);
        exact.charge(Resource::Work, 11).map_err(err)?;
        assert_eq!(context(source, 3, "a:b", &mut exact)?, actual);
        limits.allocation_units -= 1;
        let mut short = Budget::new(limits);
        short.charge(Resource::Work, 11).map_err(err)?;
        assert!(context(source, 3, "a:b", &mut short).is_err());
        assert_eq!(short.poll(), Err(StopReason::AllocationLimit));
        Ok(())
    }
}
