//! Create-only host publication after generation. Not atomic visibility: on an
//! I/O failure remove only the new directory we created, reporting cleanup errors.
use super::*;
use document::bundle::LocalBundle;
use nepl3_core::budget::Budget;
use serde_json::{Value, json};

fn fallback(item: &document::Imported<'_, '_, '_>) -> Value {
    match item.fallback() {
        Some(FallbackReason::Unavailable(cause)) => {
            json!({"kind":"renderer-unavailable", "cause":cause.as_ref().map(|c| c.code()), "module":cause.as_ref().and_then(|c| c.module())})
        }
        Some(FallbackReason::RenderError) => json!({"kind":"render-error"}),
        None => match item.tex() {
            TexPreparation::Unsupported { node, reason } => {
                json!({"kind":"tex-unsupported", "node":node,"reason":format!("{reason:?}")})
            }
            _ => Value::Null,
        },
    }
}
fn observations(item: &document::Imported<'_, '_, '_>) -> Value {
    item.observations().map_or(Value::Null, |o| json!({
        "diagnostic_bytes":o.diagnostic_bytes,"reply_bytes":o.reply_bytes,
        "reported_implementation":{"node":o.implementation.node,"platform":o.implementation.platform,
            "arch":o.implementation.arch,"expected_vm_warnings":o.implementation.expected_vm_warnings},
        "diagnostics":o.diagnostics.iter().map(|d| json!({"level":format!("{:?}",d.level),"text":d.text})).collect::<Vec<_>>()
    }))
}
pub(super) fn write(
    output: &Path,
    bundle: &LocalBundle<'_, '_, '_, '_, '_>,
    source: &str,
    host_digest: Digest,
    profile: Digest,
    b: &Budget,
) -> Result<(), String> {
    let mut files = vec![
        (
            "document.html".to_owned(),
            "text/html; charset=utf-8",
            bundle.html().as_bytes(),
        ),
        (
            "assets/doc.css".to_owned(),
            "text/css; charset=utf-8",
            bundle.doc_stylesheet().as_bytes(),
        ),
    ];
    if !bundle.math_stylesheet().is_empty() {
        files.push((
            "assets/math.css".to_owned(),
            "text/css; charset=utf-8",
            bundle.math_stylesheet().as_bytes(),
        ));
    }
    for asset in bundle.katex_assets() {
        files.push((
            format!("assets/katex/{}", asset.path()),
            asset.mime(),
            asset.bytes(),
        ));
    }
    let records = bundle.source().math().iter().map(|item| json!({
        "guest_occurrence":item.context().ordinal(),"embed":item.context().reference().0,
        "representation":match item.representation() { Representation::Dual=>"dual", Representation::MathML=>"mathml" },
        "display":format!("{:?}",item.display()),"scope":item.scope(),"fallback":fallback(item),
        "termination_failure":item.termination_failure(),"observations":observations(item),
        "fixed_assets_sha256":item.assets().map(|a| super::super::digest_hex(a.identity()))
    })).collect::<Vec<_>>();
    let usage = b.usage();
    let manifest = serde_json::to_vec_pretty(&json!({
        "format":"nepl3.trusted-native-doc-export/1",
        "source_sha256":super::super::digest_hex(Digest::of(source.as_bytes())),
        "profile_sha256":super::super::digest_hex(profile),
        "selected_host_config_sha256":super::super::digest_hex(host_digest),
        "host_identity":"trusted selection; executable and bridge identity not attested",
        "math":{"preference":"katex-preferred","native_adapter_configured":true,"occurrences":records},
        "options":{"css":"external","parallel":"Rows"},
        "files":files.iter().map(|(path,mime,bytes)|json!({"path":path,"mime":mime,"sha256":super::super::digest_hex(Digest::of(bytes))})).collect::<Vec<_>>(),
        "usage":{"work":usage.work,"allocation_units":usage.allocation_units,"output_bytes":usage.output_bytes,"nodes":usage.nodes,"depth":usage.depth,"source_bytes":usage.source_bytes,"diagnostics":usage.diagnostics,"events":usage.events},
        "budget_scope":"lower/preparation/assets/native render/import/bundle share one ledger; parser/setup/report serialization/filesystem are separate host work",
        "font":{"doc_family":"Klee One","bundled":false,"offline":"local or system fallback"},
        "viewer_scripts":false,"browser_qualified":false,"portable_identity_qualified":false,
        "publication":"create-only; rollback attempted on I/O failure; not atomic visibility"
    })).map_err(err)?;
    // Source, occurrences and diagnostic bytes have upstream bounds. This final
    // host report cap is not a claim of end-to-end allocation accounting.
    if manifest.len() > 16_000_000 {
        return Err("NativeExportManifestLimit".into());
    }
    write_files(output, &files, &manifest)?;
    for item in bundle.source().math() {
        let why = fallback(item);
        if !why.is_null() {
            eprintln!(
                "MathFallback occurrence {}: {why}",
                item.context().ordinal()
            );
        }
    }
    Ok(())
}

fn write_files(
    output: &Path,
    files: &[(String, &str, &[u8])],
    manifest: &[u8],
) -> Result<(), String> {
    fs::create_dir(output).map_err(err)?;
    let written = (|| -> std::io::Result<()> {
        for (path, _, bytes) in files {
            let path = output.join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, bytes)?;
        }
        fs::write(output.join("manifest.json"), manifest)?;
        Ok(())
    })();
    if let Err(error) = written {
        return match fs::remove_dir_all(output) {
            Ok(()) => Err(format!("NativeExportWrite: {error}")),
            Err(cleanup) => Err(format!(
                "NativeExportWrite: {error}; output cleanup failed: {cleanup}"
            )),
        };
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_output_rolls_back_midwrite_failure_and_preserves_existing() -> Result<(), String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(err)?
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("native-publish-{}-{stamp}", std::process::id()));
        fs::create_dir(&root).map_err(err)?;
        let output = root.join("output");
        // A deterministic filesystem write failure after the first file. This
        // conflicting fixture is not part of any generated asset inventory.
        let files = vec![
            ("assets".to_owned(), "text/plain", b"first".as_slice()),
            ("assets/doc.css".to_owned(), "text/css", b"later".as_slice()),
        ];
        assert!(write_files(&output, &files, b"{}").is_err());
        assert!(!output.exists());
        fs::create_dir(&output).map_err(err)?;
        fs::write(output.join("keep"), b"user data").map_err(err)?;
        assert!(write_files(&output, &files, b"{}").is_err());
        assert_eq!(fs::read(output.join("keep")).map_err(err)?, b"user data");
        fs::remove_dir_all(&root).map_err(err)?;
        Ok(())
    }
}
