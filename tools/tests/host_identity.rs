//! Development-provider source coverage, independent of executable attestation.
#[path = "../build_support/host_identity.rs"]
mod identity;
use std::{
    error::Error,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(reverse: bool) -> Result<Self, Box<dyn Error>> {
        let path = std::env::temp_dir().join(format!(
            "nepl3-host-source-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        let value = Self(path);
        for directory in identity::ROOTS {
            fs::create_dir_all(value.0.join(directory))?;
        }
        let mut files = identity::FILES
            .iter()
            .map(|path| (*path, "manifest"))
            .collect::<Vec<_>>();
        files.extend([
            (
                "crates/foundation/reader/src/builtin/lexical.rs",
                "reader A",
            ),
            (
                "crates/languages/sentence/core/src/literal/parser.rs",
                "sentence A",
            ),
            ("languages/Sentence/syntax.neplg", "grammar A"),
            ("conformance/fixtures/doc/grammar.json", "{}"),
        ]);
        if reverse {
            files.reverse();
        }
        for (path, text) in files {
            value.write(path, text)?;
        }
        Ok(value)
    }
    fn write(&self, path: &str, content: &str) -> Result<(), Box<dyn Error>> {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().ok_or("parent")?)?;
        fs::write(path, content)?;
        Ok(())
    }
    fn digest(&self) -> Result<[u8; 32], Box<dyn Error>> {
        identity::fingerprint(&self.0)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn source_identity_covers_callbacks_data_and_input_inventory() -> Result<(), Box<dyn Error>> {
    let input = Fixture::new(false)?;
    let original = input.digest()?;
    for path in [
        "crates/foundation/reader/src/builtin/lexical.rs",
        "crates/languages/sentence/core/src/literal/parser.rs",
        "languages/Sentence/syntax.neplg",
        "conformance/fixtures/doc/grammar.json",
        "Cargo.toml",
        "Cargo.lock",
        "tools/Cargo.toml",
        "tools/build.rs",
        "rust-toolchain.toml",
    ] {
        let before = fs::read(input.0.join(path))?;
        input.write(path, "changed implementation or declared input")?;
        assert_ne!(input.digest()?, original, "{path}");
        fs::write(input.0.join(path), before)?;
        assert_eq!(input.digest()?, original);
    }
    input.write("tools/src/new_callback.rs", "new callback")?;
    let added = input.digest()?;
    assert_ne!(added, original);
    fs::rename(
        input.0.join("tools/src/new_callback.rs"),
        input.0.join("tools/src/renamed.rs"),
    )?;
    assert_ne!(input.digest()?, added);
    fs::remove_file(input.0.join("tools/src/renamed.rs"))?;
    assert_eq!(input.digest()?, original);
    Ok(())
}

#[test]
fn identity_is_location_order_and_timestamp_independent_and_framed() -> Result<(), Box<dyn Error>> {
    let a = Fixture::new(false)?;
    let b = Fixture::new(true)?;
    assert_eq!(a.digest()?, b.digest()?);
    let path = a.0.join("tools/build.rs");
    let bytes = fs::read(&path)?;
    fs::write(path, bytes)?;
    assert_eq!(a.digest()?, b.digest()?);
    a.write("tools/src/a", "bc")?;
    b.write("tools/src/ab", "c")?;
    assert_ne!(a.digest()?, b.digest()?);
    Ok(())
}

#[test]
fn missing_required_inputs_are_not_silently_omitted() -> Result<(), Box<dyn Error>> {
    for path in identity::FILES {
        let a = Fixture::new(false)?;
        fs::remove_file(a.0.join(path))?;
        assert!(a.digest().is_err());
    }
    for path in identity::ROOTS {
        let a = Fixture::new(false)?;
        fs::remove_dir_all(a.0.join(path))?;
        assert!(a.digest().is_err());
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn source_file_and_directory_symlinks_are_rejected() -> Result<(), Box<dyn Error>> {
    for directory in [false, true] {
        let a = Fixture::new(false)?;
        let target = if directory { "languages" } else { "Cargo.lock" };
        std::os::unix::fs::symlink(a.0.join(target), a.0.join("tools/src/linked"))?;
        assert!(a.digest().is_err());
    }
    Ok(())
}

#[test]
fn embedded_identity_matches_current_sources() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.parent().ok_or("workspace root")?;
    assert_eq!(
        identity::fingerprint(root)?,
        nepl3_tools::doc::source::host_identity().0
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn source_root_ancestor_symlinks_are_rejected() -> Result<(), Box<dyn Error>> {
    for ancestor in ["tools", "conformance"] {
        let a = Fixture::new(false)?;
        let b = Fixture::new(false)?;
        fs::remove_dir_all(a.0.join(ancestor))?;
        std::os::unix::fs::symlink(b.0.join(ancestor), a.0.join(ancestor))?;
        assert!(a.digest().is_err(), "{ancestor}");
    }
    Ok(())
}
