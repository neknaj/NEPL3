//! Input identity for the explicit root-only, embedded-SVG host contract.
//! This is not evidence that assets, namespaces or rendered HTML were accepted.
use crate::doc::annotations::article::render::svg::pages::RootAssets;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::Digest,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::pages::PageSet;
use nepl3_doc_html::RenderOptions;

fn err(value: impl core::fmt::Debug) -> String {
    format!("{value:?}")
}
fn append(out: &mut Vec<u8>, bytes: &[u8], b: &mut Budget) -> Result<(), String> {
    let size = u64::try_from(bytes.len()).map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
    b.charge(Resource::Work, size).map_err(err)?;
    let allocation = size
        .checked_mul(2)
        .ok_or_else(|| err(b.stop(StopReason::AllocationLimit)))?;
    b.charge(Resource::AllocationUnits, allocation)
        .map_err(err)?;
    out.try_reserve(bytes.len())
        .map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
    out.extend_from_slice(bytes);
    Ok(())
}

/// Binds the canonical PageSet, exact display options, ordered root bindings,
/// each asset ID and the length/digest of its exact UTF-8 SVG bytes.
/// Empty bindings and input order are intentionally significant. Hidden inputs
/// are never omitted merely because a display selection does not show them.
/// Asset admission and unused-input rejection remain mandatory in the renderer.
pub fn input<C: FoundationValueCodec>(
    set: &PageSet,
    options: &RenderOptions,
    assets: &[RootAssets<'_>],
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<Digest, String>
where
    C::Error: core::fmt::Debug,
{
    let result = input_inner(set, options, assets, registry, codec, budget);
    budget.poll().map_err(err)?;
    result
}
fn input_inner<C: FoundationValueCodec>(
    set: &PageSet,
    options: &RenderOptions,
    assets: &[RootAssets<'_>],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<Digest, String>
where
    C::Error: core::fmt::Debug,
{
    b.poll().map_err(err)?;
    let value =
        nepl3_doc_core::portable::pages::set_to_value(set, registry, codec, b).map_err(err)?;
    let source = codec
        .canonical_value_digest(b"nepl3.local-doc-pages.svg-source/1\0", &value, b)
        .map_err(err)?;
    let mut bytes = Vec::new();
    append(
        &mut bytes,
        b"nepl3.local-doc-pages.root-svg-embedded/1\0",
        b,
    )?;
    append(&mut bytes, &source.0, b)?;
    append(&mut bytes, &(assets.len() as u64).to_le_bytes(), b)?;
    for (index, root) in assets.iter().enumerate() {
        b.charge(Resource::Work, 1).map_err(err)?;
        if usize::try_from(root.page)
            .ok()
            .is_none_or(|p| p >= set.pages.len())
        {
            return Err("InvalidSvgPage".into());
        }
        for previous in &assets[..index] {
            b.charge(Resource::Work, 1).map_err(err)?;
            if previous.page == root.page {
                return Err("DuplicateSvgPage".into());
            }
        }
        append(&mut bytes, &root.page.to_le_bytes(), b)?;
        append(&mut bytes, &(root.inputs.len() as u64).to_le_bytes(), b)?;
        for asset in root.inputs {
            append(&mut bytes, &(asset.id.len() as u64).to_le_bytes(), b)?;
            append(&mut bytes, asset.id.as_bytes(), b)?;
            append(&mut bytes, &(asset.svg.len() as u64).to_le_bytes(), b)?;
            b.charge(Resource::Work, asset.svg.len() as u64)
                .map_err(err)?;
            append(&mut bytes, &Digest::of(asset.svg.as_bytes()).0, b)?;
        }
    }
    b.charge(Resource::Work, bytes.len() as u64).map_err(err)?;
    super::super::display::bind_identity(
        b"nepl3.local-doc-pages.root-svg-display/1\0",
        Digest::of(&bytes),
        &options.parallel,
        b,
    )
}
