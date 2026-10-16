//! Concrete optional archive capability for versioned language consumers.
use super::*;
use nepl3_core::value_codec::{
    GroupEncodingError,
    archive::{FoundationGroupArchiveCodec, GroupArchiveEncoding},
};
fn selected<'a>(
    registry: &'a SchemaRegistry,
    package: &str,
    b: &mut Budget,
) -> Result<&'a SchemaRef, WireError> {
    registry
        .selected_descriptor_with_budget(package, 1, b)?
        .map(|(reference, _)| reference)
        .ok_or_else(|| SchemaError::UnknownSchema.into())
}
impl FoundationGroupArchiveCodec for FoundationCodec<'_> {
    fn encode_compact_group_at_depths(
        &mut self,
        group: &nepl3_core::syntax::group::ForeignClosureGroup,
        depths: &[u64],
        native_registry: &SchemaRegistry,
        b: &mut Budget,
    ) -> Result<GroupArchiveEncoding, GroupEncodingError<WireError>> {
        let source = selected(self.registry, "nepl3.source-archive", b)
            .map_err(GroupEncodingError::Codec)?;
        let archive =
            selected(self.registry, "nepl3.group-archive", b).map_err(GroupEncodingError::Codec)?;
        let (archive, entry_digests) = crate::syntax::group::compact::encode_at_depths(
            crate::syntax::group::compact::GroupAtDepths {
                group,
                depths,
                native_registry,
            },
            archive,
            source,
            self.schema,
            self.registry,
            self.admission,
            b,
        )?;
        Ok(GroupArchiveEncoding {
            archive,
            entry_digests,
        })
    }
    fn decode_group_archive(
        &mut self,
        bytes: &[u8],
        b: &mut Budget,
    ) -> Result<nepl3_core::syntax::group::ForeignClosureGroup, WireError> {
        let source = selected(self.registry, "nepl3.source-archive", b)?;
        let archive = selected(self.registry, "nepl3.group-archive", b)?;
        crate::syntax::group::archive::decode(
            bytes,
            archive,
            source,
            self.schema,
            self.registry,
            self.admission,
            b,
        )
    }
}
