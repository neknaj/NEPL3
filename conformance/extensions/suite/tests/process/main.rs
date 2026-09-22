//! Dedicated harness keeps test status output out of the provider protocol.
#[cfg(not(target_family = "wasm"))]
mod native;

#[cfg(not(target_family = "wasm"))]
fn main() -> Result<(), String> {
    native::run()
}

#[cfg(target_family = "wasm")]
fn main() {
    println!(
        "SKIP process: OS process spawning requires a native host; typed plan tests run on WASI"
    );
}
