//! Immutable UTF-8 snapshots, checked byte spans and editor position adapters.
use crate::budget::{Budget, Resource, StopReason};
use alloc::{string::String, vec::Vec};
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
    id: SnapshotId,
    uri: String,
    text: String,
}
impl SourceSnapshot {
    pub fn identity(&self) -> &SnapshotId {
        &self.id
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
        Ok(Self {
            id: SnapshotId {
                source,
                revision,
                digest,
            },
            uri,
            text,
        })
    }
    pub fn id(&self) -> SnapshotId {
        self.id.clone()
    }
    pub fn uri(&self) -> &str {
        &self.uri
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn has_bom(&self) -> bool {
        self.text.starts_with('\u{feff}')
    }
    pub fn reference(&self) -> SourceRef {
        SourceRef {
            source_id: self.id.source.clone(),
            revision: self.id.revision,
            digest: self.id.digest,
        }
    }
    pub fn check_range(&self, start: u64, end: u64) -> Result<(), SourceError> {
        if start > end || end > self.text.len() as u64 {
            return Err(SourceError::Bounds);
        }
        if !self.text.is_char_boundary(start as usize) || !self.text.is_char_boundary(end as usize)
        {
            return Err(SourceError::ScalarBoundary);
        }
        Ok(())
    }
    pub fn slice_range(&self, start: u64, end: u64) -> Result<&str, SourceError> {
        self.check_range(start, end)?;
        self.text
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
            core::mem::size_of::<Span>() as u64 + self.id.source.0.len() as u64,
        )?;
        self.span(start, end)
    }
    pub fn span(&self, start: u64, end: u64) -> Result<Span, SourceError> {
        self.check_range(start, end)?;
        Ok(Span {
            snapshot: self.id.clone(),
            start,
            end,
        })
    }
    pub fn slice(&self, span: &Span) -> Result<&str, SourceError> {
        if span.snapshot != self.id {
            return Err(SourceError::SnapshotMismatch);
        }
        self.text
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
        budget.charge(Resource::Work, source.text.len() as u64)?;
        let mut lines = Vec::new();
        let mut start = 0;
        let mut cursor = 0;
        let bytes = source.text.as_bytes();
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
            snapshot: source.id.clone(),
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
        if self.snapshot == source.id {
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
        if self
            .admitted
            .iter()
            .any(|(id, _)| id.source == source && id.revision == revision)
        {
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
        if self
            .admitted
            .iter()
            .any(|(id, _)| id.source == source && id.revision == revision)
        {
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
        if let Some(prior) = self
            .admitted
            .iter()
            .find(|(id, _)| id.source == snapshot.id.source && id.revision == snapshot.id.revision)
        {
            return if prior.0 == snapshot.id && prior.1 == snapshot.uri {
                Ok(())
            } else {
                Err(SourceError::IdentityConflict)
            };
        }
        budget.charge(Resource::SourceBytes, snapshot.text.len() as u64)?;
        self.remember(snapshot, budget)
    }
    fn remember(
        &mut self,
        snapshot: &SourceSnapshot,
        budget: &mut Budget,
    ) -> Result<(), SourceError> {
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<(SnapshotId, String)>() as u64
                + snapshot.id.source.0.len() as u64
                + snapshot.uri.len() as u64,
        )?;
        self.admitted.push((snapshot.id(), snapshot.uri.clone()));
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
}
impl SourceStore {
    pub fn snapshots(&self) -> &[SourceSnapshot] {
        &self.snapshots
    }
    pub fn insert(&mut self, snapshot: SourceSnapshot) -> Result<(), SourceError> {
        for prior in &self.snapshots {
            if prior.id.source == snapshot.id.source && prior.id.revision == snapshot.id.revision {
                return if prior == &snapshot {
                    Ok(())
                } else {
                    Err(SourceError::IdentityConflict)
                };
            }
        }
        self.snapshots.push(snapshot);
        Ok(())
    }
    pub fn get_ref(&self, id: &SnapshotId) -> Option<&SourceSnapshot> {
        self.snapshots
            .iter()
            .find(|snapshot| snapshot.identity() == id)
    }
    pub fn get(&self, id: SnapshotId) -> Option<&SourceSnapshot> {
        self.snapshots.iter().find(|s| s.id == id)
    }
    pub fn resolve(&self, reference: &SourceRef) -> Option<&SourceSnapshot> {
        self.snapshots.iter().find(|s| {
            s.id.source == reference.source_id
                && s.id.revision == reference.revision
                && s.id.digest == reference.digest
        })
    }
    pub fn latest(&self, source: &SourceId) -> Option<&SourceSnapshot> {
        self.snapshots
            .iter()
            .filter(|s| &s.id.source == source)
            .max_by_key(|s| s.id.revision)
    }
    /// Validates all edits before inserting any new snapshots. Each source advances exactly one revision.
    pub fn apply(
        &mut self,
        edits: &[TextEdit],
        budget: &mut Budget,
    ) -> Result<Vec<SnapshotId>, SourceError> {
        let mut sorted: Vec<&TextEdit> = edits.iter().collect();
        sorted.sort_by_key(|e| (e.span.snapshot.source.clone(), e.span.start, e.span.end));
        let mut prepared = Vec::new();
        let mut cursor = 0;
        while cursor < sorted.len() {
            let id = sorted[cursor].span.snapshot.clone();
            let source = self
                .latest(&id.source)
                .ok_or(SourceError::MissingSnapshot)?;
            if source.id != id {
                return Err(SourceError::SnapshotMismatch);
            }
            let mut next = cursor + 1;
            while next < sorted.len() && sorted[next].span.snapshot.source == id.source {
                next += 1;
            }
            let group = &sorted[cursor..next];
            let mut end = 0;
            let mut previous_start = None;
            let mut length = source.text.len() as u64;
            for edit in group {
                let expected = source.slice(&edit.span)?;
                if Digest::of(expected.as_bytes()) != edit.expected_digest {
                    return Err(SourceError::ExpectedDigest);
                }
                if edit.span.start < end || previous_start == Some(edit.span.start) {
                    return Err(SourceError::OverlappingEdits);
                }
                length = length
                    .checked_sub(edit.span.end - edit.span.start)
                    .and_then(|n| n.checked_add(edit.replacement.len() as u64))
                    .ok_or(SourceError::Stopped(StopReason::SourceLimit))?;
                end = edit.span.end;
                previous_start = Some(edit.span.start);
            }
            budget.charge(Resource::SourceBytes, length)?;
            budget.charge(Resource::AllocationUnits, length)?;
            let capacity = usize::try_from(length)
                .map_err(|_| SourceError::Stopped(StopReason::AllocationLimit))?;
            let mut output = String::with_capacity(capacity);
            end = 0;
            for edit in group {
                output.push_str(
                    source
                        .text
                        .get(end as usize..edit.span.start as usize)
                        .ok_or(SourceError::Bounds)?,
                );
                output.push_str(&edit.replacement);
                end = edit.span.end;
            }
            output.push_str(source.text.get(end as usize..).ok_or(SourceError::Bounds)?);
            let revision = id.revision.checked_add(1).ok_or(SourceError::Revision)?;
            prepared.push(SourceSnapshot::from_charged(
                id.source,
                revision,
                source.uri.clone(),
                output.into_bytes(),
                budget,
            )?);
            cursor = next;
        }
        let ids = prepared.iter().map(SourceSnapshot::id).collect();
        self.snapshots.extend(prepared);
        Ok(ids)
    }
}
