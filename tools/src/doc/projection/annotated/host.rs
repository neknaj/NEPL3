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

pub const RENDERER: &str = "nepl3-tools.markdown-annotated/2";

pub fn from_source(
    compiled: &Compiled,
    source: &str,
    aliases: &[Alias],
) -> Result<Artifact, String> {
    from_source_with_budget(compiled, source, aliases, &mut budget())
}

fn from_source_with_budget(
    compiled: &Compiled,
    source: &str,
    aliases: &[Alias],
    output_budget: &mut Budget,
) -> Result<Artifact, String> {
    output_budget.poll().map_err(err)?;
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
                output_budget,
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
    let mut source = String::new();
    std::fs::File::open(input)?
        .take(crate::doc::export::MAX_SOURCE_BYTES + 1)
        .read_to_string(&mut source)?;
    let mut alias_bytes = Vec::new();
    std::fs::File::open(aliases)?
        .take(1_048_577)
        .read_to_end(&mut alias_bytes)?;
    let text = generate(
        &crate::doc::source::compiled()?,
        path,
        &source,
        &alias_bytes,
    )?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(text.as_bytes())?;
    Ok(())
}

/// Generate the same checked file projection without performing host writes.
/// The source name is explicit so scratch output paths do not affect metadata.
pub fn generate(
    compiled: &Compiled,
    path: &str,
    source: &str,
    alias_bytes: &[u8],
) -> crate::Result<String> {
    generate_with_budget(compiled, path, source, alias_bytes, &mut budget())
}

/// Preserve the legacy source namespace and bytes while sharing a caller's
/// resolve/render/metadata budget across an explicit batch. Parse and lower
/// remain separately bounded operations; a stopped output is never restarted.
pub(crate) fn generate_with_budget(
    compiled: &Compiled,
    path: &str,
    source: &str,
    alias_bytes: &[u8],
    output_budget: &mut Budget,
) -> crate::Result<String> {
    output_budget.poll().map_err(err)?;
    if path.len() > 4096 || path.chars().any(char::is_control) {
        return Err("input path is not representable in projection metadata".into());
    }
    if alias_bytes.len() > 1_048_576 {
        return Err("alias input exceeds limit".into());
    }
    output_budget
        .charge(Resource::Work, (source.len() + alias_bytes.len()) as u64)
        .map_err(err)?;
    output_budget
        .charge(Resource::AllocationUnits, alias_bytes.len() as u64 * 32)
        .map_err(err)?;
    let options: Vec<Alias> = serde_json::from_slice(alias_bytes)?;
    let artifact = from_source_with_budget(compiled, source, &options, output_budget)?;
    // Logical allowance includes the escaped path intermediates, digest text,
    // metadata formatting and final body copy; it is not physical heap metering.
    let reserve = (path.len() * 5 + 1024 + artifact.markdown.len()) * 8;
    output_budget
        .charge(Resource::Work, reserve as u64)
        .map_err(err)?;
    output_budget
        .charge(Resource::AllocationUnits, reserve as u64)
        .map_err(err)?;
    let hex = |digest: Digest| -> String { digest.0.iter().map(|b| format!("{b:02x}")).collect() };
    let source_digest = hex(Digest::of(source.as_bytes()));
    let options_digest = hex(Digest::of(alias_bytes));
    let document_digest = hex(artifact.document_digest);
    let path = path
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('-', "&#45;");
    let metadata = format!(
        "<!-- Generated from {path}; renderer {RENDERER}; source SHA-256 {source_digest}; alias input SHA-256 {options_digest}; document digest {document_digest}. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->\n\n"
    );
    // Block separators belong between blocks; a file ends with one LF.
    let text = metadata + artifact.markdown.trim_end_matches('\n') + "\n";
    output_budget
        .charge(Resource::OutputBytes, text.len() as u64)
        .map_err(err)?;
    Ok(text)
}
