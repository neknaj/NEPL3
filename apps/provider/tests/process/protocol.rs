// A dedicated harness keeps libtest's status text off the child's protocol stdout.
#[cfg(not(target_family = "wasm"))]
mod native;

#[cfg(not(target_family = "wasm"))]
fn main() -> Result<(), String> {
    native::run()
}

#[cfg(target_family = "wasm")]
fn main() {
    println!(
        "SKIP process_protocol: native OS process spawning required; transport tests run separately"
    );
}
