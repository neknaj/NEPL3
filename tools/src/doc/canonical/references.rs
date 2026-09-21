//! Host-supplied projections of explicitly registered passive sources.
//! Source identity and delivered bytes are deliberately different identities.
use super::{Result, portable_path};
use crate::doc::export::pages::{Entry, GeneratedPages};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(crate) struct Projection {
    pub source: String,
    pub source_sha256: String,
    pub route: String,
    pub renderer: String,
    /// Rendering context, including source commit and site base path.
    pub context: String,
    pub bytes: Vec<u8>,
}

pub(super) fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(super) fn apply(inputs: &mut [(Entry, Vec<u8>)], projections: &[Projection]) -> Result<()> {
    if projections.len() > 128 || projections.len() > inputs.len() {
        return Err("unregistered reference projection".into());
    }
    let mut seen = BTreeSet::new();
    let mut total = 0usize;
    // Validate the complete selection before changing any input.
    for projection in projections {
        if !seen.insert(&projection.source)
            || !portable_path(&projection.route)
            || !projection.route.ends_with(".html")
            || projection.renderer.is_empty()
            || projection.renderer.len() > 256
            || projection.context.len() > 8192
            || projection.bytes.len() > 1024 * 1024
        {
            return Err("invalid reference projection".into());
        }
        total = total
            .checked_add(projection.bytes.len())
            .ok_or("projection size overflow")?;
        if total > 8 * 1024 * 1024 {
            return Err("projection payload limit".into());
        }
        std::str::from_utf8(&projection.bytes)?;
        let (_, source) = inputs
            .iter()
            .find(|(entry, _)| entry.source == projection.source)
            .ok_or("unregistered reference projection")?;
        if hash(source) != projection.source_sha256 {
            return Err("reference projection source changed".into());
        }
    }
    for (entry, bytes) in inputs {
        if let Some(projection) = projections.iter().find(|p| p.source == entry.source) {
            entry.route.clone_from(&projection.route);
            bytes.clone_from(&projection.bytes);
        }
    }
    // Route collisions and fragments are still checked by the normal PageSet.
    Ok(())
}

pub(super) fn record(generated: &mut GeneratedPages, projections: &[Projection]) -> Result<()> {
    if projections.is_empty() {
        return Ok(());
    }
    // Host bookkeeping has fixed size limits; it is not runtime Usage.
    if generated.manifest.len() > 8 * 1024 * 1024 {
        return Err("projection receipt limit".into());
    }
    let mut manifest: serde_json::Value = serde_json::from_str(&generated.manifest)?;
    let records = manifest["registered_files"]
        .as_array_mut()
        .ok_or("missing registered file receipt")?;
    for projection in projections {
        let record = records
            .iter_mut()
            .find(|r| r["source"].as_str() == Some(&projection.source))
            .ok_or("missing projected file receipt")?;
        // `sha256`/`bytes` describe the delivered payload, not the physical input.
        // An explicit transformation prevents treating that payload as source bytes.
        record["source_sha256"] = projection.source_sha256.clone().into();
        record["projection"] = serde_json::json!({
            "renderer": projection.renderer,
            "context": projection.context,
            "output_sha256": hash(&projection.bytes),
            "output_route": projection.route
        });
    }
    for file in manifest["files"]
        .as_array_mut()
        .ok_or("missing output file receipt")?
    {
        if projections
            .iter()
            .any(|p| file["path"].as_str() == Some(&p.route))
        {
            file["mime"] = "text/html; charset=utf-8".into();
        }
    }
    manifest["reference_projection_scope"] =
        "Bounded host preparation and receipt; excluded from PageSet runtime Usage".into();
    let updated = serde_json::to_string_pretty(&manifest)? + "\n";
    if updated.len() > 16 * 1024 * 1024 {
        return Err("projection receipt limit".into());
    }
    generated.manifest = updated;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_selection_does_not_partially_replace_inputs() {
        let mut inputs = vec![
            (
                Entry {
                    id: "a".into(),
                    source: "doc/a.md".into(),
                    route: "sources/a.md".into(),
                    input: None,
                },
                b"a".to_vec(),
            ),
            (
                Entry {
                    id: "b".into(),
                    source: "doc/b.md".into(),
                    route: "sources/b.md".into(),
                    input: None,
                },
                b"b".to_vec(),
            ),
        ];
        let projection = || Projection {
            source: "doc/a.md".into(),
            source_sha256: hash(b"a"),
            route: "docs/a.html".into(),
            renderer: "test/1".into(),
            context: "test".into(),
            bytes: b"<p>a</p>".to_vec(),
        };
        assert!(apply(&mut inputs, &[projection(), projection()]).is_err());
        assert_eq!(inputs[0].0.route, "sources/a.md");
        assert_eq!(inputs[0].1, b"a");
        let mut invalid = projection();
        invalid.route = "../outside.html".into();
        assert!(apply(&mut inputs, &[invalid]).is_err());
    }
}
