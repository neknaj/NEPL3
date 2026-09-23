use super::*;
use core::cmp::Ordering;

pub(in crate::syntax) struct EnvironmentIndex {
    bindings: Vec<usize>,
    resources: Vec<usize>,
}

fn binding_cmp(
    left: &EnvironmentBinding,
    namespace: &NamespaceRef,
    name: &str,
    b: &mut Budget,
) -> Result<Ordering, SyntaxError> {
    b.charge(
        Resource::Work,
        (left.name.len() as u64)
            .saturating_add(name.len() as u64)
            .saturating_add(left.namespace.name.len() as u64)
            .saturating_add(namespace.name.len() as u64)
            .saturating_add(left.namespace.schema.package.len() as u64)
            .saturating_add(namespace.schema.package.len() as u64)
            .saturating_add(40),
    )?;
    let a = &left.namespace.schema;
    let z = &namespace.schema;
    Ok(a.package
        .cmp(&z.package)
        .then_with(|| a.revision.cmp(&z.revision))
        .then_with(|| a.digest.0.cmp(&z.digest.0))
        .then_with(|| left.namespace.name.cmp(&namespace.name))
        .then_with(|| left.name.as_str().cmp(name)))
}

fn resource_cmp(left: &str, right: &str, b: &mut Budget) -> Result<Ordering, SyntaxError> {
    b.charge(
        Resource::Work,
        (left.len() as u64)
            .saturating_add(right.len() as u64)
            .saturating_add(1),
    )?;
    Ok(left.cmp(right))
}

// Index construction leaves the authored values untouched. Every comparison,
// swap and allocation is charged; heapsort permits stopping inside the sort.
fn sorted(
    count: usize,
    compare: impl Fn(usize, usize, &mut Budget) -> Result<Ordering, SyntaxError>,
    b: &mut Budget,
) -> Result<Vec<usize>, SyntaxError> {
    b.poll()?;
    let bytes = count
        .checked_mul(core::mem::size_of::<usize>())
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::Work, count as u64)?;
    values.extend(0..count);
    fn sift(
        values: &mut [usize],
        mut root: usize,
        compare: &impl Fn(usize, usize, &mut Budget) -> Result<Ordering, SyntaxError>,
        b: &mut Budget,
    ) -> Result<(), SyntaxError> {
        while root < values.len() / 2 {
            b.charge(Resource::Work, 1)?;
            let mut child = root * 2 + 1;
            if child + 1 < values.len()
                && compare(values[child], values[child + 1], b)? == Ordering::Less
            {
                child += 1;
            }
            if compare(values[root], values[child], b)? != Ordering::Less {
                break;
            }
            b.charge(Resource::Work, 1)?;
            values.swap(root, child);
            root = child;
        }
        Ok(())
    }
    for root in (0..count / 2).rev() {
        sift(&mut values, root, &compare, b)?;
    }
    for end in (1..count).rev() {
        b.charge(Resource::Work, 1)?;
        values.swap(0, end);
        sift(&mut values[..end], 0, &compare, b)?;
    }
    Ok(values)
}

pub(in crate::syntax) fn resource_index(
    values: &[ResourceContent],
    b: &mut Budget,
) -> Result<Vec<usize>, SyntaxError> {
    let sorted = sorted(
        values.len(),
        |a, z, b| resource_cmp(&values[a].id, &values[z].id, b),
        b,
    )?;
    for pair in sorted.windows(2) {
        if resource_cmp(&values[pair[0]].id, &values[pair[1]].id, b)? == Ordering::Equal {
            return Err(SyntaxError::ResourceDigest);
        }
    }
    Ok(sorted)
}

impl EnvironmentIndex {
    pub(in crate::syntax) fn new(value: &Environment, b: &mut Budget) -> Result<Self, SyntaxError> {
        let bindings = sorted(
            value.bindings.len(),
            |a, z, b| {
                let other = &value.bindings[z];
                binding_cmp(&value.bindings[a], &other.namespace, &other.name, b)
            },
            b,
        )?;
        for pair in bindings.windows(2) {
            let other = &value.bindings[pair[1]];
            if binding_cmp(&value.bindings[pair[0]], &other.namespace, &other.name, b)?
                == Ordering::Equal
            {
                return Err(SyntaxError::Environment);
            }
        }
        Ok(Self {
            bindings,
            resources: resource_index(&value.resources, b)?,
        })
    }
    pub(super) fn binding<'a>(
        &self,
        value: &'a Environment,
        namespace: &NamespaceRef,
        name: &str,
        b: &mut Budget,
    ) -> Result<Option<&'a EnvironmentBinding>, SyntaxError> {
        search(
            &self.bindings,
            |id, b| binding_cmp(&value.bindings[id], namespace, name, b),
            b,
        )
        .map(|id| id.map(|id| &value.bindings[id]))
    }
    pub(super) fn resource<'a>(
        &self,
        value: &'a Environment,
        id: &str,
        b: &mut Budget,
    ) -> Result<Option<&'a ResourceContent>, SyntaxError> {
        search(
            &self.resources,
            |index, b| resource_cmp(&value.resources[index].id, id, b),
            b,
        )
        .map(|index| index.map(|index| &value.resources[index]))
    }
}

fn search(
    index: &[usize],
    compare: impl Fn(usize, &mut Budget) -> Result<Ordering, SyntaxError>,
    b: &mut Budget,
) -> Result<Option<usize>, SyntaxError> {
    b.poll()?;
    let (mut lo, mut hi) = (0, index.len());
    while lo < hi {
        b.charge(Resource::Work, 1)?;
        let mid = lo + (hi - lo) / 2;
        match compare(index[mid], b)? {
            Ordering::Less => lo = mid + 1,
            Ordering::Greater => hi = mid,
            Ordering::Equal => return Ok(Some(index[mid])),
        }
    }
    Ok(None)
}
