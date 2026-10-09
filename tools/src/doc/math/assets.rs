//! Native fixed KaTeX resource-content and reviewed class-catalog boundary.
//! Preparation performs no file I/O; the separate host loader reads a trusted
//! installation. Neither path proves renderer execution or portable artifacts.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::Digest,
};
mod catalog;
pub mod loader;
mod pins;
struct Pin {
    source: &'static str,
    path: &'static str,
    mime: &'static str,
    bytes: usize,
    digest: [u8; 32],
}
/// Already bounded host input. The preparer copies only exact validated lengths,
/// so spare capacity or caller mutation cannot become retained asset authority.
pub struct Input<'a> {
    pub path: &'a str,
    pub mime: &'a str,
    pub bytes: &'a [u8],
}
pub struct Asset {
    path: &'static str,
    mime: &'static str,
    digest: Digest,
    bytes: Vec<u8>,
}
impl Asset {
    pub fn path(&self) -> &str {
        self.path
    }
    pub fn mime(&self) -> &str {
        self.mime
    }
    pub fn digest(&self) -> Digest {
        self.digest
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
pub struct PreparedAssets {
    files: Vec<Asset>,
    identity: Digest,
}
impl PreparedAssets {
    pub fn files(&self) -> &[Asset] {
        &self.files
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn version(&self) -> &str {
        pins::VERSION
    }
    /// Materialize another complete owned file set. Every call pays output and
    /// copy costs; inspecting borrowed bytes is not itself an export operation.
    pub fn materialize(&self, b: &mut Budget) -> Result<Vec<Asset>, Error> {
        b.poll()?;
        let mut files = reserve(self.files.len(), b)?;
        for file in &self.files {
            b.charge(Resource::OutputBytes, file.bytes.len() as u64)?;
            files.push(Asset {
                path: file.path,
                mime: file.mime,
                digest: file.digest,
                bytes: copy(&file.bytes, b)?,
            });
        }
        b.poll()?;
        Ok(files)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Stopped(StopReason),
    Count,
    Metadata { index: usize },
    Digest { index: usize },
    AssetByteLimit,
}
impl From<StopReason> for Error {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
fn reserve<T>(count: usize, b: &mut Budget) -> Result<Vec<T>, Error> {
    let bytes = count
        .checked_mul(core::mem::size_of::<T>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    Ok(output)
}
fn copy(bytes: &[u8], b: &mut Budget) -> Result<Vec<u8>, Error> {
    b.charge(Resource::Work, bytes.len() as u64)?;
    let mut owned = reserve(bytes.len(), b)?;
    owned.extend_from_slice(bytes);
    Ok(owned)
}
/// Validate the fixed complete inventory, then hash and copy into private parts.
/// Binary asset bytes have an explicit retained-byte cap and metered copy/hash
/// costs; they are not text SourceSnapshots and create no SourceAdmission entry.
/// Preparation emits zero OutputBytes. Content identity is not package origin,
/// execution identity, stylesheet class coverage or document namespace safety.
pub fn prepare(
    input: &[Input<'_>],
    max_asset_bytes: u64,
    b: &mut Budget,
) -> Result<PreparedAssets, Error> {
    b.poll()?;
    if input.len() != pins::PINS.len() {
        return Err(Error::Count);
    }
    let mut total = 0u64;
    for (index, (file, pin)) in input.iter().zip(pins::PINS).enumerate() {
        b.charge(Resource::Work, 1)?;
        if file.path.len() != pin.path.len()
            || file.mime.len() != pin.mime.len()
            || file.bytes.len() != pin.bytes
        {
            return Err(Error::Metadata { index });
        }
        b.charge(Resource::Work, (pin.path.len() + pin.mime.len()) as u64)?;
        if file.path != pin.path || file.mime != pin.mime {
            return Err(Error::Metadata { index });
        }
        total = total
            .checked_add(pin.bytes as u64)
            .ok_or(Error::AssetByteLimit)?;
    }
    if total > max_asset_bytes {
        return Err(Error::AssetByteLimit);
    }
    let mut files = reserve(input.len(), b)?;
    for (index, (file, pin)) in input.iter().zip(pins::PINS).enumerate() {
        b.charge(Resource::Work, file.bytes.len() as u64)?;
        let digest = Digest::of(file.bytes);
        if digest.0 != pin.digest {
            return Err(Error::Digest { index });
        }
        files.push(Asset {
            path: pin.path,
            mime: pin.mime,
            digest,
            bytes: copy(file.bytes, b)?,
        });
    }
    let identity = identity(&files, b)?;
    b.poll()?;
    Ok(PreparedAssets { files, identity })
}
fn identity(files: &[Asset], b: &mut Budget) -> Result<Digest, Error> {
    let domain = b"nepl3.katex.asset-parts/1\0";
    let mut size = domain.len() + 8 + pins::VERSION.len() + 8;
    for file in files {
        size = size
            .checked_add(8 + file.path.len() + 8 + file.mime.len() + 8 + 32)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    }
    // One framed metadata copy, then one independent SHA-256 traversal.
    b.charge(Resource::Work, size as u64)?;
    let mut bytes = reserve(size, b)?;
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&(pins::VERSION.len() as u64).to_be_bytes());
    bytes.extend_from_slice(pins::VERSION.as_bytes());
    bytes.extend_from_slice(&(files.len() as u64).to_be_bytes());
    for file in files {
        bytes.extend_from_slice(&(file.path.len() as u64).to_be_bytes());
        bytes.extend_from_slice(file.path.as_bytes());
        bytes.extend_from_slice(&(file.mime.len() as u64).to_be_bytes());
        bytes.extend_from_slice(file.mime.as_bytes());
        bytes.extend_from_slice(&(file.bytes.len() as u64).to_be_bytes());
        bytes.extend_from_slice(&file.digest.0);
    }
    b.charge(Resource::Work, bytes.len() as u64)?;
    Ok(Digest::of(&bytes))
}

/// Fixed-catalog visual content tied by lifetime to exact verified resource bytes.
/// It is not executing-renderer identity, same-Math pairing or Doc admission.
pub struct BoundVisual<'a> {
    parts: super::katex::PreparedVisual,
    assets: &'a PreparedAssets,
}
/// Typed visual content bound to the exact fixed resources. No public owner
/// replacement or mutable content access is provided; this is not a Math artifact.
/// ```compile_fail
/// fn replace(p: &mut nepl3_tools::doc::math::assets::BoundProjectedVisual<'_>) { p.assets = p.assets(); }
/// ```
pub struct BoundProjectedVisual<'a> {
    visual: nepl3_markup::katex::fragment::ProjectedVisual,
    assets: &'a PreparedAssets,
}
impl<'a> BoundProjectedVisual<'a> {
    pub(in crate::doc::math) fn into_parts(
        self,
    ) -> (
        nepl3_markup::katex::fragment::ProjectedVisual,
        &'a PreparedAssets,
    ) {
        (self.visual, self.assets)
    }
    pub fn visual(&self) -> &nepl3_markup::katex::fragment::ProjectedVisual {
        &self.visual
    }
    pub fn assets(&self) -> &PreparedAssets {
        self.assets
    }
    pub fn catalog_identity(&self) -> Digest {
        Digest(catalog::IDENTITY)
    }
}
pub struct BoundRenderedVisual<'a> {
    rendered: nepl3_markup::katex::fragment::Rendered,
    assets: &'a PreparedAssets,
}
impl<'a> BoundVisual<'a> {
    pub fn into_html(
        self,
        b: &mut Budget,
    ) -> Result<BoundProjectedVisual<'a>, super::katex::Error> {
        let visual = self.parts.into_html(b)?;
        Ok(BoundProjectedVisual {
            visual,
            assets: self.assets,
        })
    }
    pub fn parts(&self) -> &super::katex::PreparedVisual {
        &self.parts
    }
    pub fn assets(&self) -> &PreparedAssets {
        self.assets
    }
    pub fn catalog_identity(&self) -> Digest {
        Digest(catalog::IDENTITY)
    }
    pub fn serialize(
        &self,
        b: &mut Budget,
    ) -> Result<BoundRenderedVisual<'_>, super::katex::Error> {
        let rendered = self.parts.serialize(b)?;
        Ok(BoundRenderedVisual {
            rendered,
            assets: self.assets,
        })
    }
}
impl BoundRenderedVisual<'_> {
    pub fn visual(&self) -> &nepl3_markup::katex::fragment::Rendered {
        &self.rendered
    }
    pub fn assets(&self) -> &PreparedAssets {
        self.assets
    }
    pub fn catalog_identity(&self) -> Digest {
        Digest(catalog::IDENTITY)
    }
}
impl PreparedAssets {
    /// Select classes from the fixed reviewed resource catalog, never renderer
    /// output. Preserve the asset association through visual serialization.
    pub fn prepare_visual<'a>(
        &'a self,
        value: serde_json::Value,
        scope: &str,
        b: &mut Budget,
    ) -> Result<BoundVisual<'a>, super::katex::Error> {
        let policy = self.visual_policy(scope, b)?;
        let parts = super::katex::prepare(value, &policy, b)?;
        Ok(BoundVisual {
            parts,
            assets: self,
        })
    }
    pub(crate) fn prepare_fragment<'a>(
        &'a self,
        fragment: nepl3_markup::katex::fragment::Fragment,
        scope: &str,
        b: &mut Budget,
    ) -> Result<BoundVisual<'a>, super::katex::Error> {
        let policy = self.visual_policy(scope, b)?;
        let parts = super::katex::prepare_fragment(fragment, &policy, b)?;
        Ok(BoundVisual {
            parts,
            assets: self,
        })
    }
    fn visual_policy<'a>(
        &self,
        scope: &'a str,
        b: &mut Budget,
    ) -> Result<nepl3_markup::katex::fragment::Policy<'a>, super::katex::Error> {
        b.poll()?;
        b.charge(Resource::Work, 33 + pins::VERSION.len() as u64)?;
        if pins::VERSION != catalog::VERSION
            || self.files.first().map(|v| v.digest.0) != Some(catalog::CSS_DIGEST)
        {
            return Err(super::katex::Error::AssetPolicy);
        }
        Ok(nepl3_markup::katex::fragment::Policy {
            classes: catalog::CLASSES,
            scope,
        })
    }
}
