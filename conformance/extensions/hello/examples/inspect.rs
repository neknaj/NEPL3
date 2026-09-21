use nepl3_engine::{parse::ParseOutcome, recovery::ParseTree};
use std::{env, process::ExitCode};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args_os()
        .skip(1)
        .map(|value| {
            value
                .into_string()
                .map_err(|_| "input argument must be valid Unicode".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut arguments = arguments.into_iter();
    let first = arguments
        .next()
        .ok_or("usage: inspect [--partial] <input>")?;
    let (input, final_input) = if first == "--partial" {
        (
            arguments.next().ok_or("missing input after --partial")?,
            false,
        )
    } else {
        (first, true)
    };
    if arguments.next().is_some() {
        return Err("expected one input argument; quote input containing spaces".into());
    }
    let reply = external_hello_language::parse(&input, final_input)?;
    match &reply.outcome {
        ParseOutcome::Complete { tree, cursor, .. } => {
            println!("Complete; cursor={cursor}");
            print_tree(tree);
        }
        ParseOutcome::Recovered { tree, cursor, .. } => {
            println!("Recovered; cursor={cursor}");
            print_tree(tree);
        }
        outcome => println!("{outcome:#?}"),
    }
    for diagnostic in &reply.report.diagnostics {
        println!("diagnostic: {diagnostic:#?}");
    }
    Ok(())
}

fn print_tree(tree: &ParseTree) {
    println!("root: {:?}", tree.bundle.root);
    for (index, node) in tree.bundle.nodes.iter().enumerate() {
        println!(
            "node {index}: {}::{} {:?}",
            node.schema.package, node.kind, node.fields
        );
    }
    for token in &tree.bundle.tokens {
        println!(
            "token: {:?}; span={:?}; payload={:?}",
            token.head,
            (token.head.start(), token.head.end()),
            token.payload
        );
    }
    for (index, origin) in tree.bundle.origins.iter().enumerate() {
        println!("origin {index}: {origin:?}");
    }
}
