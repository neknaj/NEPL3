//! Host-selected semantic content for foreign Inline operands. No guest runs
//! implicitly, and no visibility is inferred from serialized HTML or text.
use super::*;

#[derive(Debug, Eq, PartialEq)]
pub enum ContentError<E> {
    Sentence(Error),
    Foreign(E),
}
impl<E> From<Error> for ContentError<E> {
    fn from(error: Error) -> Self {
        Self::Sentence(error)
    }
}
impl<E> From<StopReason> for ContentError<E> {
    fn from(reason: StopReason) -> Self {
        Self::Sentence(Error::Stopped(reason))
    }
}
pub(super) fn annotation(
    value: &SentenceValue,
    node: usize,
    visible: &[bool],
    b: &mut Budget,
) -> Result<(), Error> {
    let require = |child: u64| {
        let index = reference(value, child, Category::Inline)?;
        if !visible[index] {
            return Err(Error::EmptyAnnotationPart(child));
        }
        Ok(())
    };
    match &value.nodes[node] {
        Kind::Ruby { base, reading } => {
            require(base.0)?;
            require(reading.0)?;
        }
        Kind::InlineAnno { base, notes } => {
            if notes.is_empty() {
                return Err(Error::AnnotationNotes(node as u64));
            }
            require(base.0)?;
            for note in notes {
                b.charge(Resource::Work, 1)?;
                require(note.0)?;
            }
        }
        _ => {}
    }
    Ok(())
}
pub(super) fn visible(
    kind: &Kind,
    any: bool,
    foreign: bool,
    b: &mut Budget,
) -> Result<bool, Error> {
    Ok(match kind {
        Kind::Text { text } | Kind::Code { text } => {
            b.charge(Resource::Work, text.len() as u64)?;
            !text.is_empty()
        }
        Kind::Break => true,
        Kind::ForeignInline { .. } => foreign,
        Kind::ExternalLink { uri, .. } => {
            b.charge(Resource::Work, uri.len() as u64)?;
            any
        }
        _ => any,
    })
}
impl CheckedShape<'_> {
    /// Recheck Ruby/Anno content using the explicitly selected host's foreign
    /// classifications. Each embed callback runs once at its deepest occurrence.
    /// Empty guests remain valid outside a content-required annotation operand.
    /// The result is conditional on the host classifications, not a proof of
    /// closure validity, guest preparation, rendering or source authorization.
    pub fn content_with_foreign<E>(
        &self,
        foreign: &mut impl FnMut(EmbedRef, &mut Budget) -> Result<bool, E>,
        b: &mut Budget,
    ) -> Result<bool, ContentError<E>> {
        b.poll()?;
        let depths = self.foreign_depths(b)?;
        let count = self.value.nodes.len();
        let allocation = count
            .checked_mul(core::mem::size_of::<bool>() + core::mem::size_of::<u64>())
            .and_then(|n| n.checked_add(depths.len()))
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, allocation as u64)?;
        let mut visible = vec![false; count];
        let mut heights = vec![1u64; count];
        let mut guests = vec![false; depths.len()];
        let base = b.current_depth();
        for (i, depth) in depths.into_iter().enumerate() {
            let absolute = base
                .checked_add(depth)
                .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
            guests[i] = b.with_depth_at_least(absolute, |b| {
                b.charge(Resource::Work, 1)?;
                let result = foreign(EmbedRef(i as u64), b);
                b.poll()?;
                result.map_err(ContentError::Foreign)
            })?;
        }
        for &node in &self.order {
            b.charge(Resource::Work, 1)?;
            let mut any = false;
            let mut index = 0;
            while let Some(child) = self.value.nodes[node].child(index) {
                b.charge(Resource::Work, 1)?;
                let child = reference(self.value, child.0, Category::Inline)?;
                heights[node] = heights[node].max(heights[child].saturating_add(1));
                any |= visible[child];
                index += 1;
            }
            let depth = base
                .checked_add(heights[node])
                .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
            b.with_depth_at_least::<_, ContentError<E>>(depth, |_| Ok(()))?;
            annotation(self.value, node, &visible, b)?;
            let guest = match self.value.nodes[node] {
                Kind::ForeignInline { syntax } => guests[syntax.0 as usize],
                _ => false,
            };
            visible[node] = self::visible(&self.value.nodes[node], any, guest, b)?;
        }
        let root = match self.value.root {
            Root::Sentence(r) => r.0,
            Root::Inline(r) => r.0,
        };
        b.poll()?;
        Ok(visible[root as usize])
    }
}
