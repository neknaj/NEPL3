//! File and compilation adapter for the explicitly selected viewing profile.
use super::*;
use crate::doc::source::{Compiled, budget, err, with_input_route};
use nepl3_core::source::{SourceAdmission, SourceStore};
use nepl3_doc_core::{check::Category, lower};
use nepl3_wire::foundation::FoundationCodec;
use std::{
    io::{Read, Write},
    path::Path,
};

pub fn from_source(
    compiled: &Compiled,
    source: &str,
    aliases: &[Alias],
) -> Result<Artifact, String> {
    if source.len() as u64 > crate::doc::export::MAX_SOURCE_BYTES {
        return Err("SourceLimit".into());
    }
    with_input_route(
        true,
        compiled,
        source,
        "Article",
        |tree, profile, _b, _a| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let document = lower::document(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            render(
                &document,
                profile.registry(),
                &mut codec,
                &mut budget(),
                aliases,
            )
            .map_err(err)
        },
    )
}

/// Explicit all-notes view; aliases is a JSON array of {section, name} values.
pub fn write(input: &Path, aliases: &Path, output: &Path) -> crate::Result<()> {
    if output.exists() {
        return Err("output already exists".into());
    }
    let path = input.to_str().ok_or("input path must be UTF-8")?;
    if path.len() > 4096 || path.chars().any(char::is_control) {
        return Err("input path is not representable in projection metadata".into());
    }
    let mut source = String::new();
    std::fs::File::open(input)?
        .take(crate::doc::export::MAX_SOURCE_BYTES + 1)
        .read_to_string(&mut source)?;
    let mut alias_bytes = Vec::new();
    std::fs::File::open(aliases)?
        .take(1_048_577)
        .read_to_end(&mut alias_bytes)?;
    if alias_bytes.len() > 1_048_576 {
        return Err("alias input exceeds limit".into());
    }
    let options: Vec<Alias> = serde_json::from_slice(&alias_bytes)?;
    let artifact = from_source(&crate::doc::source::compiled()?, &source, &options)?;
    let hex = |digest: Digest| -> String { digest.0.iter().map(|b| format!("{b:02x}")).collect() };
    let source_digest = hex(Digest::of(source.as_bytes()));
    let options_digest = hex(Digest::of(&alias_bytes));
    let document_digest = hex(artifact.document_digest);
    let path = path
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('-', "&#45;");
    let metadata = format!(
        "<!-- Generated from {path}; renderer nepl3-tools.markdown-annotated/1; source SHA-256 {source_digest}; alias input SHA-256 {options_digest}; document digest {document_digest}. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->\n\n"
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(metadata.as_bytes())?;
    file.write_all(artifact.markdown.as_bytes())?;
    Ok(())
}
