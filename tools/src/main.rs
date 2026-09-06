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
        ["foundation", "--write"] => nepl3_tools::foundation(&root),
        ["reader", "--write"] => nepl3_tools::reader(&root),
        ["engine", "--write"] => nepl3_tools::engine(&root),
        ["grammar", "--write"] => nepl3_tools::grammar(&root),
        ["tasks", "--write"] => nepl3_tools::tasks(&root, true),
        ["tasks", "--check"] => nepl3_tools::tasks(&root, false),
        ["evidence", "identity"] => nepl3_tools::evidence_identity(&root),
        ["doc-inventory", "--write", "--commit", commit] => {
            nepl3_tools::doc_inventory(&root, Some(commit), false)
        }
        ["doc-inventory", "--check"] => nepl3_tools::doc_inventory(&root, None, false),
        ["doc-inventory", "--check-current"] => nepl3_tools::doc_inventory(&root, None, true),
        ["--help"] | ["-h"] => {
            println!(
                "nepl3-tools check | tasks --check | tasks --write | evidence identity\n  foundation --write | reader --write | engine --write\n  doc-inventory --write --commit <40-hex-commit> | doc-inventory --check | doc-inventory --check-current\nRepository checks do not establish runtime conformance."
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
