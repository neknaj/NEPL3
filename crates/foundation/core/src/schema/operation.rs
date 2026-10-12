//! Borrowed, metered operation resolution and descriptor comparison.
use super::{OperationDescriptor, SchemaRegistry, TypeDescriptor, lookup};
use crate::{
    budget::{Budget, Resource, StopReason},
    value::OperationRef,
};
use core::cmp::Ordering;

impl SchemaRegistry {
    /// Resolve an exact schema identity and operation name without copying
    /// descriptors. Registration establishes canonical operation-name order.
    /// This does not validate input/output values or authorize execution.
    pub fn operation_with_budget(
        &self,
        reference: &OperationRef,
        budget: &mut Budget,
    ) -> Result<Option<&OperationDescriptor>, StopReason> {
        let Some(descriptor) = self.descriptor_with_budget(&reference.schema, budget)? else {
            return Ok(None);
        };
        let (mut low, mut high) = (0, descriptor.operations.len());
        while low < high {
            let middle = low + (high - low) / 2;
            let operation = &descriptor.operations[middle];
            match lookup::compare(&operation.name, &reference.name, budget)? {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(Some(operation)),
            }
        }
        Ok(None)
    }
}

impl TypeDescriptor {
    /// Iterative structural equality with each visited layer and name comparison
    /// admitted before use. No allocation, resolution or validation proof.
    pub fn equal_with_budget(&self, other: &Self, budget: &mut Budget) -> Result<bool, StopReason> {
        let (mut left, mut right, mut depth) = (self, other, 1u64);
        loop {
            budget.charge(Resource::Work, 1)?;
            budget.observe_depth(depth)?;
            match (left, right) {
                (Self::List(a), Self::List(b)) | (Self::Option(a), Self::Option(b)) => {
                    left = a;
                    right = b;
                    depth = depth
                        .checked_add(1)
                        .ok_or_else(|| budget.stop(StopReason::DepthLimit))?;
                }
                (Self::Named(a), Self::Named(b)) => {
                    if lookup::compare(&a.package, &b.package, budget)? != Ordering::Equal
                        || a.revision != b.revision
                    {
                        return Ok(false);
                    }
                    return Ok(lookup::compare(&a.name, &b.name, budget)? == Ordering::Equal);
                }
                _ => return Ok(core::mem::discriminant(left) == core::mem::discriminant(right)),
            }
        }
    }
}

#[cfg(test)]
mod tests;
