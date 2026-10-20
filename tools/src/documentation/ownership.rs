//! Resolve declared document ownership from the same immutable Git snapshot.
//! Registry identity is not evidence that a projection was rendered or deployed.
use super::*;

pub(super) struct Owner {
    pub source: String,
    pub registry: String,
    pub aliases: String,
    pub id: String,
    pub route: String,
    pub renderer: String,
}

pub(super) fn is_registry(path: &str) -> bool {
    path == "doc/canonical.json"
        || path.starts_with("examples/document/") && path.ends_with(".pages.json")
}

pub(super) fn collect(
    root: &Path,
    commit: &str,
    paths: &BTreeSet<String>,
) -> Result<BTreeMap<String, Owner>> {
    let mut owners = BTreeMap::new();
    for manifest in paths.iter().filter(|path| is_registry(path)) {
        let registry = crate::doc::canonical::parse_registry(&blob(root, commit, manifest)?)?;
        // Repository canonical paths are root-relative. Standalone example
        // manifests are resolved relative to their own directory, as exporters do.
        let prefix = if manifest == "doc/canonical.json" {
            ""
        } else {
            manifest
                .rsplit_once('/')
                .ok_or("registry parent missing")?
                .0
        };
        let resolve = |path: &str| {
            if prefix.is_empty() {
                path.to_owned()
            } else {
                format!("{prefix}/{path}")
            }
        };
        for page in registry.pages {
            let source = resolve(&page.source);
            let projection = resolve(&page.projection);
            let aliases = resolve(&page.aliases);
            for path in [&source, &projection, &aliases] {
                if !paths.contains(path) {
                    return Err(format!(
                        "{manifest}: registered input missing from baseline: {path}"
                    )
                    .into());
                }
            }
            if owners
                .insert(
                    projection.clone(),
                    Owner {
                        source,
                        registry: manifest.clone(),
                        aliases,
                        id: page.id,
                        route: page.route,
                        renderer: page.renderer,
                    },
                )
                .is_some()
            {
                return Err(
                    format!("multiple canonical owners for projection: {projection}").into(),
                );
            }
        }
    }
    Ok(owners)
}
