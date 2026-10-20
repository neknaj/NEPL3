//! Derive root/guest site interleaving from the retained constructor sequence.
use super::*;
pub(super) struct Order {
    pub(super) definitions: Vec<labels::DefinitionIndex>,
    pub(super) references: Vec<labels::ReferenceIndex>,
}
pub(super) fn build<E>(
    input: &Selection<'_>,
    members: &[labels::Member<'_>],
    ranges: &[Range],
    b: &mut Budget,
) -> Result<Order, Error<E>> {
    let mut definitions_at = Vec::new();
    let mut references_at = Vec::new();
    let mut visited = Vec::new();
    for _ in &input.document().value.nodes {
        push(&mut definitions_at, None, b)?;
        push(&mut references_at, None, b)?;
        push(&mut visited, false, b)?;
    }
    for (index, site) in members[0].definitions().iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        definitions_at[site.node as usize] = Some(index);
    }
    for (index, site) in members[0].references().iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        references_at[site.node as usize] = Some(index);
    }
    let mut definitions = Vec::new();
    let mut references = Vec::new();
    let mut next = 0;
    for (position, occurrence) in input.structure().iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let node = occurrence.node as usize;
        if !visited[node] {
            visited[node] = true;
            if let Some(index) = definitions_at[node] {
                push(
                    &mut definitions,
                    labels::DefinitionIndex {
                        member: labels::MemberId(0),
                        index: index as u64,
                    },
                    b,
                )?;
            }
            if let Some(index) = references_at[node] {
                push(
                    &mut references,
                    labels::ReferenceIndex {
                        member: labels::MemberId(0),
                        index: index as u64,
                    },
                    b,
                )?;
            }
        }
        if let Some(range) = ranges.get(next)
            && range.occurrence == position
        {
            for member in range.start..range.end {
                let id = labels::MemberId(member as u64 + 1);
                for index in 0..members[member + 1].definitions().len() {
                    push(
                        &mut definitions,
                        labels::DefinitionIndex {
                            member: id,
                            index: index as u64,
                        },
                        b,
                    )?;
                }
                for index in 0..members[member + 1].references().len() {
                    push(
                        &mut references,
                        labels::ReferenceIndex {
                            member: id,
                            index: index as u64,
                        },
                        b,
                    )?;
                }
            }
            next += 1;
        }
    }
    if next != ranges.len() {
        return Err(Error::InternalShape);
    }
    Ok(Order {
        definitions,
        references,
    })
}
