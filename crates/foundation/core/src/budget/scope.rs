//! Stack-only restoration of scoped state, including Rust unwinding. These
//! guards do not refund usage, clear stops, recover panics, or handle aborts.
use super::{Budget, Limits};
pub(super) struct Depth<'a> {
    pub budget: &'a mut Budget,
    pub previous: u64,
}
impl Drop for Depth<'_> {
    fn drop(&mut self) {
        self.budget.depth = self.previous;
    }
}
pub(super) struct Ceiling<'a> {
    pub budget: &'a mut Budget,
    pub previous: Limits,
}
impl Drop for Ceiling<'_> {
    fn drop(&mut self) {
        self.budget.limits = self.previous;
    }
}
pub(super) struct Measurement<'a> {
    pub budget: &'a mut Budget,
    pub previous: u64,
}
impl Drop for Measurement<'_> {
    fn drop(&mut self) {
        self.budget.observed_depth = self.previous.max(self.budget.observed_depth);
    }
}
