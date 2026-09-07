use crate::head::{HeadCall, HeadError, SourceWindow, compatible_windows};
use alloc::vec::Vec;
use nepl3_core::budget::{Budget, Resource};

/// A receiving operation's partial-window ledger, never a SourceAdmission or
/// a claim that a complete snapshot was supplied. Keep it with that operation's
/// cumulative Budget; create a fresh ledger for a new operation.
#[derive(Default)]
pub struct WindowAdmission {
    windows: Vec<SourceWindow>,
}
impl WindowAdmission {
    pub(super) fn admit(&mut self, call: &HeadCall, budget: &mut Budget) -> Result<(), HeadError> {
        for window in call.windows() {
            window.validate(budget)?;
            for previous in &self.windows {
                compatible_windows(previous, window, budget)?;
            }
            let uncovered = uncovered(window, self.windows.iter(), budget)?;
            budget.charge(
                Resource::Work,
                window.span.source.source_id.0.len() as u64 + window.bytes.len() as u64 + 1,
            )?;
            // Preparation precedes publication into the ledger. A stopped
            // charge retains prior accounting, without an uncharged exemption.
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<SourceWindow>() as u64
                    + window.span.source.source_id.0.len() as u64
                    + window.bytes.len() as u64,
            )?;
            budget.charge(Resource::SourceBytes, uncovered)?;
            self.windows.push(window.clone());
        }
        Ok(())
    }
}
pub(super) fn uncovered<'a>(
    window: &SourceWindow,
    previous: impl Iterator<Item = &'a SourceWindow> + Clone,
    budget: &mut Budget,
) -> Result<u64, HeadError> {
    let mut cursor = window.span.start;
    let mut uncovered = 0u64;
    while cursor < window.span.end {
        let mut covered_end = cursor;
        let mut next = window.span.end;
        for previous in previous.clone() {
            if !window.span.same_snapshot(&previous.span, budget)? {
                continue;
            }
            if previous.span.start <= cursor {
                covered_end = covered_end.max(previous.span.end.min(window.span.end));
            } else {
                next = next.min(previous.span.start);
            }
        }
        if covered_end > cursor {
            cursor = covered_end;
        } else {
            uncovered += next - cursor;
            cursor = next;
        }
    }
    Ok(uncovered)
}
