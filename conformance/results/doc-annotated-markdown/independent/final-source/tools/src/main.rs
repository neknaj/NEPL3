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
        ["bootstrap", "seed-check"] => nepl3_tools::bootstrap::cli::seed_check(),
        ["foundation", "--write"] => nepl3_tools::foundation(&root),
        ["reader", "--write"] => nepl3_tools::reader(&root),
        ["engine", "--write"] => nepl3_tools::engine(&root),
        ["grammar", "--write"] => nepl3_tools::grammar(&root),
        ["doc", "--write"] => nepl3_tools::doc(&root),
        ["math", "--write"] => nepl3_tools::math(&root),
        ["markup", "--write"] => nepl3_tools::markup(&root),
        ["doc-html", "--write"] => nepl3_tools::doc_html(&root),
        ["doc-html", "export", input, output] => nepl3_tools::doc::export::write(
            std::path::Path::new(input),
            std::path::Path::new(output),
        ),
        ["doc-html", "pages", manifest, output] => nepl3_tools::doc::export::pages::write(
            std::path::Path::new(manifest),
            std::path::Path::new(output),
        ),
        ["doc-markdown", input, output] => nepl3_tools::doc::projection::write(
            std::path::Path::new(input),
            std::path::Path::new(output),
        ),
        ["doc-markdown", "annotated", input, aliases, output] => {
            nepl3_tools::doc::projection::annotated::host::write(
                std::path::Path::new(input),
                std::path::Path::new(aliases),
                std::path::Path::new(output),
            )
        }
        ["tasks", "--write"] => nepl3_tools::tasks(&root, true),
        ["tasks", "--check"] => nepl3_tools::tasks(&root, false),
        ["evidence", "identity"] => nepl3_tools::evidence_identity(&root),
        ["doc-inventory", "--write", "--commit", commit] => {
            nepl3_tools::doc_inventory(&root, Some(commit), false)
        }
        ["doc-inventory", "--check"] => nepl3_tools::doc_inventory(&root, None, false),
        ["doc-inventory", "--check-current"] => nepl3_tools::doc_inventory(&root, None, true),
        ["--help"] | ["-h"] => {
            println!("nepl3-tools doc-markdown <input.nepld> <new-output.md>");
            println!(
                "nepl3-tools doc-markdown annotated <input.nepld> <aliases.json> <new-output.md>"
            );
            println!(
                "nepl3-tools check | tasks --check | tasks --write | evidence identity\n  foundation --write | reader --write | engine --write | grammar --write | doc --write | math --write | markup --write | doc-html --write\n  doc-html export <input.nepld> <new-output-directory>\n  doc-html pages <manifest.json> <new-output-directory>\n  doc-inventory --write --commit <40-hex-commit> | doc-inventory --check | doc-inventory --check-current\nRepository checks do not establish runtime conformance."
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
