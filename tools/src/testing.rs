//! Isolated test files; no mutation of the checked-out project inputs.
use crate::{Result, command};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Fixture {
    root: PathBuf,
}

impl Fixture {
    pub fn new() -> Result<Self> {
        let root = std::env::temp_dir().join(format!(
            "nepl3-tools-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root)?;
        Ok(Self { root })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn write(&self, path: &str, bytes: impl AsRef<[u8]>) -> Result<()> {
        let full = self.root.join(path);
        fs::create_dir_all(full.parent().ok_or("test path has no parent")?)?;
        fs::write(full, bytes)?;
        Ok(())
    }
    pub fn json(&self, path: &str, value: &Value) -> Result<()> {
        self.write(path, serde_json::to_vec_pretty(value)?)
    }
    pub fn git(&self) -> Result<()> {
        command(&self.root, "git", &["init", "--quiet"])?;
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // The immutable root was created exclusively for this fixture above.
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!("test cleanup {}: {error}", self.root.display());
        }
    }
}
