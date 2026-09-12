//! Record the compiler actually used for this host tool, not a later PATH lookup.
use std::{env, error::Error, process::Command};

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
    Ok(())
}
