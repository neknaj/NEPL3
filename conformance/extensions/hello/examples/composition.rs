use nepl3_engine::parse::ParseOutcome;
use std::{env, process::ExitCode};

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let input = args
        .next()
        .ok_or("usage: composition <input>")?
        .into_string()
        .map_err(|_| "input must be valid Unicode")?;
    if args.next().is_some() {
        return Err("expected one quoted input argument".into());
    }
    let result = external_hello_language::composition::inspect(&input, true)?;
    match &result.parse.outcome {
        ParseOutcome::Complete { tree, cursor, .. }
        | ParseOutcome::Recovered { tree, cursor, .. } => {
            let state = if matches!(result.parse.outcome, ParseOutcome::Complete { .. }) {
                "Complete"
            } else {
                "Recovered"
            };
            println!("{state}; cursor={cursor}");
            for context in &tree.contexts {
                println!("bundle path: {:?}", context.path);
                for node in &context.nodes {
                    println!(
                        "node {}: alias={}; category={}; mode={}; shape={:?}",
                        node.node.0,
                        node.entry.alias,
                        node.entry.category,
                        node.entry.mode,
                        node.shape
                    );
                }
            }
        }
        other => println!("outcome: {other:?}"),
    }
    for diagnostic in result.parse.report.diagnostics {
        println!("diagnostic: {diagnostic:?}");
    }
    if let Some(printed) = result.printed {
        println!("source print: {:?}", printed.outcome);
    }
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
