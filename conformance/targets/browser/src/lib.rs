//! A bounded scalar test ABI, not a production API or an acceptance decision.
//! u64 arguments/results cross JS as BigInt; success has its high bit clear.
use nepl3_core::{
    budget::{Budget, Limits},
    source::{LineIndex, Position, PositionEncoding, SourceError, SourceId, SourceSnapshot},
};
const FAILURE: u64 = 1 << 63;
const ADAPTER_ERROR: u64 = FAILURE | 255;
fn fixture(id: u32) -> Option<&'static str> {
    match id {
        0 => Some("\u{feff}日🙂\r\nx\ry\nz"),
        1 => Some(""),
        2 => Some("日\r\n"),
        3 => Some("a𠮷b\r\n文書"),
        _ => None,
    }
}
fn encoding(id: u32) -> Option<PositionEncoding> {
    match id {
        0 => Some(PositionEncoding::Utf8),
        1 => Some(PositionEncoding::Utf16),
        2 => Some(PositionEncoding::Utf32),
        _ => None,
    }
}
fn error(error: SourceError) -> u64 {
    FAILURE
        | match error {
            SourceError::Bounds => 1,
            SourceError::ScalarBoundary => 2,
            SourceError::LineTerminator => 3,
            SourceError::Position => 4,
            SourceError::SnapshotMismatch => 5,
            _ => 255,
        }
}
fn context(id: u32, mismatch: u32) -> Result<(SourceSnapshot, LineIndex), u64> {
    let text = fixture(id).ok_or(ADAPTER_ERROR)?;
    let mut budget = Budget::new(Limits {
        source_bytes: 100_000,
        work: 1_000_000,
        depth: 100,
        nodes: 100_000,
        allocation_units: 1_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    });
    let make = |text: &str, revision, budget: &mut Budget| {
        SourceSnapshot::new(
            SourceId("browser-fixture".into()),
            revision,
            "memory:browser-fixture".into(),
            text.as_bytes().to_vec(),
            budget,
        )
        .map_err(error)
    };
    let original = make(text, 0, &mut budget)?;
    let index = LineIndex::new(&original, &mut budget).map_err(error)?;
    let received = match mismatch {
        0 => original,
        1 => make(text, 1, &mut budget)?,
        2 => make(&format!("{text}!"), 0, &mut budget)?,
        3 => SourceSnapshot::new(
            SourceId("other-fixture".into()),
            0,
            "memory:browser-fixture".into(),
            text.as_bytes().to_vec(),
            &mut budget,
        )
        .map_err(error)?,
        _ => return Err(ADAPTER_ERROR),
    };
    Ok((received, index))
}
#[unsafe(no_mangle)]
pub extern "C" fn abi_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn fixture_count() -> u32 {
    4
}
#[unsafe(no_mangle)]
pub extern "C" fn source_length(id: u32) -> u64 {
    fixture(id).map_or(ADAPTER_ERROR, |text| text.len() as u64)
}
#[unsafe(no_mangle)]
pub extern "C" fn source_byte(id: u32, offset: u64) -> u64 {
    fixture(id)
        .and_then(|text| {
            usize::try_from(offset)
                .ok()
                .and_then(|at| text.as_bytes().get(at))
        })
        .map_or(ADAPTER_ERROR, |byte| u64::from(*byte))
}
#[unsafe(no_mangle)]
pub extern "C" fn source_lines(id: u32) -> u64 {
    match context(id, 0) {
        Ok((_, index)) => index.line_count(),
        Err(reason) => reason,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn source_position(id: u32, mismatch: u32, codec: u32, offset: u64) -> u64 {
    let Some(codec) = encoding(codec) else {
        return ADAPTER_ERROR;
    };
    let (source, index) = match context(id, mismatch) {
        Ok(value) => value,
        Err(error) => return error,
    };
    match index.position(&source, offset, codec) {
        Ok(position) if position.line <= 0x7fff_ffff && position.character <= 0xffff_ffff => {
            (position.line << 32) | position.character
        }
        Ok(_) => ADAPTER_ERROR,
        Err(reason) => error(reason),
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn source_offset(
    id: u32,
    mismatch: u32,
    codec: u32,
    line: u64,
    character: u64,
) -> u64 {
    let Some(codec) = encoding(codec) else {
        return ADAPTER_ERROR;
    };
    let (source, index) = match context(id, mismatch) {
        Ok(value) => value,
        Err(error) => return error,
    };
    match index.offset(&source, Position { line, character }, codec) {
        Ok(offset) if offset < FAILURE => offset,
        Ok(_) => ADAPTER_ERROR,
        Err(reason) => error(reason),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adapter_discriminates_results_and_rejects_unknown_inputs() {
        assert_eq!(abi_version(), 1);
        assert_eq!(fixture_count(), 4);
        assert_eq!(source_length(0), 17);
        for (offset, byte) in fixture(0).unwrap().bytes().enumerate() {
            assert_eq!(source_byte(0, offset as u64), u64::from(byte));
        }
        assert_eq!(source_byte(0, 17), ADAPTER_ERROR);
        assert_eq!(source_byte(0, u64::MAX), ADAPTER_ERROR);
        assert_eq!(source_byte(4, 0), ADAPTER_ERROR);
        assert_eq!(source_position(0, 0, 1, 10), 4);
        assert_eq!(source_offset(0, 0, 1, 0, 4), 10);
        assert_eq!(source_offset(0, 0, 1, 0, 3), FAILURE | 2);
        assert_eq!(source_position(0, 0, 0, u64::MAX), FAILURE | 1);
        for mismatch in [1, 2, 3] {
            assert_eq!(source_position(0, mismatch, 0, 0), FAILURE | 5);
            assert_eq!(source_offset(0, mismatch, 0, 0, 0), FAILURE | 5);
        }
        assert_eq!(source_length(4), ADAPTER_ERROR);
        assert_eq!(source_position(4, 0, 0, 0), ADAPTER_ERROR);
        assert_eq!(source_position(0, 4, 0, 0), ADAPTER_ERROR);
        assert_eq!(source_position(0, 0, 3, 0), ADAPTER_ERROR);
        assert_eq!(source_offset(0, 0, 3, 0, 0), ADAPTER_ERROR);
    }

    #[test]
    fn catalog_unicode_fixture_preserves_bytes_and_endpoints() {
        assert_eq!(source_length(3), 14);
        assert_eq!(source_lines(3), 2);
        for (offset, byte) in b"a\xf0\xa0\xae\xb7b\r\n\xe6\x96\x87\xe6\x9b\xb8"
            .iter()
            .enumerate()
        {
            assert_eq!(source_byte(3, offset as u64), u64::from(*byte));
        }
        assert_eq!(source_position(3, 0, 1, 1), 1);
        assert_eq!(source_position(3, 0, 1, 5), 3);
        assert_eq!(source_offset(3, 0, 1, 0, 1), 1);
        assert_eq!(source_offset(3, 0, 1, 0, 3), 5);
        assert_eq!(source_position(3, 0, 2, 5), 2);
        assert_eq!(source_offset(3, 0, 2, 0, 2), 5);
    }
}
