//! Only private admission evidence from the exact root renderer reaches here.
use super::*;
use planned::ExternalFile;
use std::collections::BTreeSet;

pub(super) fn add_files(
    set: &PageSet,
    assets: &[ExternalFile],
    files: &mut BTreeMap<String, Vec<u8>>,
    b: &mut Budget,
) -> Result<(), String> {
    let mut generated = BTreeSet::new();
    for asset in assets {
        charge(b, 1, 0)?;
        if asset.member() != nepl3_doc_core::labels::namespace::MemberId(0) {
            return Err("ExternalSvgRootRequired".into());
        }
        let page = usize::try_from(asset.page())
            .ok()
            .and_then(|index| set.pages.get(index))
            .ok_or("InvalidExternalSvgPage")?;
        let route = &page.registration.route;
        charge(
            b,
            route.len() + asset.path().len() + 1,
            route.len() + asset.path().len() + 1,
        )?;
        let path = match route.rsplit_once('/') {
            Some((parent, _)) => format!("{parent}/{}", asset.path()),
            None => asset.path().to_owned(),
        };
        // Every lookup/equality scan is accounted independently of physical
        // file deduplication. Existing registered files never enter this set.
        for existing in &generated {
            charge(b, 2 * (path.len() + String::len(existing) + 1), 0)?;
        }
        if generated.contains(&path) {
            for existing in files.keys() {
                charge(b, path.len() + existing.len() + 1, 0)?;
            }
            let prior = files.get(&path).ok_or("MissingGeneratedSvg")?;
            charge(b, prior.len() + asset.svg().len(), 0)?;
            if prior != asset.svg().as_bytes() {
                return Err("GeneratedSvgByteCollision".into());
            }
            continue;
        }
        charge(b, asset.svg().len(), asset.svg().len())?;
        b.charge(Resource::OutputBytes, asset.svg().len() as u64)
            .map_err(err)?;
        charge(b, path.len() + 1, path.len() + 256)?;
        insert(files, path.clone(), asset.svg().as_bytes().to_vec(), b)?;
        generated.insert(path);
    }
    Ok(())
}
