use super::*;

/// A completed node retains its maximum descendant depth. Reusing only a
/// visited bit would hide deep paths when a shared child was checked earlier.
#[derive(Clone, Copy)]
enum GraphState {
    Unseen,
    Active,
    Complete(u64),
}
struct GraphFrame {
    index: usize,
    next_child: usize,
    height: u64,
}
pub(super) struct Cache {
    states: Vec<GraphState>,
    touched: Vec<usize>,
}
impl Cache {
    pub(super) fn new(
        package: &LanguagePackage,
        budget: &mut Budget,
        at: &mut Option<BindingId>,
    ) -> Result<Self, PackageError> {
        let mut states = Vec::new();
        for index in 0..package.bindings.len() {
            *at = Some(BindingId(index as u64));
            budget.charge(Resource::Work, 1)?;
            push(&mut states, GraphState::Unseen, budget)?;
        }
        Ok(Self {
            states,
            touched: Vec::new(),
        })
    }
    fn reset(
        &mut self,
        budget: &mut Budget,
        at: &mut Option<BindingId>,
    ) -> Result<(), PackageError> {
        while let Some(&index) = self.touched.last() {
            *at = Some(BindingId(index as u64));
            budget.charge(Resource::Work, 1)?;
            self.states[index] = GraphState::Unseen;
            self.touched.pop();
        }
        Ok(())
    }
}
/// Reuse storage, never validation results, across distinct owners. Reset work
/// follows the previous reachable set, not the entire arena for every owner.
pub(super) fn walk(
    package: &LanguagePackage,
    roots: impl Iterator<Item = BindingId>,
    cache: &mut Cache,
    budget: &mut Budget,
    at: &mut Option<BindingId>,
    mut visit: impl FnMut(&Binding, &mut Budget) -> Result<(), PackageError>,
) -> Result<(), PackageError> {
    cache.reset(budget, at)?;
    let mut stack: Vec<GraphFrame> = Vec::new();
    for root in roots {
        let mut pending = Some(root);
        loop {
            if let Some(id) = pending.take() {
                *at = Some(id);
                budget.charge(Resource::Work, 1)?;
                budget.observe_depth(stack.len() as u64 + 1)?;
                let index = usize::try_from(id.0)
                    .ok()
                    .filter(|i| *i < cache.states.len())
                    .ok_or(PackageError::InvalidBinding)?;
                match cache.states[index] {
                    GraphState::Active => return Err(PackageError::DirectCycle),
                    // Admit a cached subtree atomically. A depth stop names this
                    // entry, not a descendant that was never traversed here.
                    GraphState::Complete(height) => {
                        budget.observe_depth(stack.len() as u64 + height)?;
                        if let Some(parent) = stack.last_mut() {
                            parent.height = parent.height.max(height + 1);
                        }
                    }
                    GraphState::Unseen => {
                        push(&mut cache.touched, index, budget)?;
                        push(
                            &mut stack,
                            GraphFrame {
                                index,
                                next_child: 0,
                                height: 1,
                            },
                            budget,
                        )?;
                        cache.states[index] = GraphState::Active;
                        visit(&package.bindings[index], budget)?;
                    }
                }
            }
            let Some(frame) = stack.last_mut() else {
                break;
            };
            *at = Some(BindingId(frame.index as u64));
            budget.charge(Resource::Work, 1)?;
            let child = match &package.bindings[frame.index] {
                Binding::Group(children) | Binding::Scope(children) => {
                    children.get(frame.next_child).copied()
                }
                _ => None,
            };
            if let Some(child) = child {
                frame.next_child += 1;
                pending = Some(child);
            } else {
                let height = frame.height;
                cache.states[frame.index] = GraphState::Complete(height);
                stack.pop();
                if let Some(parent) = stack.last_mut() {
                    parent.height = parent.height.max(height + 1);
                }
            }
        }
    }
    Ok(())
}
