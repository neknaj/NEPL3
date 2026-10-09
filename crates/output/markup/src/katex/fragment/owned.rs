//! Pure native owned visual content, without asset or document admission.
use super::{Error, Fragment, Policy, Rendered, serialize, validate};
use alloc::{string::String, vec::Vec};
use nepl3_core::budget::{Budget, Resource, StopReason};

/// Immutable visual content with an exact copied host class/scope policy.
/// This is not a renderer authenticity, stylesheet identity, scope uniqueness,
/// accessible Math artifact, portable proof or HTML insertion capability.
/// Input buffers existed before this call: ownership transfer does not measure
/// or cap their spare capacity. The native caller admits that preexisting
/// storage; this API meters new allocations, validation and emitted output.
///
/// Immutable access cannot modify retained content:
/// ```compile_fail
/// use nepl3_markup::katex::fragment::PreparedVisual;
/// fn mutate(parts: &PreparedVisual) { parts.fragment().nodes.clear(); }
/// ```
/// No unbudgeted owned clone is supplied:
/// ```compile_fail
/// use nepl3_markup::katex::fragment::PreparedVisual;
/// fn duplicate(parts: &PreparedVisual) -> PreparedVisual { parts.clone() }
/// ```
pub struct PreparedVisual {
    fragment: Fragment,
    classes: Vec<String>,
    scope: String,
}
impl PreparedVisual {
    /// Consume visual content into typed HTML plus generated CSS. This emits
    /// only CSS OutputBytes; later HTML serialization charges its own bytes.
    /// Resource binding, accessible MathML and artifact admission remain with
    /// the host. The returned object does not establish those properties.
    pub fn into_html(
        self,
        b: &mut Budget,
    ) -> Result<super::ProjectedVisual, super::ProjectionError> {
        b.charge(Resource::Work, self.classes.len() as u64 + 1)?;
        let stylesheet = {
            let mut classes = reserve::<&str>(self.classes.len(), b)?;
            for class in &self.classes {
                classes.push(class.as_str());
            }
            let checked = validate(
                &self.fragment,
                &Policy {
                    classes: &classes,
                    scope: &self.scope,
                },
                b,
            )?;
            super::projection::stylesheet(&checked, b)?
        };
        super::projection::project(self.fragment, self.classes, self.scope, stylesheet, b)
    }
    pub fn fragment(&self) -> &Fragment {
        &self.fragment
    }
    pub fn scope(&self) -> &str {
        &self.scope
    }
    pub fn classes(&self) -> &[String] {
        &self.classes
    }
    /// Revalidates at the current caller depth with the supplied sticky Budget.
    /// Repeat calls revalidate and charge every emitted HTML/CSS byte again.
    pub fn serialize(&self, b: &mut Budget) -> Result<Rendered, Error> {
        b.poll()?;
        let mut classes = reserve::<&str>(self.classes.len(), b)?;
        for class in &self.classes {
            b.charge(Resource::Work, 1)?;
            classes.push(class.as_str());
        }
        let policy = Policy {
            classes: &classes,
            scope: &self.scope,
        };
        let checked = validate(&self.fragment, &policy, b)?;
        serialize(&checked, b)
    }
}
pub(super) fn reserve<T>(count: usize, b: &mut Budget) -> Result<Vec<T>, Error> {
    let bytes = count
        .checked_mul(core::mem::size_of::<T>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    Ok(values)
}
pub(super) fn copy(value: &str, b: &mut Budget) -> Result<String, Error> {
    b.charge(Resource::Work, value.len() as u64)?;
    b.charge(Resource::AllocationUnits, value.len() as u64)?;
    let mut text = String::new();
    text.try_reserve_exact(value.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    text.push_str(value);
    Ok(text)
}
/// Validate before retaining the moved native tree and copied host policy.
/// Emits zero OutputBytes; any failure returns no prepared object. This prepares
/// content only. Classes must later be bound to independently verified assets.
pub fn prepare_owned(
    fragment: Fragment,
    policy: &Policy<'_>,
    b: &mut Budget,
) -> Result<PreparedVisual, Error> {
    validate(&fragment, policy, b)?;
    let scope = copy(policy.scope, b)?;
    let mut classes = reserve(policy.classes.len(), b)?;
    for class in policy.classes {
        classes.push(copy(class, b)?);
    }
    b.poll()?;
    Ok(PreparedVisual {
        fragment,
        classes,
        scope,
    })
}
