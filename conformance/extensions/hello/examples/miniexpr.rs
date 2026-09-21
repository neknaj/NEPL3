use std::{env, process::ExitCode};

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let input = args
        .next()
        .ok_or("usage: miniexpr <input>")?
        .into_string()
        .map_err(|_| "input must be valid Unicode")?;
    if args.next().is_some() {
        return Err("expected one quoted input argument".into());
    }
    let result = external_hello_language::miniexpr::inspect(&input, true)?;
    use nepl3_engine::parse::ParseOutcome;
    match &result.parse.outcome {
        ParseOutcome::Complete { cursor, .. } => println!("Complete; cursor={cursor}"),
        ParseOutcome::Recovered { cursor, .. } => println!("Recovered; cursor={cursor}"),
        ParseOutcome::NeedMore { .. } => println!("NeedMore"),
        ParseOutcome::Stopped { reason, .. } => println!("Stopped: {reason:?}"),
        ParseOutcome::Await { .. } => println!("Await"),
        ParseOutcome::AwaitHead { .. } => println!("AwaitHead"),
        ParseOutcome::Reserve { .. } => println!("Reserve"),
    }
    if let nepl3_engine::parse::ParseOutcome::Complete { tree, .. } = &result.parse.outcome {
        for (index, node) in tree.bundle.nodes.iter().enumerate() {
            println!(
                "node {index}: {}::{} {:?}",
                node.schema.package, node.kind, node.fields
            );
        }
        for token in &tree.bundle.tokens {
            println!(
                "token: {}..{}; payload={:?}",
                token.head.start(),
                token.head.end(),
                token.payload
            );
        }
    }
    for diagnostic in &result.parse.report.diagnostics {
        println!("diagnostic: {diagnostic:#?}");
    }
    if let Some(printed) = result.printed {
        println!("source print: {:?}", printed.outcome);
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
