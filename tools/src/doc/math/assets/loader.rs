//! Bounded host loading from a trusted, stable KaTeX installation.
//! This is content checking, not package provenance or race-free confinement.
//! Observed symlinks are rejected, but path replacement and blocking filesystem
//! I/O cannot be prevented/interrupted by portable std path checks or polling.
//! Root ancestors are host-selected authority; these checks are not a filesystem
//! sandbox or a no-follow guarantee.
use super::{Input, PreparedAssets, pins};
use nepl3_core::budget::{Budget, Resource, StopReason};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    RootMetadata,
    PathMetadata,
    Open,
    HandleMetadata,
    Read,
    Eof,
}
#[derive(Debug, PartialEq, Eq)]
pub enum LoadError {
    Stopped(StopReason),
    Io {
        index: Option<usize>,
        stage: Stage,
        kind: io::ErrorKind,
    },
    InvalidSource {
        index: usize,
    },
    Symlink {
        index: Option<usize>,
    },
    NotDirectory {
        index: Option<usize>,
    },
    NotRegular {
        index: usize,
    },
    Size {
        index: usize,
    },
    Assets(super::Error),
}
impl From<StopReason> for LoadError {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl From<super::Error> for LoadError {
    fn from(e: super::Error) -> Self {
        match e {
            super::Error::Stopped(s) => Self::Stopped(s),
            e => Self::Assets(e),
        }
    }
}
fn io_error(index: Option<usize>, stage: Stage, error: io::Error) -> LoadError {
    LoadError::Io {
        index,
        stage,
        kind: error.kind(),
    }
}

/// Only compiled pins determine what is read. No directory scan, environment
/// lookup, runtime manifest, package install or network request is performed.
/// max_asset_bytes limits retained inventory bytes; bounded EOF probes are
/// detection overhead. Binary assets consume no SourceBytes or OutputBytes.
/// Raw buffers, path/table storage and the preparer's retained copies are all
/// charged cumulatively, without refunds. These are logical, not RSS charges.
pub fn load_installation(
    root: &Path,
    max_asset_bytes: u64,
    b: &mut Budget,
) -> Result<PreparedAssets, LoadError> {
    let result = (|| {
        b.poll()?;
        let mut total = 0u64;
        let mut longest = 0usize;
        for (index, pin) in pins::PINS.iter().enumerate() {
            b.charge(Resource::Work, pin.source.len() as u64 + 1)?;
            if pin.source.is_empty()
                || Path::new(pin.source)
                    .components()
                    .any(|c| !matches!(c, Component::Normal(_)))
            {
                return Err(LoadError::InvalidSource { index });
            }
            total = total
                .checked_add(pin.bytes as u64)
                .ok_or(LoadError::Assets(super::Error::AssetByteLimit))?;
            longest = longest.max(pin.source.len());
        }
        if total > max_asset_bytes {
            return Err(LoadError::Assets(super::Error::AssetByteLimit));
        }
        let path_capacity = root
            .as_os_str()
            .len()
            .checked_add(longest)
            .and_then(|n| n.checked_add(2))
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, path_capacity as u64)?;
        let mut path = PathBuf::new();
        path.try_reserve_exact(path_capacity)
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        let mut raw = super::reserve::<Vec<u8>>(pins::PINS.len(), b)?;
        b.charge(Resource::Work, root.as_os_str().len() as u64 + 1)?;
        let root_metadata =
            fs::symlink_metadata(root).map_err(|e| io_error(None, Stage::RootMetadata, e))?;
        if root_metadata.file_type().is_symlink() {
            return Err(LoadError::Symlink { index: None });
        }
        if !root_metadata.is_dir() {
            return Err(LoadError::NotDirectory { index: None });
        }
        for (index, pin) in pins::PINS.iter().enumerate() {
            b.charge(Resource::Work, root.as_os_str().len() as u64 + 1)?;
            path.clear();
            path.push(root);
            let mut components = Path::new(pin.source).components().peekable();
            while let Some(component) = components.next() {
                let Component::Normal(name) = component else {
                    return Err(LoadError::InvalidSource { index });
                };
                b.charge(Resource::Work, name.len() as u64 + 1)?;
                path.push(name);
                b.charge(Resource::Work, 1)?;
                let metadata = fs::symlink_metadata(&path)
                    .map_err(|e| io_error(Some(index), Stage::PathMetadata, e))?;
                if metadata.file_type().is_symlink() {
                    return Err(LoadError::Symlink { index: Some(index) });
                }
                if components.peek().is_some() {
                    if !metadata.is_dir() {
                        return Err(LoadError::NotDirectory { index: Some(index) });
                    }
                } else if !metadata.is_file() {
                    return Err(LoadError::NotRegular { index });
                }
            }
            b.charge(Resource::Work, 1)?;
            let mut file = File::open(&path).map_err(|e| io_error(Some(index), Stage::Open, e))?;
            b.charge(Resource::Work, 1)?;
            let metadata = file
                .metadata()
                .map_err(|e| io_error(Some(index), Stage::HandleMetadata, e))?;
            if !metadata.is_file() {
                return Err(LoadError::NotRegular { index });
            }
            if metadata.len() != pin.bytes as u64 {
                return Err(LoadError::Size { index });
            }
            let mut bytes = super::reserve::<u8>(pin.bytes, b)?;
            b.charge(Resource::Work, pin.bytes as u64)?;
            bytes.resize(pin.bytes, 0);
            read_bounded(&mut file, &mut bytes, index, b)?;
            b.charge(Resource::Work, 1)?;
            raw.push(bytes);
        }
        let mut input = super::reserve::<Input<'_>>(pins::PINS.len(), b)?;
        for (pin, bytes) in pins::PINS.iter().zip(&raw) {
            b.charge(Resource::Work, 1)?;
            input.push(Input {
                path: pin.path,
                mime: pin.mime,
                bytes,
            });
        }
        super::prepare(&input, max_asset_bytes, b).map_err(LoadError::from)
    })();
    b.poll()?;
    result
}
fn read_bounded(
    reader: &mut impl Read,
    bytes: &mut [u8],
    index: usize,
    b: &mut Budget,
) -> Result<(), LoadError> {
    // Prepay bounded byte processing; separately meter every call/retry.
    b.charge(Resource::Work, bytes.len() as u64 + 1)?;
    let mut offset = 0;
    while offset < bytes.len() {
        b.charge(Resource::Work, 1)?;
        match reader.read(&mut bytes[offset..]) {
            Ok(0) => return Err(LoadError::Size { index }),
            Ok(n) if n <= bytes.len() - offset => offset += n,
            Ok(_) => {
                return Err(LoadError::Io {
                    index: Some(index),
                    stage: Stage::Read,
                    kind: io::ErrorKind::InvalidData,
                });
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(io_error(Some(index), Stage::Read, e)),
        }
    }
    let mut extra = [0u8; 1];
    loop {
        b.charge(Resource::Work, 1)?;
        match reader.read(&mut extra) {
            Ok(0) => return Ok(()),
            Ok(_) => return Err(LoadError::Size { index }),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(io_error(Some(index), Stage::Eof, e)),
        }
    }
}

#[cfg(test)]
mod tests;
