//! Immutable UTF-8 snapshots, checked byte spans and editor position adapters.
mod edit;
use crate::budget::{Budget, Resource, StopReason};
use alloc::{string::String, vec::Vec};
#[cfg(target_has_atomic = "ptr")]
type SnapshotStorage = alloc::sync::Arc<SnapshotData>;
// Keep alloc-only targets without pointer atomics supported, and retain their
// Send/Sync properties. Those targets keep the original owned-copy behavior.
#[cfg(not(target_has_atomic = "ptr"))]
type SnapshotStorage = SnapshotData;
use sha2::{Digest as _, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Digest(pub [u8; 32]);
impl Digest {
    pub fn domain(domain: &[u8], bytes: &[u8]) -> Self {
        let mut hash = Sha256::new();
        hash.update(domain);
        hash.update(bytes);
        Self(hash.finalize().into())
    }
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SourceId(pub String);
/// Host-assigned identity reserved for one generated snapshot; its digest comes from the output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceReservation {
    pub source_id: SourceId,
    pub revision: u64,
    pub uri: String,
}
impl SourceReservation {
    pub fn validate(&self, budget: &mut Budget) -> Result<(), SourceError> {
        budget.charge(Resource::Work, self.uri.len() as u64 + 1)?;
        if self.source_id.0.is_empty() || !valid_locator(&self.uri) {
            return Err(SourceError::Locator);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SnapshotId {
    pub source: SourceId,
    pub revision: u64,
    pub digest: Digest,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SourceRef {
    pub source_id: SourceId,
    pub revision: u64,
    pub digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceError {
    Stopped(StopReason),
    Decode {
        valid_up_to: u64,
        error_len: Option<u64>,
    },
    Bounds,
    ScalarBoundary,
    SnapshotMismatch,
    Position,
    LineTerminator,
    IdentityConflict,
    MissingSnapshot,
    Revision,
    OverlappingEdits,
    ExpectedDigest,
    Locator,
}
impl From<StopReason> for SourceError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}

/// No public mutable byte access: identity always hashes exactly the stored UTF-8.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSnapshot {
    storage: SnapshotStorage,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct SnapshotData {
    id: SnapshotId,
    uri: String,
    text: String,
}
impl SourceSnapshot {
    pub fn identity(&self) -> &SnapshotId {
        &self.storage.id
    }
    pub fn new(
        source: SourceId,
        revision: u64,
        uri: String,
        bytes: Vec<u8>,
        budget: &mut Budget,
    ) -> Result<Self, SourceError> {
        budget.charge(Resource::SourceBytes, bytes.len() as u64)?;
        Self::from_charged(source, revision, uri, bytes, budget)
    }
    fn from_charged(
        source: SourceId,
        revision: u64,
        uri: String,
        bytes: Vec<u8>,
        budget: &mut Budget,
    ) -> Result<Self, SourceError> {
        budget.charge(Resource::Work, uri.len() as u64)?;
        if source.0.is_empty() || !valid_locator(&uri) {
            return Err(SourceError::Locator);
        }
        budget.charge(Resource::Work, bytes.len() as u64)?;
        let digest = Digest::of(&bytes);
        let text = String::from_utf8(bytes).map_err(|e| SourceError::Decode {
            valid_up_to: e.utf8_error().valid_up_to() as u64,
            error_len: e.utf8_error().error_len().map(|n| n as u64),
        })?;
        Self::from_parts(
            SnapshotId {
                source,
                revision,
                digest,
            },
            uri,
            text,
            budget,
        )
    }
    fn from_parts(
        id: SnapshotId,
        uri: String,
        text: String,
        budget: &mut Budget,
    ) -> Result<Self, SourceError> {
        #[cfg(target_has_atomic = "ptr")]
        budget.charge(
            Resource::AllocationUnits,
            (core::mem::size_of::<SnapshotData>() + 2 * core::mem::size_of::<usize>()) as u64,
        )?;
        #[cfg(not(target_has_atomic = "ptr"))]
        budget.poll()?;
        let data = SnapshotData { id, uri, text };
        #[cfg(target_has_atomic = "ptr")]
        let storage = SnapshotStorage::new(data);
        #[cfg(not(target_has_atomic = "ptr"))]
        let storage = data;
        Ok(Self { storage })
    }
    pub fn id(&self) -> SnapshotId {
        self.storage.id.clone()
    }
    pub fn uri(&self) -> &str {
        &self.storage.uri
    }
    pub fn text(&self) -> &str {
        &self.storage.text
    }
    /// Conservative copy/comparison traversal. Pointer-atomic targets share the
    /// entire immutable snapshot, but external comparison still needs a bound
    /// covering separately decoded metadata and text.
    pub fn charge_clone(&self, budget: &mut Budget) -> Result<(), StopReason> {
        #[cfg(target_has_atomic = "ptr")]
        let bytes = 0u64;
        #[cfg(not(target_has_atomic = "ptr"))]
        let bytes = (self.storage.id.source.0.len() as u64)
            .saturating_add(self.storage.uri.len() as u64)
            .saturating_add(self.storage.text.len() as u64);
        // Existing continuation checks use this traversal before structural Eq.
        // A separately decoded snapshot can have different storage, so retain
        // a full-text comparison bound even though native cloning shares text.
        budget.charge(
            Resource::Work,
            (self.storage.id.source.0.len() as u64)
                .saturating_add(self.storage.uri.len() as u64)
                .saturating_add(self.storage.text.len() as u64)
                .saturating_add(1),
        )?;
        budget.charge(
            Resource::AllocationUnits,
            bytes.saturating_add(core::mem::size_of::<Self>() as u64),
        )
    }
    /// Charge only an actual clone. This must not be used to bound equality
    /// against independently supplied storage; charge_clone retains that bound.
    pub fn charge_shared_clone(&self, budget: &mut Budget) -> Result<(), StopReason> {
        #[cfg(target_has_atomic = "ptr")]
        let bytes = 0u64;
        #[cfg(not(target_has_atomic = "ptr"))]
        let bytes = (self.storage.id.source.0.len() as u64)
            .saturating_add(self.storage.uri.len() as u64)
            .saturating_add(self.storage.text.len() as u64);
        budget.charge(Resource::Work, bytes.saturating_add(1))?;
        budget.charge(
            Resource::AllocationUnits,
            bytes.saturating_add(core::mem::size_of::<Self>() as u64),
        )
    }
    pub fn clone_with_budget(&self, budget: &mut Budget) -> Result<Self, StopReason> {
        self.charge_shared_clone(budget)?;
        Ok(self.clone())
    }
    /// Budget structural equality. Only identical immutable storage can skip
    /// the byte comparison; independently decoded snapshots still pay for it.
    pub fn eq_with_budget(&self, other: &Self, budget: &mut Budget) -> Result<bool, StopReason> {
        budget.charge(Resource::Work, 1)?;
        #[cfg(target_has_atomic = "ptr")]
        if alloc::sync::Arc::ptr_eq(&self.storage, &other.storage) {
            return Ok(true);
        }
        budget.charge(
            Resource::Work,
            (self.storage.id.source.0.len() as u64)
                .saturating_add(other.storage.id.source.0.len() as u64)
                .saturating_add(self.storage.uri.len() as u64)
                .saturating_add(other.storage.uri.len() as u64)
                .saturating_add(34),
        )?;
        if self.storage.id != other.storage.id || self.storage.uri != other.storage.uri {
            return Ok(false);
        }
        budget.charge(
            Resource::Work,
            (self.storage.text.len() as u64).saturating_add(other.storage.text.len() as u64),
        )?;
        Ok(self.storage.text == other.storage.text)
    }
    pub fn has_bom(&self) -> bool {
        self.storage.text.starts_with('\u{feff}')
    }
    pub fn reference(&self) -> SourceRef {
        SourceRef {
            source_id: self.storage.id.source.clone(),
            revision: self.storage.id.revision,
            digest: self.storage.id.digest,
        }
    }
    pub fn check_range(&self, start: u64, end: u64) -> Result<(), SourceError> {
        if start > end || end > self.storage.text.len() as u64 {
            return Err(SourceError::Bounds);
        }
        if !self.storage.text.is_char_boundary(start as usize)
            || !self.storage.text.is_char_boundary(end as usize)
        {
            return Err(SourceError::ScalarBoundary);
        }
        Ok(())
    }
    pub fn slice_range(&self, start: u64, end: u64) -> Result<&str, SourceError> {
        self.check_range(start, end)?;
        self.storage
            .text
            .get(start as usize..end as usize)
            .ok_or(SourceError::Bounds)
    }
    pub fn span_with_budget(
        &self,
        start: u64,
        end: u64,
        budget: &mut Budget,
    ) -> Result<Span, SourceError> {
        self.check_range(start, end)?;
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Span>() as u64 + self.storage.id.source.0.len() as u64,
        )?;
        self.span(start, end)
    }
    pub fn span(&self, start: u64, end: u64) -> Result<Span, SourceError> {
        self.check_range(start, end)?;
        Ok(Span {
            snapshot: self.storage.id.clone(),
            start,
            end,
        })
    }
    pub fn slice(&self, span: &Span) -> Result<&str, SourceError> {
        if span.snapshot != self.storage.id {
            return Err(SourceError::SnapshotMismatch);
        }
        self.storage
            .text
            .get(span.start as usize..span.end as usize)
            .ok_or(SourceError::Bounds)
    }
}
fn valid_locator(uri: &str) -> bool {
    let Some((scheme, rest)) = uri.split_once(':') else {
        return false;
    };
    if scheme.is_empty()
        || rest.is_empty()
        || !scheme.as_bytes()[0].is_ascii_alphabetic()
        || !scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
    {
        return false;
    }
    if uri.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    let bytes = rest.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            if !bytes.get(at + 1).is_some_and(u8::is_ascii_hexdigit)
                || !bytes.get(at + 2).is_some_and(u8::is_ascii_hexdigit)
            {
                return false;
            }
            at += 3;
        } else {
            at += 1;
        }
    }
    true
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Span {
    snapshot: SnapshotId,
    start: u64,
    end: u64,
}
impl Span {
    pub fn snapshot_ref(&self) -> &SnapshotId {
        &self.snapshot
    }
    pub fn snapshot(&self) -> SnapshotId {
        self.snapshot.clone()
    }
    pub fn start(&self) -> u64 {
        self.start
    }
    pub fn end(&self) -> u64 {
        self.end
    }
    pub fn contains(&self, other: &Self) -> bool {
        self.snapshot == other.snapshot && self.start <= other.start && other.end <= self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionEncoding {
    Utf8,
    Utf16,
    Utf32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position {
    pub line: u64,
    pub character: u64,
}
#[derive(Clone, Copy, Debug)]
struct Line {
    start: usize,
    end: usize,
}
/// CRLF, CR and LF each end one line. Terminator interiors have no editor position.
#[derive(Debug)]
pub struct LineIndex {
    snapshot: SnapshotId,
    lines: Vec<Line>,
}
impl LineIndex {
    pub fn new(source: &SourceSnapshot, budget: &mut Budget) -> Result<Self, SourceError> {
        budget.charge(Resource::Work, source.storage.text.len() as u64)?;
        let mut lines = Vec::new();
        let mut start = 0;
        let mut cursor = 0;
        let bytes = source.storage.text.as_bytes();
        while cursor < bytes.len() {
            if bytes[cursor] == b'\r' || bytes[cursor] == b'\n' {
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<Line>() as u64,
                )?;
                lines.push(Line { start, end: cursor });
                cursor += if bytes[cursor] == b'\r' && bytes.get(cursor + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
                start = cursor;
            } else {
                cursor += 1;
            }
        }
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Line>() as u64,
        )?;
        lines.push(Line {
            start,
            end: bytes.len(),
        });
        Ok(Self {
            snapshot: source.storage.id.clone(),
            lines,
        })
    }
    pub fn line_count(&self) -> u64 {
        self.lines.len() as u64
    }
    pub fn position(
        &self,
        source: &SourceSnapshot,
        offset: u64,
        encoding: PositionEncoding,
    ) -> Result<Position, SourceError> {
        self.verify(source)?;
        source.span(offset, offset)?;
        let offset = offset as usize;
        let index = self
            .lines
            .partition_point(|line| line.start <= offset)
            .saturating_sub(1);
        let line = self.lines.get(index).ok_or(SourceError::Position)?;
        if offset > line.end {
            return Err(SourceError::LineTerminator);
        }
        let text = source
            .storage
            .text
            .get(line.start..offset)
            .ok_or(SourceError::ScalarBoundary)?;
        Ok(Position {
            line: index as u64,
            character: units(text, encoding),
        })
    }
    pub fn offset(
        &self,
        source: &SourceSnapshot,
        position: Position,
        encoding: PositionEncoding,
    ) -> Result<u64, SourceError> {
        self.verify(source)?;
        let index = usize::try_from(position.line).map_err(|_| SourceError::Position)?;
        let line = self.lines.get(index).ok_or(SourceError::Position)?;
        let text = source
            .storage
            .text
            .get(line.start..line.end)
            .ok_or(SourceError::Bounds)?;
        let mut character = 0;
        for (byte, scalar) in text.char_indices() {
            if character == position.character {
                return Ok((line.start + byte) as u64);
            }
            character += match encoding {
                PositionEncoding::Utf8 => scalar.len_utf8() as u64,
                PositionEncoding::Utf16 => scalar.len_utf16() as u64,
                PositionEncoding::Utf32 => 1,
            };
            if character > position.character {
                return Err(SourceError::ScalarBoundary);
            }
        }
        if character == position.character {
            Ok(line.end as u64)
        } else {
            Err(SourceError::Position)
        }
    }
    fn verify(&self, source: &SourceSnapshot) -> Result<(), SourceError> {
        if self.snapshot == source.storage.id {
            Ok(())
        } else {
            Err(SourceError::SnapshotMismatch)
        }
    }
}
fn units(text: &str, encoding: PositionEncoding) -> u64 {
    match encoding {
        PositionEncoding::Utf8 => text.len() as u64,
        PositionEncoding::Utf16 => text.encode_utf16().count() as u64,
        PositionEncoding::Utf32 => text.chars().count() as u64,
    }
}

/// One operation's admitted snapshots. Share this alongside the Budget across guest calls.
#[derive(Debug, Default)]
pub struct SourceAdmission {
    admitted: Vec<(SnapshotId, String)>,
    index: Vec<usize>,
    #[cfg(target_has_atomic = "ptr")]
    shared: Vec<SnapshotStorage>,
}
impl SourceAdmission {
    /// Reconstructs a repeated declaration from a nested wire bundle without charging
    /// its source bytes twice. UTF-8, locator, digest and work are still checked.
    pub fn import(
        &mut self,
        source: SourceId,
        revision: u64,
        uri: String,
        bytes: Vec<u8>,
        budget: &mut Budget,
    ) -> Result<SourceSnapshot, SourceError> {
        budget.poll()?;
        if self.find(&source, revision, budget)?.is_ok() {
            let snapshot = SourceSnapshot::from_charged(source, revision, uri, bytes, budget)?;
            self.admit_existing(&snapshot, budget)?;
            Ok(snapshot)
        } else {
            self.create(source, revision, uri, bytes, budget)
        }
    }
    pub fn create(
        &mut self,
        source: SourceId,
        revision: u64,
        uri: String,
        bytes: Vec<u8>,
        budget: &mut Budget,
    ) -> Result<SourceSnapshot, SourceError> {
        budget.poll()?;
        if self.find(&source, revision, budget)?.is_ok() {
            return Err(SourceError::IdentityConflict);
        }
        let snapshot = SourceSnapshot::new(source, revision, uri, bytes, budget)?;
        self.remember(&snapshot, budget)?;
        Ok(snapshot)
    }
    pub fn admit_existing(
        &mut self,
        snapshot: &SourceSnapshot,
        budget: &mut Budget,
    ) -> Result<(), SourceError> {
        budget.poll()?;
        #[cfg(target_has_atomic = "ptr")]
        let insertion = {
            // Keep an owning reference: allocator address reuse cannot make a
            // new snapshot inherit an old operation's admission. This index is
            // private to this admission object, never encoded or shared across
            // operations. Independently decoded storage takes the full check.
            let key = alloc::sync::Arc::as_ptr(&snapshot.storage);
            let (mut low, mut high) = (0, self.shared.len());
            // Charge a size-derived search/shift bound, not pointer ordering:
            // allocator placement must not change logical operation usage.
            budget.charge(Resource::Work, (usize::BITS - high.leading_zeros()) as u64)?;
            while low < high {
                let mid = low + (high - low) / 2;
                match alloc::sync::Arc::as_ptr(&self.shared[mid]).cmp(&key) {
                    core::cmp::Ordering::Equal => return Ok(()),
                    core::cmp::Ordering::Less => low = mid + 1,
                    core::cmp::Ordering::Greater => high = mid,
                }
            }
            budget.charge(Resource::Work, self.shared.len() as u64 + 1)?;
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<SnapshotStorage>() as u64,
            )?;
            low
        };
        self.admit_parts(
            &snapshot.storage.id.source,
            snapshot.storage.id.revision,
            snapshot.storage.id.digest,
            &snapshot.storage.uri,
            snapshot.storage.text.len() as u64,
            budget,
        )?;
        #[cfg(target_has_atomic = "ptr")]
        self.shared
            .insert(insertion, alloc::sync::Arc::clone(&snapshot.storage));
        Ok(())
    }
    fn find(
        &self,
        source: &SourceId,
        revision: u64,
        budget: &mut Budget,
    ) -> Result<Result<usize, usize>, SourceError> {
        admission_index_position(
            &self.index,
            |i| &self.admitted[i].0,
            source,
            revision,
            budget,
        )
    }
    fn prepare_insert(
        &self,
        source: &SourceId,
        revision: u64,
        budget: &mut Budget,
    ) -> Result<usize, SourceError> {
        let at = self
            .find(source, revision, budget)?
            .map_or_else(Ok, |_| Err(SourceError::IdentityConflict))?;
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<usize>() as u64,
        )?;
        budget.charge(Resource::Work, (self.index.len() - at) as u64)?;
        Ok(at)
    }
    // Used only with immutable validated snapshots or fully checked UTF-8 edit
    // output whose digest was computed from the exact fragments to be copied.
    fn admit_parts(
        &mut self,
        source: &SourceId,
        revision: u64,
        digest: Digest,
        uri: &str,
        length: u64,
        budget: &mut Budget,
    ) -> Result<(), SourceError> {
        if self.check_parts(source, revision, digest, uri, budget)? {
            return Ok(());
        }
        budget.charge(Resource::SourceBytes, length)?;
        budget.charge(
            Resource::Work,
            (source.0.len() as u64)
                .saturating_add(uri.len() as u64)
                .saturating_add(1),
        )?;
        budget.charge(
            Resource::AllocationUnits,
            (core::mem::size_of::<(SnapshotId, String)>() as u64)
                .saturating_add(source.0.len() as u64)
                .saturating_add(uri.len() as u64),
        )?;
        let at = self.prepare_insert(source, revision, budget)?;
        self.index.insert(at, self.admitted.len());
        self.admitted.push((
            SnapshotId {
                source: source.clone(),
                revision,
                digest,
            },
            uri.into(),
        ));
        Ok(())
    }
    fn check_parts(
        &self,
        source: &SourceId,
        revision: u64,
        digest: Digest,
        uri: &str,
        budget: &mut Budget,
    ) -> Result<bool, SourceError> {
        budget.poll()?;
        if let Ok(at) = self.find(source, revision, budget)? {
            let (id, locator) = &self.admitted[self.index[at]];
            budget.charge(
                Resource::Work,
                (locator.len() as u64)
                    .saturating_add(uri.len() as u64)
                    .saturating_add(34),
            )?;
            return if id.digest == digest && locator == uri {
                Ok(true)
            } else {
                Err(SourceError::IdentityConflict)
            };
        }
        Ok(false)
    }
    fn remember(
        &mut self,
        snapshot: &SourceSnapshot,
        budget: &mut Budget,
    ) -> Result<(), SourceError> {
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<(SnapshotId, String)>() as u64
                + snapshot.storage.id.source.0.len() as u64
                + snapshot.storage.uri.len() as u64,
        )?;
        budget.charge(
            Resource::Work,
            (snapshot.storage.id.source.0.len() as u64)
                .saturating_add(snapshot.storage.uri.len() as u64)
                .saturating_add(1),
        )?;
        let at = self.prepare_insert(
            &snapshot.storage.id.source,
            snapshot.storage.id.revision,
            budget,
        )?;
        self.index.insert(at, self.admitted.len());
        self.admitted
            .push((snapshot.id(), snapshot.storage.uri.clone()));
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextEdit {
    pub span: Span,
    pub expected_digest: Digest,
    pub replacement: String,
}
/// Stores immutable snapshots keyed by host-assigned portable identity; URIs are locators.
#[derive(Debug, Default)]
pub struct SourceStore {
    snapshots: Vec<SourceSnapshot>,
    // Snapshot insertion order remains public; only this private index is sorted.
    index: Vec<usize>,
    // Position of the most recent successful insertion/duplicate in `index`.
    // This is only a search hint, never an identity or validation proof.
    insertion_hint: Option<usize>,
}
impl SourceStore {
    pub fn snapshots(&self) -> &[SourceSnapshot] {
        &self.snapshots
    }
    pub fn insert(&mut self, snapshot: SourceSnapshot) -> Result<(), SourceError> {
        let position = self
            .index
            .binary_search_by(|i| source_key(&self.snapshots[*i], &snapshot));
        match position {
            Ok(at) => {
                if self.snapshots[self.index[at]] == snapshot {
                    self.insertion_hint = Some(at);
                    Ok(())
                } else {
                    Err(SourceError::IdentityConflict)
                }
            }
            Err(at) => {
                self.index.insert(at, self.snapshots.len());
                self.snapshots.push(snapshot);
                self.insertion_hint = Some(at);
                Ok(())
            }
        }
    }
    /// Insert with metered index search, duplicate comparison, and storage growth.
    pub fn insert_with_budget(
        &mut self,
        snapshot: SourceSnapshot,
        budget: &mut Budget,
    ) -> Result<(), SourceError> {
        let position = source_index_position(
            &self.index,
            |i| &self.snapshots[i],
            &snapshot,
            self.insertion_hint,
            budget,
        )?;
        match position {
            Ok(at) => {
                if self.snapshots[self.index[at]].eq_with_budget(&snapshot, budget)? {
                    self.insertion_hint = Some(at);
                    Ok(())
                } else {
                    Err(SourceError::IdentityConflict)
                }
            }
            Err(at) => {
                budget.charge(Resource::Work, (self.index.len() - at) as u64)?;
                budget.charge(
                    Resource::AllocationUnits,
                    (core::mem::size_of::<SourceSnapshot>() + core::mem::size_of::<usize>()) as u64,
                )?;
                self.index.insert(at, self.snapshots.len());
                self.snapshots.push(snapshot);
                self.insertion_hint = Some(at);
                Ok(())
            }
        }
    }
    /// Admit a borrowed snapshot to this store, cloning only a new declaration.
    /// This does not replace operation-wide SourceAdmission resource accounting.
    pub fn insert_ref_with_budget(
        &mut self,
        snapshot: &SourceSnapshot,
        budget: &mut Budget,
    ) -> Result<(), SourceError> {
        budget.poll()?;
        let position = source_index_position(
            &self.index,
            |i| &self.snapshots[i],
            snapshot,
            self.insertion_hint,
            budget,
        )?;
        match position {
            Ok(at) => {
                if self.snapshots[self.index[at]].eq_with_budget(snapshot, budget)? {
                    self.insertion_hint = Some(at);
                    Ok(())
                } else {
                    Err(SourceError::IdentityConflict)
                }
            }
            Err(at) => {
                budget.charge(Resource::Work, (self.index.len() - at) as u64)?;
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<usize>() as u64,
                )?;
                let owned = snapshot.clone_with_budget(budget)?;
                self.index.insert(at, self.snapshots.len());
                self.snapshots.push(owned);
                self.insertion_hint = Some(at);
                Ok(())
            }
        }
    }
    /// Find a host source/revision with every index comparison charged.
    pub fn get_revision_with_budget(
        &self,
        source: &SourceId,
        revision: u64,
        budget: &mut Budget,
    ) -> Result<Option<&SourceSnapshot>, StopReason> {
        budget.poll()?;
        let (mut low, mut high) = (0, self.index.len());
        while low < high {
            let mid = low + (high - low) / 2;
            let snapshot = &self.snapshots[self.index[mid]];
            budget.charge(
                Resource::Work,
                (snapshot.storage.id.source.0.len() as u64)
                    .saturating_add(source.0.len() as u64)
                    .saturating_add(1),
            )?;
            match snapshot
                .storage
                .id
                .source
                .cmp(source)
                .then_with(|| snapshot.storage.id.revision.cmp(&revision))
            {
                core::cmp::Ordering::Equal => return Ok(Some(snapshot)),
                core::cmp::Ordering::Less => low = mid + 1,
                core::cmp::Ordering::Greater => high = mid,
            }
        }
        Ok(None)
    }
    pub fn get_ref(&self, id: &SnapshotId) -> Option<&SourceSnapshot> {
        self.snapshots
            .iter()
            .find(|snapshot| snapshot.identity() == id)
    }
    pub fn get(&self, id: SnapshotId) -> Option<&SourceSnapshot> {
        self.snapshots.iter().find(|s| s.storage.id == id)
    }
    pub fn resolve(&self, reference: &SourceRef) -> Option<&SourceSnapshot> {
        self.snapshots.iter().find(|s| {
            s.storage.id.source == reference.source_id
                && s.storage.id.revision == reference.revision
                && s.storage.id.digest == reference.digest
        })
    }
    pub fn latest(&self, source: &SourceId) -> Option<&SourceSnapshot> {
        self.snapshots
            .iter()
            .filter(|s| &s.storage.id.source == source)
            .max_by_key(|s| s.storage.id.revision)
    }
}

fn source_key(a: &SourceSnapshot, b: &SourceSnapshot) -> core::cmp::Ordering {
    a.storage
        .id
        .source
        .cmp(&b.storage.id.source)
        .then_with(|| a.storage.id.revision.cmp(&b.storage.id.revision))
}
fn source_index_position<'a>(
    index: &[usize],
    get: impl Fn(usize) -> &'a SourceSnapshot,
    snapshot: &SourceSnapshot,
    hint: Option<usize>,
    budget: &mut Budget,
) -> Result<Result<usize, usize>, SourceError> {
    let (mut low, mut high) = (0, index.len());
    let mut compare = |position: usize| {
        let prior = get(index[position]);
        budget.charge(
            Resource::Work,
            (prior.storage.id.source.0.len() as u64)
                .saturating_add(snapshot.storage.id.source.0.len() as u64)
                .saturating_add(1),
        )?;
        Ok::<_, SourceError>(source_key(prior, snapshot))
    };
    if let Some(at) = hint.filter(|&at| at < index.len()) {
        match compare(at)? {
            core::cmp::Ordering::Equal => return Ok(Ok(at)),
            core::cmp::Ordering::Less => {
                low = at + 1;
                if low == high {
                    return Ok(Err(low));
                }
                match compare(low)? {
                    core::cmp::Ordering::Equal => return Ok(Ok(low)),
                    core::cmp::Ordering::Greater => return Ok(Err(low)),
                    core::cmp::Ordering::Less => low += 1,
                }
            }
            core::cmp::Ordering::Greater => {
                high = at;
                if high == 0 {
                    return Ok(Err(0));
                }
                match compare(high - 1)? {
                    core::cmp::Ordering::Equal => return Ok(Ok(high - 1)),
                    core::cmp::Ordering::Less => return Ok(Err(high)),
                    core::cmp::Ordering::Greater => high -= 1,
                }
            }
        }
    }
    while low < high {
        let mid = low + (high - low) / 2;
        match compare(mid)? {
            core::cmp::Ordering::Equal => return Ok(Ok(mid)),
            core::cmp::Ordering::Less => low = mid + 1,
            core::cmp::Ordering::Greater => high = mid,
        }
    }
    Ok(Err(low))
}

fn admission_index_position<'a>(
    index: &[usize],
    get: impl Fn(usize) -> &'a SnapshotId,
    source: &SourceId,
    revision: u64,
    budget: &mut Budget,
) -> Result<Result<usize, usize>, SourceError> {
    let (mut low, mut high) = (0, index.len());
    while low < high {
        let mid = low + (high - low) / 2;
        let id = get(index[mid]);
        budget.charge(
            Resource::Work,
            (id.source.0.len() as u64)
                .saturating_add(source.0.len() as u64)
                .saturating_add(34),
        )?;
        match id
            .source
            .cmp(source)
            .then_with(|| id.revision.cmp(&revision))
        {
            core::cmp::Ordering::Equal => return Ok(Ok(mid)),
            core::cmp::Ordering::Less => low = mid + 1,
            core::cmp::Ordering::Greater => high = mid,
        }
    }
    Ok(Err(low))
}
