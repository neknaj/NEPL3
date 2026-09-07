use crate::model::*;
use nepl3_core::budget::{Budget, Resource, StopReason};
impl DocumentSyntax {
    /// Storage/work accounting only; operation owners separately admit sources
    /// and check structure. Recursive Doc ownership is represented by arena IDs.
    pub fn clone_with_budget(&self, b: &mut Budget) -> Result<Self, StopReason> {
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Self>() as u64,
        )?;
        for node in &self.value.nodes {
            b.charge(Resource::Work, 1)?;
            let mut bytes = kind_bytes(&node.kind);
            for location in &node.locations {
                b.charge(Resource::Work, 1)?;
                bytes = bytes
                    .saturating_add(core::mem::size_of::<DocFieldLocation>() as u64)
                    .saturating_add(
                        location
                            .span
                            .as_ref()
                            .map_or(0, |s| s.snapshot_ref().source.0.len() as u64),
                    );
            }
            if let Some(span) = &node.span {
                bytes = bytes.saturating_add(span.snapshot_ref().source.0.len() as u64);
            }
            b.charge(Resource::Work, bytes)?;
            b.charge(
                Resource::AllocationUnits,
                bytes.saturating_add(core::mem::size_of::<DocNode>() as u64),
            )?;
        }
        for embed in &self.value.embeds {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<DocEmbed>() as u64,
            )?;
            embed.closure.charge_clone(b)?;
        }
        for source in &self.sources {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of_val(source) as u64,
            )?;
            source.charge_clone(b)?;
        }
        for origin in &self.origins {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of_val(origin) as u64,
            )?;
            origin.charge_clone(b)?;
        }
        for view in &self.views {
            b.charge(
                Resource::Work,
                view.head.snapshot_ref().source.0.len() as u64,
            )?;
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<DocView>() as u64
                    + view.head.snapshot_ref().source.0.len() as u64,
            )?;
            view.view.charge_clone(b)?;
        }
        for map in &self.source_maps {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of_val(map) as u64,
            )?;
            map.charge_clone(b)?;
        }
        Ok(self.clone())
    }
}
pub(crate) fn kind_bytes(kind: &DocKind) -> u64 {
    use DocKind::*;
    match kind {
        Article { language, .. } | Variant { language, .. } => language.len() as u64,
        Body { blocks } => blocks.len() as u64 * 8,
        Paragraph { items } => items.len() as u64 * 8,
        Section { id, .. } | Anchor { id, .. } => id.len() as u64,
        Sentence { inlines } | Concat { inlines } => inlines.len() as u64 * 8,
        Parallel { variants } => variants.len() as u64 * 8,
        Text { text } | InlineCode { text } => text.len() as u64,
        Anno { notes, .. } => notes.len() as u64 * 8,
        Reference { target, .. } => target.len() as u64,
        Table { columns, rows, .. } => (columns.len() as u64)
            .saturating_mul(core::mem::size_of::<crate::model::Alignment>() as u64)
            .saturating_add(rows.len() as u64 * 8),
        Row { cells } => cells.len() as u64 * 8,
        List { items, .. } => items.len() as u64 * 8,
        Link { target, .. } | Target { target } => match target {
            LinkTarget::Page { page, fragment } => {
                page.len() as u64 + fragment.as_ref().map_or(0, |s| s.len() as u64)
            }
            LinkTarget::Relative { path, fragment } => {
                path.len() as u64 + fragment.as_ref().map_or(0, |s| s.len() as u64)
            }
            LinkTarget::External { uri } => uri.len() as u64,
        },
        RawCode {
            language_hint,
            text,
        } => text.len() as u64 + language_hint.as_ref().map_or(0, |s| s.len() as u64),
        Image { asset, .. } | InlineImage { asset, .. } | Asset { asset } => asset.id.len() as u64,
        OptionalText { text } => text.as_ref().map_or(0, |s| s.len() as u64),
        Alignment { .. }
        | ListStyle { .. }
        | Check { .. }
        | OptionalRow { .. }
        | OptionalSentence { .. } => 0,
        Ruby { .. }
        | InlineMath { .. }
        | Emphasis { .. }
        | Strong { .. }
        | Break
        | DisplayMath { .. }
        | CircuitFigure { .. }
        | Code { .. }
        | ListItem { .. } => 0,
    }
}
