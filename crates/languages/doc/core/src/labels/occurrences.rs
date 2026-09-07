use super::*;
#[derive(Clone, Copy)]
struct Link {
    owner: usize,
    occurrence: usize,
    child: usize,
}
pub(super) fn check<'a>(
    doc: &'a DocumentSyntax,
    order: &[usize],
    root: ArticleRef,
    b: &mut Budget,
) -> Result<(), LabelError<'a>> {
    let count = doc.value.nodes.len();
    b.charge(
        Resource::AllocationUnits,
        (count as u64).saturating_mul(1 + core::mem::size_of::<[Option<Link>; 2]>() as u64),
    )?;
    let mut counts = vec![0u8; count];
    let mut links = vec![[None; 2]; count];
    counts[root.0 as usize] = 1;
    // Reverse postorder is topological even for a shared descendant. Counts
    // saturate at two; no exponentially large display expansion is built.
    for owner in order.iter().rev().copied() {
        b.charge(Resource::Work, 1)?;
        let node = &doc.value.nodes[owner];
        if counts[owner] > 1 {
            let name = match &node.kind {
                DocKind::Section { id, .. } | DocKind::Anchor { id, .. } => Some(id.as_str()),
                _ => None,
            };
            if let Some(name) = name {
                return Err(LabelError::DuplicateOccurrence {
                    definition: site(node, owner, name),
                    paths: LabelOccurrencePaths {
                        first: path(&links, root.0 as usize, owner, 0, b)?,
                        second: path(&links, root.0 as usize, owner, 1, b)?,
                    },
                });
            }
        }
        let mut child = 0;
        while let Some((target, _)) = edges::edge(&node.kind, child) {
            b.charge(Resource::Work, 1)?;
            let target = target as usize;
            for occurrence in 0..counts[owner] as usize {
                b.charge(Resource::Work, 1)?;
                if counts[target] < 2 {
                    links[target][counts[target] as usize] = Some(Link {
                        owner,
                        occurrence,
                        child,
                    });
                    counts[target] += 1;
                }
            }
            child += 1;
        }
    }
    Ok(())
}
fn path<'a>(
    links: &[[Option<Link>; 2]],
    root: usize,
    mut target: usize,
    mut occurrence: usize,
    b: &mut Budget,
) -> Result<Vec<DocPathStep>, LabelError<'a>> {
    let mut result = Vec::new();
    while target != root {
        b.charge(Resource::Work, 1)?;
        // The checked DAG and propagation above guarantee an incoming edge.
        // Keep this boundary fallible if a future internal caller changes it.
        let Some(link) = links[target][occurrence] else {
            return Err(
                StructureError::Shape(crate::check::ShapeError::Reference(target as u64)).into(),
            );
        };
        push(
            &mut result,
            DocPathStep {
                owner: link.owner as u64,
                child: link.child as u64,
                target: target as u64,
            },
            b,
        )?;
        target = link.owner;
        occurrence = link.occurrence;
    }
    b.charge(Resource::Work, result.len() as u64)?;
    result.reverse();
    Ok(result)
}
