//! HTML byte emission retaining the exact local composite owner.
use super::{ComposedMath, Error};
use nepl3_core::budget::Budget;
use nepl3_markup::html;

/// Only the HTML fragment is emitted here. CSS and fixed resources remain on
/// the borrowed source and require their own metered artifact assembly.
/// This is not process-cleanup, CSP, resource or complete artifact admission.
/// ```compile_fail
/// fn mutate(s: &mut nepl3_tools::doc::math::display::generation::composite::serialize::SerializedComposite<'_, '_, '_>) { s.html.clear(); }
/// ```
/// ```compile_fail
/// fn extract(s: nepl3_tools::doc::math::display::generation::composite::serialize::SerializedComposite<'_, '_, '_>) { s.into_parts(); }
/// ```
/// ```compile_fail
/// fn duplicate(s: nepl3_tools::doc::math::display::generation::composite::serialize::SerializedComposite<'_, '_, '_>) { s.clone(); }
/// ```
/// ```compile_fail
/// use nepl3_tools::doc::math::display::generation::composite::{ComposedMath, serialize::SerializedComposite};
/// fn pair(source: &ComposedMath<'_, '_>, html: String) { let _ = SerializedComposite { source, html }; }
/// ```
#[must_use]
pub struct SerializedComposite<'a, 'c, 'r> {
    source: &'a ComposedMath<'c, 'r>,
    html: String,
}
impl<'a, 'c, 'r> SerializedComposite<'a, 'c, 'r> {
    pub fn source(&self) -> &'a ComposedMath<'c, 'r> {
        self.source
    }
    pub fn html(&self) -> &str {
        &self.html
    }
}
impl<'c, 'r> ComposedMath<'c, 'r> {
    /// Validate and serialize at the caller's current depth. Each invocation
    /// pays the full HTML validation/serialization cost, including emitted
    /// bytes. Existing CSS is neither copied nor emitted by this operation.
    pub fn serialize_html<'a>(
        &'a self,
        budget: &mut Budget,
    ) -> Result<SerializedComposite<'a, 'c, 'r>, Error> {
        budget.poll()?;
        let request = &self.math.markup;
        let proof = html::validate(&request.fragment, request.slot, &request.policy, budget)?;
        let html = html::serialize(&proof, budget)?;
        budget.poll()?;
        Ok(SerializedComposite { source: self, html })
    }
}
