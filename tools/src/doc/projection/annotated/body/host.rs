//! Explicit Body-only file adapter. Output is a Markdown fragment, not a page.
use super::*;
use crate::doc::{
    export::pages::resources::{ExecutionLimits, PhaseLimits},
    source::{Compiled, err, with_named_input_limits},
};
use nepl3_core::source::{SourceAdmission, SourceStore};
use nepl3_doc_core::check::Category;
use nepl3_wire::foundation::FoundationCodec;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Parse/lower/prepare/render with separate explicit phase budgets. No aliases,
/// metadata, source links or final newline are added to the native artifact.
pub fn from_source(
    compiled: &Compiled,
    source: &str,
    limits: ExecutionLimits,
) -> Result<BodyArtifact, String> {
    from_source_profile(
        compiled,
        source,
        limits.phases(),
        &mut limits.output_limits.budget(),
    )
}
fn from_source_profile(
    compiled: &Compiled,
    source: &str,
    phases: PhaseLimits,
    output: &mut Budget,
) -> Result<BodyArtifact, String> {
    output.poll().map_err(err)?;
    if source.len() as u64 > crate::doc::export::MAX_SOURCE_BYTES.min(phases.parse.source_bytes) {
        return Err("SourceLimit".into());
    }
    with_named_input_limits(
        true,
        compiled,
        source,
        "doc-input",
        "Body",
        phases.parse,
        |tree, profile, _, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let document = nepl3_suite::adapters::sentence::literal::lower(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Body,
                profile.registry(),
                &mut Budget::new(phases.lower),
                &mut codec,
            )
            .map_err(err)?;
            render_footnotes(&document, profile.registry(), &mut codec, output).map_err(err)
        },
    )
}

// Reject existing special files before opening; opened metadata is checked too.
// This is not a promise of protection against concurrent hostile path replacement.
fn read_regular(path: &Path, ceiling: u64) -> crate::Result<String> {
    if !fs::metadata(path)?.is_file() {
        return Err("input must be a regular file".into());
    }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err("opened input must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(ceiling + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > ceiling {
        return Err("input exceeds byte limit".into());
    }
    Ok(String::from_utf8(bytes)?)
}

pub fn read_limits(path: &Path) -> crate::Result<ExecutionLimits> {
    let text = read_regular(path, 65_536)?;
    crate::repository::json::validate(&text)?;
    Ok(serde_json::from_str(&text)?)
}

fn validate_output(path: &Path) -> std::io::Result<()> {
    // Windows stream suffixes such as ::$DATA can alias an ordinary file.
    // Reject colons in normal components; a Windows drive prefix is separate.
    for part in path.components() {
        if let std::path::Component::Normal(name) = part
            && name.as_encoded_bytes().contains(&b':')
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "stream-qualified output components are unsupported",
            ));
        }
    }
    Ok(())
}
fn absent(path: &Path) -> std::io::Result<()> {
    validate_output(path)?;
    match fs::symlink_metadata(path) {
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "output already exists",
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Generate fully before touching the output. A complete sibling staging file
/// is atomically hard-linked without replacing any destination. Filesystems
/// without hard-link support fail; there is no unsafe rename/copy fallback.
/// This guarantees no partial final file, not power-loss durability or hostile
/// filesystem race protection. Cleanup failure after publication reports that
/// the complete output was published, rather than pretending it is absent.
pub fn write(input: &Path, output: &Path, limits: ExecutionLimits) -> crate::Result<()> {
    absent(output)?;
    let ceiling = crate::doc::export::MAX_SOURCE_BYTES.min(limits.parse_limits.source_bytes);
    let source = read_regular(input, ceiling)?;
    let artifact = from_source(&crate::doc::source::compiled()?, &source, limits)?;
    publish(output, |file| file.write_all(artifact.markdown.as_bytes()))
}

static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);
struct Stage {
    path: PathBuf,
    file: File,
}
impl Stage {
    // ASCII 8.3 spelling avoids creation of a distinct generated DOS alias.
    // Case/trailing-dot-space spellings are still excluded against output.
    fn name(serial: u64) -> String {
        let token = serial.wrapping_add(u64::from(std::process::id()).wrapping_mul(0x9e37_79b1))
            & 0x0fff_ffff;
        format!("n{token:07x}.tmp")
    }
    fn create(output: &Path) -> std::io::Result<Self> {
        Self::create_with_serial(output, || NEXT_STAGE.fetch_add(1, Ordering::Relaxed))
    }
    fn create_with_serial(output: &Path, mut next: impl FnMut() -> u64) -> std::io::Result<Self> {
        validate_output(output)?;
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        for _ in 0..64 {
            let serial = next();
            let name = Self::name(serial);
            // A caller may choose our staging spelling as the destination.
            // Also conservatively exclude ASCII case/trailing-dot variants.
            if output
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.trim_end_matches([' ', '.']).eq_ignore_ascii_case(&name))
            {
                continue;
            }
            let path = parent.join(name);
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => return Ok(Self { path, file }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "no free Body staging name",
        ))
    }
}
fn publish(
    output: &Path,
    write: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> crate::Result<()> {
    publish_with_cleanup(output, write, |path| fs::remove_file(path))
}
fn publish_with_cleanup(
    output: &Path,
    write: impl FnOnce(&mut File) -> std::io::Result<()>,
    cleanup: impl FnOnce(&Path) -> std::io::Result<()>,
) -> crate::Result<()> {
    absent(output)?;
    let mut stage = Stage::create(output)?;
    let result = (|| {
        write(&mut stage.file)?;
        stage.file.flush()?;
        fs::hard_link(&stage.path, output)
    })();
    // Close before unlink, including on Windows.
    drop(stage.file);
    let cleanup = cleanup(&stage.path);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error.into()),
        (Ok(()), Err(error)) => {
            Err(format!("complete output published; staging cleanup failed: {error}").into())
        }
        (Err(error), Err(cleanup)) => {
            Err(format!("publication failed: {error}; staging cleanup failed: {cleanup}").into())
        }
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests;
