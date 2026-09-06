use crate::{
    Result, json,
    repository::{inventory, local_path},
    task::TaskFile,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct Identity {
    profile: String,
    source_sha256: String,
    spec_sha256: String,
}

#[derive(Serialize, Debug)]
pub(crate) struct Snapshot {
    pub design_revision: String,
    pub identity: Identity,
}

pub(crate) fn is_digest(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn included(path: &str) -> bool {
    path != "implementation-status.json"
        && !path.starts_with("conformance/results/")
        && !path.starts_with("tasks/")
}

fn update(hash: &mut Sha256, path: &str, bytes: &[u8]) -> Result<()> {
    hash.update(u64::try_from(path.len())?.to_be_bytes());
    hash.update(path.as_bytes());
    hash.update(u64::try_from(bytes.len())?.to_be_bytes());
    hash.update(bytes);
    Ok(())
}

pub(crate) fn identity(root: &Path) -> Result<Snapshot> {
    let task_file: TaskFile = json(root, "design/tasks.json")?;
    let mut source = Sha256::new();
    source.update(b"nepl3.repository-inputs/1\0source\0");
    let mut spec = Sha256::new();
    spec.update(b"nepl3.repository-inputs/1\0spec\0");
    // Inventory comes from Git, never from an evidence author's supplied list.
    // Path and content lengths prevent ambiguous concatenations. Raw bytes keep
    // source fixtures lossless; .gitattributes fixes ordinary files to LF.
    for path in inventory(root)?.iter().filter(|path| included(path)) {
        let full = local_path(root, path)?;
        if fs::symlink_metadata(&full)?.file_type().is_symlink() || !full.is_file() {
            return Err(format!("identity input is not a regular file: {path}").into());
        }
        if fs::metadata(&full)?.len() > 1024 * 1024 {
            return Err(format!("identity input exceeds 1 MiB: {path}").into());
        }
        let bytes = fs::read(&full)?;
        update(&mut source, path, &bytes)?;
        if ["doc/spec/", "interfaces/", "design/"]
            .iter()
            .any(|prefix| path.starts_with(prefix))
        {
            update(&mut spec, path, &bytes)?;
        }
    }
    Ok(Snapshot {
        design_revision: task_file.design,
        identity: Identity {
            profile: "nepl3.repository-inputs/1".into(),
            source_sha256: hex(&source.finalize()),
            spec_sha256: hex(&spec.finalize()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_prefixed_profile_matches_independent_hashlib_vector() -> Result<()> {
        // Independently computed with Python hashlib.sha256 and int.to_bytes(8, "big").
        // This fixes byte order and domain prefix, not an implementation-generated golden.
        let mut hash = Sha256::new();
        hash.update(b"nepl3.repository-inputs/1\0source\0");
        update(&mut hash, "a", b"x")?;
        update(&mut hash, "bc", b"yz")?;
        assert_eq!(
            hex(&hash.finalize()),
            "6a9cfd06380b644a7ab9d5570f554c3e750fda7029749275397a5fa6b9b1e116"
        );
        Ok(())
    }
}
