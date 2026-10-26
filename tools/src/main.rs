use std::{env, path::PathBuf, process::ExitCode};

fn configured_html(
    config: Option<&str>,
    display: Option<&str>,
    args: &[&str],
) -> nepl3_tools::Result<()> {
    use nepl3_tools::doc::export::{
        self, CssMode, MathRenderer, pages::resources::ExecutionLimits,
    };
    let (css, renderer, input, output) = match args {
        [input, output] => (
            CssMode::External,
            MathRenderer::KaTeXPreferred,
            *input,
            *output,
        ),
        ["--css", css, input, output] => {
            (css.parse()?, MathRenderer::KaTeXPreferred, *input, *output)
        }
        ["--math-renderer", renderer, input, output] => {
            (CssMode::External, renderer.parse()?, *input, *output)
        }
        ["--css", css, "--math-renderer", renderer, input, output]
        | ["--math-renderer", renderer, "--css", css, input, output] => {
            (css.parse()?, renderer.parse()?, *input, *output)
        }
        _ => {
            return Err(
                "expected optional --css and --math-renderer followed by input and output".into(),
            );
        }
    };
    if display.is_some() && (input.starts_with("--") || output.starts_with("--")) {
        return Err("expected input and output paths after display options; use ./ for paths beginning with --".into());
    }
    let mut selected = export::DisplayOptions {
        math_renderer: renderer,
        ..export::DisplayOptions::default()
    };
    if let Some(path) = display {
        selected.document = export::DisplaySelection::read(std::path::Path::new(path))?.into();
    }
    let limits = match config {
        Some(path) => ExecutionLimits::read(std::path::Path::new(path))?,
        None => ExecutionLimits::default(),
    };
    export::write_with_display(
        std::path::Path::new(input),
        std::path::Path::new(output),
        css,
        selected,
        limits,
    )
}

fn configured_pages_svg(
    mode: &str,
    renderer: nepl3_tools::doc::export::MathRenderer,
    manifest: &str,
    output: &str,
) -> nepl3_tools::Result<()> {
    let manifest = std::path::Path::new(manifest);
    let output = std::path::Path::new(output);
    match mode {
        "embedded" => nepl3_tools::doc::export::pages::svg::cli::write(manifest, output, renderer),
        "external" => {
            nepl3_tools::doc::export::pages::svg::cli::write_external(manifest, output, renderer)
        }
        _ => Err("SVG mode must be embedded or external".into()),
    }
}

fn configured_pages_svg_interactive(
    mode: &str,
    renderer: nepl3_tools::doc::export::MathRenderer,
    limits: &str,
    manifest: &str,
    output: &str,
) -> nepl3_tools::Result<()> {
    let manifest = std::path::Path::new(manifest);
    let limits = std::path::Path::new(limits);
    let output = std::path::Path::new(output);
    match mode {
        "embedded" => nepl3_tools::doc::export::pages::svg::cli::interactive::write(
            manifest, limits, output, renderer,
        ),
        "external" => nepl3_tools::doc::export::pages::svg::cli::interactive::write_external(
            manifest, limits, output, renderer,
        ),
        _ => Err("SVG mode must be embedded or external".into()),
    }
}

