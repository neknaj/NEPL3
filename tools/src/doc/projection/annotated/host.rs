//! File and compilation adapter for the explicitly selected viewing profile.
use super::*;
use crate::doc::export::pages::resources::{ExecutionLimits, PhaseLimits};
use crate::doc::source::{Compiled, budget, err, with_named_input_limits};
use nepl3_core::source::{SourceAdmission, SourceStore};
use nepl3_doc_core::check::Category;
use nepl3_wire::foundation::FoundationCodec;
use std::{
    io::{Read, Write},
    path::Path,
};

pub const RENDERER: &str = "nepl3-tools.markdown-annotated/4";
pub const FOOTNOTES_RENDERER: &str = "nepl3-tools.markdown-footnotes/1";

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
    from_source_profile(
        compiled,
        source,
        aliases,
        output_budget,
        NotesMode::Inline,
        PhaseLimits::default(),
    )
}
pub fn from_source_footnotes(
    compiled: &Compiled,
    source: &str,
    aliases: &[Alias],
) -> Result<Artifact, String> {
    from_source_profile(
        compiled,
        source,
        aliases,
        &mut budget(),
        NotesMode::Footnotes,
        PhaseLimits::default(),
    )
}
fn from_source_profile(
    compiled: &Compiled,
    source: &str,
    aliases: &[Alias],
    output_budget: &mut Budget,
    mode: NotesMode,
    phases: PhaseLimits,
) -> Result<Artifact, String> {
    output_budget.poll().map_err(err)?;
    if source.len() as u64 > crate::doc::export::MAX_SOURCE_BYTES {
        return Err("SourceLimit".into());
    }
    with_named_input_limits(
        true,
        compiled,
        source,
        "doc-input",
        "Article",
        phases.parse,
        |tree, profile, _b, _a| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let document = nepl3_suite::adapters::sentence::literal::lower(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut Budget::new(phases.lower),
                &mut codec,
            )
            .map_err(err)?;
            render_profile(
                &document,
                profile.registry(),
                &mut codec,
                output_budget,
                aliases,
                mode,
            )
            .map_err(err)
        },
    )
}

