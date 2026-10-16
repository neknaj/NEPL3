//! Explicit optional grouped archive boundary; no v1 codec silently opts in.
use super::*;
use crate::{schema::SchemaRegistry, syntax::group::ForeignClosureGroup};

/// One checked GroupArchive/1 transport and its ordered v2 entry commitments.
/// Bytes or digests received from elsewhere do not constitute validation proof.
pub struct GroupArchiveEncoding {
    pub archive: Vec<u8>,
    pub entry_digests: Vec<Digest>,
}

/// Optional host capability for versioned consumers. Ordinary FoundationValueCodec
/// implementations remain valid without it; there is no success-shaped fallback.
pub trait FoundationGroupArchiveCodec: FoundationValueCodec {
    /// Enforce the native consuming-language entry depths and both registry
    /// authorities before producing archive bytes and compact component identities.
    /// Use the current codec admission and the supplied cumulative Budget throughout.
    fn encode_compact_group_at_depths(
        &mut self,
        group: &ForeignClosureGroup,
        depths: &[u64],
        native_registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<GroupArchiveEncoding, GroupEncodingError<Self::Error>>;
    /// Restore every local source closure and validate the native group. This
    /// does not prove consuming-language occurrence depths or validate a Doc plan.
    fn decode_group_archive(
        &mut self,
        archive: &[u8],
        budget: &mut Budget,
    ) -> Result<ForeignClosureGroup, Self::Error>;
}
