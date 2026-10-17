//! Optional identity-only group capability. No transport buffer is implied.
use super::*;
use crate::{schema::SchemaRegistry, syntax::group::ForeignClosureGroup};
/// Complete ordered group commitment and existing v2 entry commitments.
/// These values are data, not native validation receipts.
pub struct GroupCommitments {
    pub group_digest: Digest,
    pub entry_digests: Vec<Digest>,
}
pub trait FoundationGroupCommitmentCodec: FoundationValueCodec {
    /// Validate the exact native group, consuming occurrence depths and registry
    /// authorities using the same admission and cumulative Budget, then hash the
    /// versioned identity preimages without producing archive transport bytes.
    fn compact_group_commitments_at_depths(
        &mut self,
        group: &ForeignClosureGroup,
        depths: &[u64],
        native_registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<GroupCommitments, GroupEncodingError<Self::Error>>;
}
