//! Conservative source identity for the development Doc provider callbacks.
//! This is not an executable hash, transport authentication or reproducible-build proof.
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

pub const ROOTS: &[&str] = &[
    "crates",
    "apps",
    "tools/src",
    "tools/build_support",
    "languages",
    "conformance/fixtures",
];
pub const FILES: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "tools/Cargo.toml",
    "tools/build.rs",
];
const MAX_FILES: usize = 16_384;
const MAX_BYTES: u64 = 64 * 1024 * 1024;

fn collect(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    let mut pending = vec![directory.to_path_buf()];
    let mut directories = 0usize;
    while let Some(directory) = pending.pop() {
        directories += 1;
        if directories > MAX_FILES {
            return Err("host source identity directory limit".into());
        }
        if !fs::symlink_metadata(&directory)?.file_type().is_dir() {
            return Err("host identity requires a real source directory".into());
        }
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                if pending.len() >= MAX_FILES {
                    return Err("host source identity directory limit".into());
                }
                pending.push(entry.path());
            } else if kind.is_file() {
                if paths.len() >= MAX_FILES {
                    return Err("host source identity file limit".into());
                }
                paths.push(entry.path());
            } else {
                return Err("host source identity rejects symlinks and special files".into());
            }
        }
    }
    Ok(())
}

// The workspace base is accepted by the caller; every input component beneath it
// must be real. This detects pre-existing links, not concurrent filesystem races.
fn check_components(root: &Path, relative: &Path) -> Result<(), Box<dyn Error>> {
    let mut path = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err("invalid host source path component".into());
        }
        path.push(component);
        if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err("host source identity rejects symlink ancestors".into());
        }
    }
    Ok(())
}

fn field(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_be_bytes());
    hash.update(bytes);
}

pub fn fingerprint(root: &Path) -> Result<[u8; 32], Box<dyn Error>> {
    let mut paths = Vec::new();
    for directory in ROOTS {
        check_components(root, Path::new(directory))?;
        collect(&root.join(directory), &mut paths)?;
    }
    for file in FILES {
        check_components(root, Path::new(file))?;
        let path = root.join(file);
        if !fs::symlink_metadata(&path)?.file_type().is_file() {
            return Err("host identity requires a regular manifest/build file".into());
        }
        paths.push(path);
    }
    let mut inputs = Vec::new();
    for path in paths {
        let relative = path.strip_prefix(root)?;
        let name = relative
            .components()
            .map(|part| {
                part.as_os_str()
                    .to_str()
                    .ok_or("non-UTF-8 host source path")
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("/");
        if name.contains(['\n', '\r', '\0', '\\']) {
            return Err("invalid host source path".into());
        }
        inputs.push((name, path));
    }
    inputs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hash = Sha256::new();
    field(&mut hash, b"nepl3.doc-host-source/1");
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    for (name, path) in inputs {
        field(&mut hash, name.as_bytes());
        check_components(root, path.strip_prefix(root)?)?;
        let mut file = File::open(path)?;
        let length = file.metadata()?.len();
        total = total
            .checked_add(length)
            .ok_or("host source size overflow")?;
        if total > MAX_BYTES {
            return Err("host source identity byte limit".into());
        }
        hash.update(length.to_be_bytes());
        let mut seen = 0u64;
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            seen = seen
                .checked_add(count as u64)
                .ok_or("host source size overflow")?;
            if seen > length {
                return Err("host source changed during identity generation".into());
            }
            hash.update(&buffer[..count]);
        }
        if seen != length {
            return Err("host source changed during identity generation".into());
        }
    }
    Ok(hash.finalize().into())
}
