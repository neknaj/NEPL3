//! Record the compiler actually used for this host tool, not a later PATH lookup.
use std::{env, error::Error, fs, path::PathBuf, process::Command};
#[path = "build_support/host_identity.rs"]
mod host_identity;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=RUSTC");
    let output = Command::new(env::var_os("RUSTC").ok_or("missing RUSTC")?)
        .arg("--version")
        .output()?;
    if !output.status.success() {
        return Err("cannot identify the build compiler".into());
    }
    let version = std::str::from_utf8(&output.stdout)?.trim();
    if version.is_empty() || version.contains(['\r', '\n']) {
        return Err("invalid build compiler identity".into());
    }
    println!("cargo:rustc-env=NEPL3_BUILD_RUSTC={version}");
    let manifest =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or("missing manifest directory")?);
    let root = manifest.parent().ok_or("missing workspace root")?;
    for input in host_identity::ROOTS.iter().chain(host_identity::FILES) {
        // Directory watches also invalidate additions, removals and renames.
        println!("cargo:rerun-if-changed={}", root.join(input).display());
    }
    let digest = host_identity::fingerprint(root)?;
    let output = PathBuf::from(env::var_os("OUT_DIR").ok_or("missing build output directory")?);
    fs::write(
        output.join("doc_host_source_digest.rs"),
        format!("{digest:?}"),
    )?;
    Ok(())
}
