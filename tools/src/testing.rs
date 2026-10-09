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
        Self::new_in(&std::env::temp_dir(), &NEXT)
    }
    fn new_in(directory: &Path, next: &AtomicU64) -> Result<Self> {
        // PIDs can be reused or shared by separate namespaces using the same
        // temporary directory. Only exclusive creation grants cleanup ownership.
        let mut attempts = 0;
        loop {
            let root = directory.join(format!(
                "nepl3-tools-test-{}-{}",
                std::process::id(),
                next.fetch_add(1, Ordering::Relaxed)
            ));
            attempts += 1;
            match fs::create_dir(&root) {
                Ok(()) => return Ok(Self { root }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if attempts == 128 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::AlreadyExists,
                            format!(
                                "128 fixture candidates already exist under {}",
                                directory.display()
                            ),
                        )
                        .into());
                    }
                    // Never inspect, reuse, or remove an unowned candidate.
                }
                Err(error) => return Err(error.into()),
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate(parent: &Fixture, index: u64) -> PathBuf {
        parent
            .root()
            .join(format!("nepl3-tools-test-{}-{index}", std::process::id()))
    }
    #[test]
    fn fixture_collision_preserves_existing_directory_and_file() -> Result<()> {
        for directory in [false, true] {
            let parent = Fixture::new()?;
            let occupied = candidate(&parent, 0);
            let marker = if directory {
                fs::create_dir(&occupied)?;
                occupied.join("marker")
            } else {
                occupied.clone()
            };
            fs::write(&marker, b"keep")?;
            let next = AtomicU64::new(0);
            let fixture = Fixture::new_in(parent.root(), &next)?;
            let owned = fixture.root().to_path_buf();
            assert_eq!(owned, candidate(&parent, 1));
            assert_eq!(next.load(Ordering::Relaxed), 2);
            drop(fixture);
            assert!(!owned.exists());
            assert_eq!(fs::read(&marker)?, b"keep");
        }
        Ok(())
    }
    #[test]
    fn fixture_collision_exhaustion_keeps_all_unowned_candidates() -> Result<()> {
        let parent = Fixture::new()?;
        for index in 0..128 {
            fs::write(candidate(&parent, index), b"keep")?;
        }
        let next = AtomicU64::new(0);
        let error = match Fixture::new_in(parent.root(), &next) {
            Ok(_) => return Err("occupied candidates were accepted".into()),
            Err(error) => error,
        };
        let io = error.downcast_ref::<std::io::Error>().ok_or("I/O error")?;
        assert_eq!(io.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(next.load(Ordering::Relaxed), 128);
        for index in 0..128 {
            assert_eq!(fs::read(candidate(&parent, index))?, b"keep");
        }
        Ok(())
    }
    #[test]
    fn fixture_noncollision_error_returns_without_retry() -> Result<()> {
        let parent = Fixture::new()?;
        let file = parent.root().join("file");
        fs::write(&file, b"keep")?;
        let next = AtomicU64::new(0);
        assert!(Fixture::new_in(&file, &next).is_err());
        assert_eq!(next.load(Ordering::Relaxed), 1);
        assert_eq!(fs::read(&file)?, b"keep");
        Ok(())
    }
}
