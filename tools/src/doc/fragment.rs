//! Bounded UTF-8 fragment input and no-replace single-file publication.
use crate::doc::export::pages::resources::ExecutionLimits;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
// Reject existing special files before opening; opened metadata is checked too.
// This is not a promise of protection against concurrent hostile path replacement.
pub(crate) fn read_regular(path: &Path, ceiling: u64) -> crate::Result<String> {
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

pub(crate) fn read_limits(path: &Path) -> crate::Result<ExecutionLimits> {
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
pub(crate) fn absent(path: &Path) -> std::io::Result<()> {
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
pub(crate) fn publish(
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