fn configured_svg(display: &str, args: &[&str]) -> nepl3_tools::Result<()> {
    let ["--css", css, "--svg", mode, input, assets, output] = args else {
        return Err("expected --css and --svg followed by input, assets and output".into());
    };
    if [display, *input, *assets, *output]
        .iter()
        .any(|path| path.starts_with("--"))
    {
        return Err(
            "expected display, input, assets and output paths; use ./ for paths beginning with --"
                .into(),
        );
    }
    let selection =
        nepl3_tools::doc::export::DisplaySelection::read(std::path::Path::new(display))?;
    nepl3_tools::doc::export::assets::write_with_display(
        std::path::Path::new(input),
        std::path::Path::new(assets),
        std::path::Path::new(output),
        css.parse()?,
        mode,
        selection,
    )
}

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
        ["signatures", "--check"] => nepl3_tools::doc::signatures::run(&root, false),
        ["signatures", "--write"] => nepl3_tools::doc::signatures::run(&root, true),
        ["site", "build", config, output] => {
            nepl3_tools::site::build(&root, config, std::path::Path::new(output))
        }
        ["doc-canonical", "html-retry-limits"] => nepl3_tools::doc::canonical::html_retry_limits(),
        [
            "doc-canonical",
            "--registry",
            registry,
            "html",
            "--interactive-limits",
            limits,
            output,
        ] => nepl3_tools::doc::canonical::html_interactive(
            &root,
            registry,
            std::path::Path::new(limits),
            std::path::Path::new(output),
        ),
        [
            "doc-canonical",
            "html",
            "--interactive-limits",
            limits,
            output,
        ] => nepl3_tools::doc::canonical::html_interactive(
            &root,
            "doc/canonical.json",
            std::path::Path::new(limits),
            std::path::Path::new(output),
        ),
        ["doc-canonical", "retry-limits"] => nepl3_tools::doc::canonical::retry_limits(),
        [
            "doc-canonical",
            "--registry",
            registry,
            "markdown",
            "--interactive-limits",
            limits,
            output,
        ] => nepl3_tools::doc::canonical::markdown_interactive(
            &root,
            registry,
            std::path::Path::new(limits),
            std::path::Path::new(output),
        ),
        ["doc-canonical", "--check"] => {
            nepl3_tools::doc::canonical::check(&root, "doc/canonical.json")
        }
        ["doc-canonical", "--registry", registry, "--check"] => {
            nepl3_tools::doc::canonical::check(&root, registry)
        }
        [
            "doc-canonical",
            "--registry",
            registry,
            "markdown",
            "--interactive-output-limits",
            limits,
            output,
        ] => nepl3_tools::doc::canonical::markdown_interactive_output(
            &root,
            registry,
            std::path::Path::new(limits),
            std::path::Path::new(output),
        ),
        ["doc-canonical", "--registry", registry, "markdown", output] => {
            nepl3_tools::doc::canonical::markdown(&root, registry, std::path::Path::new(output))
        }
        ["doc-canonical", "--registry", registry, "html", output] => {
            nepl3_tools::doc::canonical::html(&root, registry, std::path::Path::new(output))
        }
        ["doc-canonical", "markdown", output] => nepl3_tools::doc::canonical::markdown(
            &root,
            "doc/canonical.json",
            std::path::Path::new(output),
        ),
        ["doc-canonical", "html", output] => nepl3_tools::doc::canonical::html(
            &root,
            "doc/canonical.json",
            std::path::Path::new(output),
        ),
        ["bootstrap", "seed-check"] => nepl3_tools::bootstrap::cli::seed_check(),
        ["foundation", "--write"] => nepl3_tools::foundation(&root),
        ["reader", "--write"] => nepl3_tools::reader(&root),
        ["engine", "--write"] => nepl3_tools::engine(&root),
        ["grammar", "--write"] => nepl3_tools::grammar(&root),
        ["doc", "--write"] => nepl3_tools::doc(&root),
        ["math", "--write"] => nepl3_tools::math(&root),
        ["sentence", "--write"] => nepl3_tools::sentence(&root),
        ["markup", "--write"] => nepl3_tools::markup(&root),
        ["doc-html", "--write"] => nepl3_tools::doc_html(&root),
        [
            "doc-html",
            "export",
            "--limits",
            limits,
            "--display",
            display,
            tail @ ..,
        ]
        | [
            "doc-html",
            "export",
            "--display",
            display,
            "--limits",
            limits,
            tail @ ..,
        ] => configured_html(Some(limits), Some(display), tail),
        ["doc-html", "export", "--display", display, tail @ ..] => {
            configured_html(None, Some(display), tail)
        }
        ["doc-html", "export", input, output] => nepl3_tools::doc::export::write(
            std::path::Path::new(input),
            std::path::Path::new(output),
        ),
        ["doc-html", "export", "--limits", config, tail @ ..] => {
            configured_html(Some(config), None, tail)
        }
        ["doc-html", "export", "--css", mode, input, output] => {
            let css = mode.parse::<nepl3_tools::doc::export::CssMode>()?;
            nepl3_tools::doc::export::write_with_css(
                std::path::Path::new(input),
                std::path::Path::new(output),
                css,
            )
        }
        [
            "doc-html",
            "export",
            "--math-renderer",
            renderer,
            input,
            output,
        ] => nepl3_tools::doc::export::write_with_renderer(
            std::path::Path::new(input),
            std::path::Path::new(output),
            nepl3_tools::doc::export::CssMode::External,
            renderer.parse()?,
        ),
        [
            "doc-html",
            "export",
            "--css",
            css,
            "--math-renderer",
            renderer,
            input,
            output,
        ]
        | [
            "doc-html",
            "export",
            "--math-renderer",
            renderer,
            "--css",
            css,
            input,
            output,
        ] => nepl3_tools::doc::export::write_with_renderer(
            std::path::Path::new(input),
            std::path::Path::new(output),
            css.parse()?,
            renderer.parse()?,
        ),
        ["doc-html", "svg", "--display", display, tail @ ..] => configured_svg(display, tail),
        [
            "doc-html",
            "svg",
            "--css",
            css,
            "--svg",
            mode,
            input,
            assets,
            output,
        ] => nepl3_tools::doc::export::assets::write(
            std::path::Path::new(input),
            std::path::Path::new(assets),
            std::path::Path::new(output),
            css.parse()?,
            mode,
        ),
        [
            "doc-html",
            "pages",
            "--math-renderer",
            renderer,
            manifest,
            output,
        ] => nepl3_tools::doc::export::pages::write_with_renderer(
            std::path::Path::new(manifest),
            std::path::Path::new(output),
            renderer.parse()?,
        ),
        ["doc-html", "pages-svg", manifest, output] if !manifest.starts_with("--") && !output.starts_with("--") => nepl3_tools::doc::export::pages::svg::cli::write(
            std::path::Path::new(manifest), std::path::Path::new(output), nepl3_tools::doc::export::MathRenderer::KaTeXPreferred,
        ),
        ["doc-html", "pages-svg", "--math-renderer", renderer, manifest, output] if !manifest.starts_with("--") && !output.starts_with("--") => nepl3_tools::doc::export::pages::svg::cli::write(
            std::path::Path::new(manifest), std::path::Path::new(output), renderer.parse()?,
        ),
        ["doc-html", "pages-svg", "--svg", mode, manifest, output] if !manifest.starts_with("--") && !output.starts_with("--") => configured_pages_svg(mode, nepl3_tools::doc::export::MathRenderer::KaTeXPreferred, manifest, output),
        ["doc-html", "pages-svg", "--svg", mode, "--math-renderer", renderer, manifest, output]
        | ["doc-html", "pages-svg", "--math-renderer", renderer, "--svg", mode, manifest, output]
            if !manifest.starts_with("--") && !output.starts_with("--") => configured_pages_svg(mode, renderer.parse()?, manifest, output),
        ["doc-html", "pages-svg", "--interactive-limits", limits, manifest, output] if !limits.starts_with("--") && !manifest.starts_with("--") && !output.starts_with("--") => nepl3_tools::doc::export::pages::svg::cli::interactive::write(
            std::path::Path::new(manifest), std::path::Path::new(limits), std::path::Path::new(output), nepl3_tools::doc::export::MathRenderer::KaTeXPreferred,
        ),
        ["doc-html", "pages-svg", "--interactive-limits", limits, "--math-renderer", renderer, manifest, output] if !limits.starts_with("--") && !manifest.starts_with("--") && !output.starts_with("--") => nepl3_tools::doc::export::pages::svg::cli::interactive::write(
            std::path::Path::new(manifest), std::path::Path::new(limits), std::path::Path::new(output), renderer.parse()?,
        ),
        ["doc-html", "pages-svg", "--interactive-limits", limits, "--svg", mode, manifest, output]
            if !limits.starts_with("--") && !manifest.starts_with("--") && !output.starts_with("--") => configured_pages_svg_interactive(mode, nepl3_tools::doc::export::MathRenderer::KaTeXPreferred, limits, manifest, output),
        ["doc-html", "pages-svg", "--interactive-limits", limits, "--svg", mode, "--math-renderer", renderer, manifest, output]
        | ["doc-html", "pages-svg", "--interactive-limits", limits, "--math-renderer", renderer, "--svg", mode, manifest, output]
            if !limits.starts_with("--") && !manifest.starts_with("--") && !output.starts_with("--") => configured_pages_svg_interactive(mode, renderer.parse()?, limits, manifest, output),
        ["doc-html", "pages-svg", ..] => Err("usage: doc-html pages-svg [--interactive-limits <limits.json>] [--svg embedded|external] [--math-renderer katex-preferred|mathml-only] <manifest.json> <new-output-directory>; --interactive-limits must precede renderer/SVG options; durable restoration is not implemented".into()),
        ["doc-html", "retry-limits"] => nepl3_tools::doc::export::pages::interactive::defaults(),
        [
            "doc-html",
            "pages",
            "--interactive-limits",
            limits,
            manifest,
            output,
        ] => nepl3_tools::doc::export::pages::interactive::write(
            std::path::Path::new(manifest),
            std::path::Path::new(limits),
            std::path::Path::new(output),
            nepl3_tools::doc::export::MathRenderer::KaTeXPreferred,
        ),
        [
            "doc-html",
            "pages",
            "--interactive-limits",
            limits,
            "--math-renderer",
            renderer,
            manifest,
            output,
        ] => nepl3_tools::doc::export::pages::interactive::write(
            std::path::Path::new(manifest),
            std::path::Path::new(limits),
            std::path::Path::new(output),
            renderer.parse()?,
        ),
        ["doc-html", "pages", manifest, output] => nepl3_tools::doc::export::pages::write(
            std::path::Path::new(manifest),
            std::path::Path::new(output),
        ),
        ["doc-markdown", input, output] => nepl3_tools::doc::projection::write(
            std::path::Path::new(input),
            std::path::Path::new(output),
        ),
        ["doc-markdown", "footnotes", input, aliases, output] => {
            nepl3_tools::doc::projection::annotated::host::write_footnotes(
                std::path::Path::new(input),
                std::path::Path::new(aliases),
                std::path::Path::new(output),
            )
        }
        [
            "doc-markdown",
            mode,
            "--limits",
            config,
            input,
            aliases,
            output,
        ] if matches!(*mode, "annotated" | "footnotes") => {
            use nepl3_tools::doc::projection::annotated::host;
            host::write_with_limits(
                std::path::Path::new(input),
                std::path::Path::new(aliases),
                std::path::Path::new(output),
                mode,
                nepl3_tools::doc::export::pages::resources::ExecutionLimits::read(
                    std::path::Path::new(config),
                )?,
            )
        }
        ["doc-markdown", "footnotes-pages", manifest, output] => {
            nepl3_tools::doc::canonical::footnotes_manifest(
                std::path::Path::new(manifest),
                std::path::Path::new(output),
            )
        }
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
        ["evidence", "identity", "--commit", commit] => {
            nepl3_tools::evidence_committed_identity(&root, commit)
        }
        ["doc-inventory", "--write", "--commit", commit] => {
            nepl3_tools::doc_inventory(&root, Some(commit), false)
        }
        ["doc-inventory", "--check"] => nepl3_tools::doc_inventory(&root, None, false),
        ["doc-inventory", "--check-current"] => nepl3_tools::doc_inventory(&root, None, true),
        ["--help"] | ["-h"] => {
            println!("nepl3-tools signatures --check | signatures --write");
            println!("nepl3-tools site build <site-config.json> <new-output-directory>");
            println!("nepl3-tools doc-html retry-limits");
            println!(
                "nepl3-tools doc-html pages --interactive-limits <limits.json> [--math-renderer katex-preferred|mathml-only] <manifest.json> <new-output-directory>"
            );
            println!("nepl3-tools doc-canonical retry-limits");
            println!("nepl3-tools doc-canonical html-retry-limits");
            println!(
                "nepl3-tools doc-canonical [--registry <repository-relative.json>] html --interactive-limits <limits.json> <new-output-directory>"
            );
            println!(
                "nepl3-tools doc-canonical --registry <repository-relative.json> markdown --interactive-limits <limits.json> <new-output-directory>"
            );
            println!("nepl3-tools doc-canonical --check");
            println!(
                "nepl3-tools doc-html export --limits <limits.json> [--css <external|inline>] [--math-renderer <katex-preferred|mathml-only>] <input.nepld> <new-output-directory>"
            );
            println!(
                "nepl3-tools doc-html export --display <display.json> [--limits <limits.json>] [--css <external|inline>] [--math-renderer <katex-preferred|mathml-only>] <input.nepld> <new-output-directory>"
            );
            println!(
                "--display and --limits may appear in either leading order; display uses rows, columns, or single with language and ordered fallbacks."
            );
            println!(
                "nepl3-tools doc-html svg --display <display.json> --css <external|inline> --svg <external|embedded> <input.nepld> <assets.json> <new-output-directory>"
            );
            println!(
                "nepl3-tools doc-markdown <annotated|footnotes> --limits <limits.json> <input.nepld> <aliases.json> <new-output.md>"
            );
            println!("nepl3-tools doc-canonical --registry <repository-relative.json> --check");
            println!(
                "nepl3-tools doc-canonical --registry <repository-relative.json> markdown <new-output-directory>"
            );
            println!(
                "nepl3-tools doc-canonical --registry <repository-relative.json> html <new-output-directory>"
            );
            println!(
                "nepl3-tools doc-canonical --registry <repository-relative.json> markdown --interactive-output-limits <limits.json> <new-output-directory>"
            );
            println!("nepl3-tools doc-canonical markdown <new-output-directory>");
            println!("nepl3-tools doc-canonical html <new-output-directory>");
            println!("nepl3-tools doc-markdown <input.nepld> <new-output.md>");
            println!(
                "nepl3-tools doc-markdown footnotes <input.nepld> <aliases.json> <new-output.md>"
            );
            println!(
                "nepl3-tools doc-markdown footnotes-pages <manifest.json> <new-output-directory>"
            );
            println!(
                "nepl3-tools doc-markdown annotated <input.nepld> <aliases.json> <new-output.md>"
            );
            println!(
                "nepl3-tools check | tasks --check | tasks --write | evidence identity [--commit <40-hex-commit>]\n  foundation --write | reader --write | engine --write | grammar --write | doc --write | math --write | sentence --write | markup --write | doc-html --write\n  doc-html export [--css external|inline] [--math-renderer katex-preferred|mathml-only] <input.nepld> <new-output-directory>\n  doc-html svg --css external|inline --svg external|embedded <input.nepld> <assets.json> <new-output-directory>\n  doc-html pages [--math-renderer katex-preferred|mathml-only] <manifest.json> <new-output-directory>\n  doc-html pages-svg [--svg embedded|external] [--math-renderer katex-preferred|mathml-only] <manifest.json> <new-output-directory>\n  doc-html pages-svg --interactive-limits <limits.json> [--svg embedded|external] [--math-renderer katex-preferred|mathml-only] <manifest.json> <new-output-directory> (durable restoration unsupported)\n  doc-inventory --write --commit <40-hex-commit> | doc-inventory --check | doc-inventory --check-current\nRepository checks do not establish runtime conformance."
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
