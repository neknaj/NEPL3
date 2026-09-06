use std::{env, path::PathBuf, process::ExitCode};

fn run() -> nepl3_tools::Result<()> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("tools manifest has no workspace parent")?
        .to_path_buf();
    match arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["check"] => nepl3_tools::check(&root),
        ["tasks", "--write"] => nepl3_tools::tasks(&root, true),
        ["tasks", "--check"] => nepl3_tools::tasks(&root, false),
        ["evidence", "identity"] => nepl3_tools::evidence_identity(&root),
        ["--help"] | ["-h"] => {
            println!(
                "nepl3-tools check | tasks --check | tasks --write | evidence identity\nRepository checks only; runtime conformance is not implemented."
            );
            Ok(())
        }
        _ => Err(
            "expected: check | tasks --check | tasks --write | evidence identity (see --help)"
                .into(),
        ),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("nepl3-tools: {error}");
            ExitCode::FAILURE
        }
    }
}