/// Explicit all-notes view; aliases is a JSON array of {section, name} values.
pub fn write(input: &Path, aliases: &Path, output: &Path) -> crate::Result<()> {
    write_profile(
        input,
        aliases,
        output,
        NotesMode::Inline,
        ExecutionLimits::default(),
    )
}
pub fn write_footnotes(input: &Path, aliases: &Path, output: &Path) -> crate::Result<()> {
    write_profile(
        input,
        aliases,
        output,
        NotesMode::Footnotes,
        ExecutionLimits::default(),
    )
}
pub fn write_with_limits(
    input: &Path,
    aliases: &Path,
    output: &Path,
    mode: &str,
    limits: ExecutionLimits,
) -> crate::Result<()> {
    let mode = match mode {
        "annotated" => NotesMode::Inline,
        "footnotes" => NotesMode::Footnotes,
        _ => return Err("expected annotated or footnotes".into()),
    };
    write_profile(input, aliases, output, mode, limits)
}
fn write_profile(
    input: &Path,
    aliases: &Path,
    output: &Path,
    mode: NotesMode,
    limits: ExecutionLimits,
) -> crate::Result<()> {
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
    let (text, _) = generate_receipt_profile(
        &crate::doc::source::compiled()?,
        path,
        &source,
        &alias_bytes,
        None,
        &mut limits.output_limits.budget(),
        mode,
        limits.phases(),
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
    generate_with_budget(compiled, path, source, alias_bytes, None, &mut budget())
}

/// Preserve the legacy source namespace and bytes while sharing a caller's
/// resolve/render/metadata budget across an explicit batch. Parse and lower
/// remain separately bounded operations; a stopped output is never restarted.
pub(crate) fn generate_with_budget(
    compiled: &Compiled,
    path: &str,
    source: &str,
    alias_bytes: &[u8],
    source_href: Option<&str>,
    output_budget: &mut Budget,
) -> crate::Result<String> {
    generate_with_receipt(
        compiled,
        path,
        source,
        alias_bytes,
        source_href,
        output_budget,
    )
    .map(|(text, _)| text)
}

/// Keep the internal schema-dependent identity in the generation receipt.
pub(crate) fn generate_with_receipt(
    compiled: &Compiled,
    path: &str,
    source: &str,
    alias_bytes: &[u8],
    source_href: Option<&str>,
    output_budget: &mut Budget,
) -> crate::Result<(String, Digest)> {
    generate_receipt_profile(
        compiled,
        path,
        source,
        alias_bytes,
        source_href,
        output_budget,
        NotesMode::Inline,
        PhaseLimits::default(),
    )
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_with_phase_limits(
    compiled: &Compiled,
    path: &str,
    source: &str,
    alias_bytes: &[u8],
    source_href: Option<&str>,
    output_budget: &mut Budget,
    phases: PhaseLimits,
) -> crate::Result<(String, Digest)> {
    generate_receipt_profile(
        compiled,
        path,
        source,
        alias_bytes,
        source_href,
        output_budget,
        NotesMode::Inline,
        phases,
    )
}
#[allow(clippy::too_many_arguments)]
fn generate_receipt_profile(
    compiled: &Compiled,
    path: &str,
    source: &str,
    alias_bytes: &[u8],
    source_href: Option<&str>,
    output_budget: &mut Budget,
    mode: NotesMode,
    phases: PhaseLimits,
) -> crate::Result<(String, Digest)> {
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
    let artifact = from_source_profile(compiled, source, &options, output_budget, mode, phases)?;
    finish_projection(
        path,
        source,
        alias_bytes,
        source_href,
        output_budget,
        mode,
        artifact,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_projection(
    path: &str,
    source: &str,
    alias_bytes: &[u8],
    source_href: Option<&str>,
    output_budget: &mut Budget,
    mode: NotesMode,
    mut artifact: Artifact,
) -> crate::Result<(String, Digest)> {
    if let Some(href) = source_href {
        artifact.source_link(href, output_budget).map_err(err)?;
    }
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
    let path = path
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('-', "&#45;");
    let renderer = if mode == NotesMode::Footnotes {
        FOOTNOTES_RENDERER
    } else {
        RENDERER
    };
    let metadata = format!(
        "<!-- Generated from {path}; renderer {renderer}; source SHA-256 {source_digest}; alias input SHA-256 {options_digest}. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->\n\n"
    );
    // Block separators belong between blocks; a file ends with one LF.
    let text = metadata + artifact.markdown.trim_end_matches('\n') + "\n";
    output_budget
        .charge(Resource::OutputBytes, text.len() as u64)
        .map_err(err)?;
    Ok((text, artifact.document_digest))
}

/// Owned legacy source/AST binding. Fields are private so a caller cannot pair
/// a document with another source or alias digest. Never contains admission proofs.
pub(crate) struct PreparedLegacy {
    document: DocumentSyntax,
    path: String,
    source: String,
    alias_bytes: Vec<u8>,
}
impl PreparedLegacy {
    pub(crate) fn capture(
        compiled: &Compiled,
        path: &str,
        source: &str,
        alias_bytes: &[u8],
        phases: PhaseLimits,
        budget: &mut Budget,
    ) -> crate::Result<Self> {
        budget.poll().map_err(err)?;
        if path.len() > 4096 || path.chars().any(char::is_control) {
            return Err("input path is not representable in projection metadata".into());
        }
        if source.len() as u64 > crate::doc::export::MAX_SOURCE_BYTES
            || alias_bytes.len() > 1_048_576
        {
            return Err("legacy snapshot input exceeds limit".into());
        }
        let size = path.len() + source.len() + alias_bytes.len();
        budget.charge(Resource::Work, size as u64).map_err(err)?;
        budget
            .charge(
                Resource::AllocationUnits,
                (size + core::mem::size_of::<Self>()) as u64,
            )
            .map_err(err)?;
        let document = with_named_input_limits(
            true,
            compiled,
            source,
            "doc-input",
            "Article",
            phases.parse,
            |tree, profile, _, _| {
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(err)?;
                nepl3_suite::adapters::sentence::literal::lower(
                    tree.syntax(),
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    &mut Budget::new(phases.lower),
                    &mut codec,
                )
                .map_err(err)
            },
        )?;
        Ok(Self {
            document,
            path: path.to_owned(),
            source: source.to_owned(),
            alias_bytes: alias_bytes.to_vec(),
        })
    }

    /// Measured parse/lower preparation for explicit full-job retry. The owned
    /// binding is constructed here, never from a caller-supplied document.
    /// Bounded retained host metadata is outside the logical phase ledger.
    pub(crate) fn capture_measured(
        compiled: &Compiled,
        path: &str,
        source: &str,
        alias_bytes: &[u8],
        phases: PhaseLimits,
        meter: &mut crate::doc::export::pages::resources::job::Meter<'_>,
        records: &mut Vec<crate::doc::export::pages::resources::job::preparation::Observation>,
    ) -> Result<Self, String> {
        if path.len() > 4096 || path.chars().any(char::is_control) {
            return Err("input path is not representable in projection metadata".into());
        }
        if source.len() as u64 > crate::doc::export::MAX_SOURCE_BYTES
            || alias_bytes.len() > 1_048_576
        {
            return Err("legacy snapshot input exceeds limit".into());
        }
        crate::doc::export::pages::resources::job::preparation::with_prepared_document(
            meter,
            compiled,
            "doc-input",
            source,
            phases,
            records,
            |document, _, _, _, _| {
                Ok(Self {
                    document,
                    path: path.to_owned(),
                    source: source.to_owned(),
                    alias_bytes: alias_bytes.to_vec(),
                })
            },
        )
    }

    pub(crate) fn render(
        &self,
        compiled: &Compiled,
        source_href: Option<&str>,
        output_budget: &mut Budget,
    ) -> crate::Result<(String, Digest)> {
        output_budget.poll().map_err(err)?;
        output_budget
            .charge(
                Resource::Work,
                (self.source.len() + self.alias_bytes.len()) as u64,
            )
            .map_err(err)?;
        output_budget
            .charge(
                Resource::AllocationUnits,
                self.alias_bytes.len() as u64 * 32,
            )
            .map_err(err)?;
        let aliases: Vec<Alias> = serde_json::from_slice(&self.alias_bytes)?;
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&compiled.doc.registry, &store, &mut admission).map_err(err)?;
        let artifact = render_profile(
            &self.document,
            &compiled.doc.registry,
            &mut codec,
            output_budget,
            &aliases,
            NotesMode::Inline,
        )
        .map_err(err)?;
        finish_projection(
            &self.path,
            &self.source,
            &self.alias_bytes,
            source_href,
            output_budget,
            NotesMode::Inline,
            artifact,
        )
    }
}
