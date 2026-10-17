//! Optional identity-only capability, independent of archive transport selection.
use super::*;
use nepl3_core::value_codec::{
    GroupEncodingError,
    commitment::{FoundationGroupCommitmentCodec, GroupCommitments},
};
impl FoundationGroupCommitmentCodec for FoundationCodec<'_> {
    fn compact_group_commitments_at_depths(
        &mut self,
        group: &nepl3_core::syntax::group::ForeignClosureGroup,
        depths: &[u64],
        native_registry: &SchemaRegistry,
        b: &mut Budget,
    ) -> Result<GroupCommitments, GroupEncodingError<WireError>> {
        crate::syntax::group::compact::commit_at_depths(
            crate::syntax::group::compact::GroupAtDepths {
                group,
                depths,
                native_registry,
            },
            self.schema,
            self.registry,
            self.admission,
            b,
        )
    }
}
