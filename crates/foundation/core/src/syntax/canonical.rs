//! Bundle-local root-first DFS numbering. This mapping checks reference bounds
//! and reachability only; syntax/category/source validation remains separate.
use super::{FieldValue, NodeRef, SyntaxBundle};
use crate::budget::{Budget, Resource, StopReason};
use alloc::vec::Vec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalError {
    Stopped(StopReason),
    Reference,
    Unreachable,
}
impl From<StopReason> for CanonicalError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
pub struct NodeMapping<'a> {
    bundle: &'a SyntaxBundle,
    order: Vec<usize>,
    indices: Vec<u64>,
}
fn push<T>(v: &mut Vec<T>, x: T, b: &mut Budget) -> Result<(), CanonicalError> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    v.push(x);
    Ok(())
}
impl<'a> NodeMapping<'a> {
    pub fn new(bundle: &'a SyntaxBundle, b: &mut Budget) -> Result<Self, CanonicalError> {
        b.charge(Resource::Work, 1)?;
        b.charge(
            Resource::AllocationUnits,
            (bundle.nodes.len() as u64).saturating_mul(core::mem::size_of::<u64>() as u64),
        )?;
        let mut indices = alloc::vec![u64::MAX; bundle.nodes.len()];
        let mut order = Vec::new();
        let mut pending = Vec::new();
        push(&mut pending, bundle.root, b)?;
        while let Some(id) = pending.pop() {
            b.charge(Resource::Work, 1)?;
            let index = usize::try_from(id.0).map_err(|_| CanonicalError::Reference)?;
            let old = indices.get_mut(index).ok_or(CanonicalError::Reference)?;
            if *old != u64::MAX {
                continue;
            }
            *old = order.len() as u64;
            push(&mut order, index, b)?;
            let node = bundle.nodes.get(index).ok_or(CanonicalError::Reference)?;
            for field in node.fields.iter().rev() {
                b.charge(Resource::Work, 1)?;
                match field {
                    FieldValue::Child(id) => push(&mut pending, *id, b)?,
                    FieldValue::Children(ids) => {
                        for id in ids.iter().rev() {
                            push(&mut pending, *id, b)?;
                        }
                    }
                    FieldValue::Atom(_) | FieldValue::Foreign(_) => {}
                }
            }
        }
        if order.len() != bundle.nodes.len() {
            return Err(CanonicalError::Unreachable);
        }
        Ok(Self {
            bundle,
            order,
            indices,
        })
    }
    pub fn bundle(&self) -> &'a SyntaxBundle {
        self.bundle
    }
    pub fn order(&self) -> &[usize] {
        &self.order
    }
    pub fn mapped(&self, id: NodeRef) -> Result<NodeRef, CanonicalError> {
        self.indices
            .get(usize::try_from(id.0).map_err(|_| CanonicalError::Reference)?)
            .copied()
            .filter(|i| *i != u64::MAX)
            .map(NodeRef)
            .ok_or(CanonicalError::Reference)
    }
    /// Allows adapters to reuse the exact mapping without a second allocation.
    pub fn into_parts(self) -> (Vec<usize>, Vec<u64>) {
        (self.order, self.indices)
    }
}
