//! Name-sorted declaration indices; declaration IDs retain constructor order.
use super::*;
use core::cmp::Ordering;

fn compare(a: &str, c: &str, b: &mut Budget) -> Result<Ordering, StopReason> {
    b.charge(Resource::Work, a.len().min(c.len()) as u64 + 1)?;
    Ok(a.cmp(c))
}
fn less(a: usize, c: usize, sites: &[LabelSite<'_>], b: &mut Budget) -> Result<bool, StopReason> {
    Ok(compare(sites[a].name, sites[c].name, b)?
        .then(a.cmp(&c))
        .is_lt())
}
fn sift(
    order: &mut [usize],
    mut root: usize,
    sites: &[LabelSite<'_>],
    b: &mut Budget,
) -> Result<(), StopReason> {
    while root < order.len() / 2 {
        let mut child = root * 2 + 1;
        if child + 1 < order.len() && less(order[child], order[child + 1], sites, b)? {
            child += 1;
        }
        if !less(order[root], order[child], sites, b)? {
            break;
        }
        order.swap(root, child);
        root = child;
    }
    Ok(())
}
pub(super) fn build<'a>(
    sites: &[LabelSite<'a>],
    b: &mut Budget,
) -> Result<Vec<usize>, LabelError<'a>> {
    let mut order = Vec::new();
    for i in 0..sites.len() {
        b.charge(Resource::Work, 1)?;
        push(&mut order, i, b)?;
    }
    // In-place heapsort permits stopping at each comparison and needs no scratch
    // allocation. Ties use declaration IDs to preserve the first definition.
    for root in (0..order.len() / 2).rev() {
        sift(&mut order, root, sites, b)?;
    }
    for end in (1..order.len()).rev() {
        order.swap(0, end);
        sift(&mut order[..end], 0, sites, b)?;
    }
    let mut duplicate: Option<(usize, usize)> = None;
    for pair in order.windows(2) {
        if compare(sites[pair[0]].name, sites[pair[1]].name, b)? == Ordering::Equal
            && duplicate.is_none_or(|(_, current)| pair[1] < current)
        {
            duplicate = Some((pair[0], pair[1]));
        }
    }
    if let Some((previous, current)) = duplicate {
        return Err(LabelError::Duplicate {
            definition: sites[current],
            previous: sites[previous],
        });
    }
    Ok(order)
}
pub(super) fn find(
    order: &[usize],
    sites: &[LabelSite<'_>],
    name: &str,
    b: &mut Budget,
) -> Result<Option<DocLabelId>, StopReason> {
    let (mut lo, mut hi) = (0, order.len());
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        match compare(sites[order[mid]].name, name, b)? {
            Ordering::Less => lo = mid + 1,
            Ordering::Greater => hi = mid,
            Ordering::Equal => return Ok(Some(DocLabelId(order[mid] as u64))),
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nepl3_core::budget::Limits;
    fn budget() -> Budget {
        Budget::new(Limits {
            source_bytes: 1_000_000,
            work: 1_000_000,
            depth: 100,
            nodes: 1_000_000,
            allocation_units: 1_000_000,
            output_bytes: 1_000_000,
            diagnostics: 100,
            events: 100,
        })
    }
    fn site(node: usize, name: &str) -> LabelSite<'_> {
        LabelSite {
            node: node as u64,
            name,
            selection: None,
            range: None,
        }
    }
    #[test]
    fn index_preserves_ids_with_subquadratic_comparison_work() -> Result<(), alloc::string::String>
    {
        let mut previous = None;
        for count in [128, 256, 512] {
            let names: Vec<_> = (0..count)
                .rev()
                .map(|i| alloc::format!("name-{i:04}"))
                .collect();
            let sites: Vec<_> = names.iter().enumerate().map(|(i, n)| site(i, n)).collect();
            let mut b = budget();
            let order = build(&sites, &mut b).map_err(|e| alloc::format!("{e:?}"))?;
            for i in 0..count {
                assert_eq!(
                    find(&order, &sites, &alloc::format!("name-{i:04}"), &mut b)
                        .map_err(|e| alloc::format!("{e:?}"))?,
                    Some(DocLabelId((count - 1 - i) as u64))
                );
            }
            assert_eq!(
                find(&order, &sites, "missing", &mut b).map_err(|e| alloc::format!("{e:?}"))?,
                None
            );
            if let Some(work) = previous {
                assert!(b.usage().work < work * 3);
            }
            previous = Some(b.usage().work);
        }
        Ok(())
    }
    #[test]
    fn short_odd_even_heaps_and_rotated_input_resolve_exact_ids()
    -> Result<(), alloc::string::String> {
        for count in 0..10 {
            let mut names: Vec<_> = (0..count).map(|i| alloc::format!("名{i}")).collect();
            for rotation in 0..count.max(1) {
                if !names.is_empty() {
                    names.rotate_left(rotation);
                }
                let sites: Vec<_> = names.iter().enumerate().map(|(i, n)| site(i, n)).collect();
                let mut b = budget();
                let order = build(&sites, &mut b).map_err(|e| alloc::format!("{e:?}"))?;
                for (id, name) in names.iter().enumerate() {
                    assert_eq!(
                        find(&order, &sites, name, &mut b).map_err(|e| alloc::format!("{e:?}"))?,
                        Some(DocLabelId(id as u64))
                    );
                }
                assert_eq!(
                    find(&order, &sites, "", &mut b).map_err(|e| alloc::format!("{e:?}"))?,
                    None
                );
            }
        }
        Ok(())
    }
    #[test]
    fn duplicate_reports_first_repeated_declaration_not_sorted_first_name() {
        let sites: Vec<_> = ["z", "a", "z", "a", "z"]
            .iter()
            .enumerate()
            .map(|(i, n)| site(i, n))
            .collect();
        assert_eq!(
            build(&sites, &mut budget()),
            Err(LabelError::Duplicate {
                definition: sites[2],
                previous: sites[0]
            })
        );
        let mut b = budget();
        b.cancel();
        assert_eq!(
            build(&sites, &mut b),
            Err(LabelError::Stopped(StopReason::Cancelled))
        );
    }
}
