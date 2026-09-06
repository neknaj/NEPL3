use crate::{
    Result, command, json, read,
    task::{dag, unique},
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    design_revision: String,
    edge_direction: String,
    workspace: Vec<Crate>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Crate {
    name: String,
    path: String,
    dependencies: Vec<String>,
    no_std: bool,
    responsibility: String,
}

// Cargo explicitly permits additional metadata fields as the format evolves.
#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    manifest_path: String,
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
    kind: Option<String>,
    path: Option<String>,
}

fn validate_plan(plan: &Plan) -> Result<()> {
    if plan.edge_direction != "consumer -> dependency" {
        return Err("unexpected dependency direction".into());
    }
    unique(
        plan.workspace.iter().map(|c| c.name.as_str()),
        "dependency plan names",
    )?;
    unique(
        plan.workspace.iter().map(|c| c.path.as_str()),
        "dependency plan paths",
    )?;
    let graph: BTreeMap<_, _> = plan
        .workspace
        .iter()
        .map(|c| {
            (
                c.name.as_str(),
                c.dependencies.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    dag(&graph, "crate dependency plan")?;
    let domains = ["nepl3-doc-core", "nepl3-math-core", "nepl3-circuit-core"];
    for domain in domains {
        let mut pending = vec![domain];
        let mut visited = BTreeSet::new();
        while let Some(node) = pending.pop() {
            if !visited.insert(node) {
                continue;
            }
            if node != domain && domains.contains(&node) {
                return Err(
                    format!("forbidden transitive domain dependency: {domain} -> {node}").into(),
                );
            }
            if let Some(dependencies) = graph.get(node) {
                pending.extend(dependencies);
            }
        }
    }
    for krate in &plan.workspace {
        if krate.responsibility.is_empty() {
            return Err(format!("{}: missing responsibility", krate.name).into());
        }
        for name in &krate.dependencies {
            let dep = plan
                .workspace
                .iter()
                .find(|c| &c.name == name)
                .ok_or("unknown planned dependency")?;
            if krate.path.starts_with("crates/")
                && (dep.path.starts_with("apps/")
                    || dep.path == "tools"
                    || dep.path.starts_with("tools/"))
            {
                return Err(format!(
                    "production dependency on host entry point: {} -> {name}",
                    krate.name
                )
                .into());
            }
            if krate.no_std && !dep.no_std {
                return Err(format!("{}: no_std depends on host crate {name}", krate.name).into());
            }
            let domains = ["nepl3-doc-core", "nepl3-math-core", "nepl3-circuit-core"];
            if domains.contains(&krate.name.as_str()) && domains.contains(&name.as_str()) {
                return Err(
                    format!("forbidden domain dependency: {} -> {name}", krate.name).into(),
                );
            }
        }
    }
    Ok(())
}

fn actual_edges(package: &Package, policy: &Crate, names: &BTreeSet<&str>) -> Result<Vec<String>> {
    let mut development = Vec::new();
    for dependency in &package.dependencies {
        if dependency.kind.as_deref() == Some("dev") {
            development.push(format!("{} -> {}", package.name, dependency.name));
            continue;
        }
        if !matches!(dependency.kind.as_deref(), None | Some("build")) {
            return Err("unknown Cargo dependency kind".into());
        }
        // Cargo `name` is the package name even when the manifest uses `package =`.
        if names.contains(dependency.name.as_str()) {
            if dependency.path.is_none() || !policy.dependencies.contains(&dependency.name) {
                return Err(format!(
                    "unapproved workspace dependency: {} -> {} ({:?})",
                    package.name, dependency.name, dependency.kind
                )
                .into());
            }
        } else if dependency.path.is_some() {
            return Err(format!(
                "local dependency outside dependency plan: {} -> {}",
                package.name, dependency.name
            )
            .into());
        }
    }
    Ok(development)
}

pub(crate) fn check(root: &Path, implemented: &[String]) -> Result<()> {
    let plan: Plan = json(root, "design/dependencies.json")?;
    let tasks: crate::task::TaskFile = json(root, "design/tasks.json")?;
    if plan.design_revision != tasks.design {
        return Err("dependency plan revision differs from tasks".into());
    }
    validate_plan(&plan)?;
    let metadata: Metadata = serde_json::from_slice(&command(
        root,
        "cargo",
        &["metadata", "--locked", "--no-deps", "--format-version", "1"],
    )?)?;
    let members = unique(
        metadata.workspace_members.iter().map(String::as_str),
        "Cargo members",
    )?;
    let packages: Vec<_> = metadata
        .packages
        .iter()
        .filter(|p| members.contains(p.id.as_str()))
        .collect();
    let names = unique(
        packages.iter().map(|p| p.name.as_str()),
        "Cargo member names",
    )?;
    if names != unique(implemented.iter().map(String::as_str), "implemented crates")? {
        return Err("implemented_crates differs from Cargo workspace members".into());
    }
    let planned_names = unique(
        plan.workspace.iter().map(|c| c.name.as_str()),
        "planned crates",
    )?;
    let mut dev = Vec::new();
    let mut core_count = 0;
    for package in packages {
        let policy = plan
            .workspace
            .iter()
            .find(|c| c.name == package.name)
            .ok_or_else(|| format!("unplanned workspace member: {}", package.name))?;
        if root.join(&policy.path).join("Cargo.toml").canonicalize()?
            != Path::new(&package.manifest_path).canonicalize()?
        {
            return Err(format!(
                "{}: manifest path differs from dependency plan",
                package.name
            )
            .into());
        }
        dev.extend(actual_edges(package, policy, &planned_names)?);
        for dependency in &package.dependencies {
            if dependency.kind.as_deref() == Some("dev") {
                continue;
            }
            if let Some(path) = &dependency.path {
                let expected = plan
                    .workspace
                    .iter()
                    .find(|c| c.name == dependency.name)
                    .ok_or("unplanned local dependency")?;
                if Path::new(path).canonicalize()? != root.join(&expected.path).canonicalize()? {
                    return Err(format!(
                        "{}: dependency {} resolves outside its planned path",
                        package.name, dependency.name
                    )
                    .into());
                }
            }
        }
        if policy.no_std {
            core_count += 1;
            let source = read(root, &format!("{}/src/lib.rs", policy.path))?;
            if !source.lines().any(|line| line.trim() == "#![no_std]")
                || !source
                    .lines()
                    .any(|line| line.trim() == "extern crate alloc;")
            {
                return Err(format!(
                    "{}: requires unconditional no_std and alloc declarations",
                    package.name
                )
                .into());
            }
            // Source declarations alone do not establish external-crate portability.
            command(
                root,
                "cargo",
                &[
                    "check",
                    "--locked",
                    "-p",
                    &package.name,
                    "--lib",
                    "--no-default-features",
                ],
            )?;
        }
    }
    println!(
        "Dependency plan: {} crates; {} implemented members; {core_count} no_std declarations checked. Dev dependencies: {}.",
        plan.workspace.len(),
        names.len(),
        if dev.is_empty() {
            "none".to_owned()
        } else {
            dev.join(", ")
        }
    );
    println!(
        "External dependency portability and cross-target execution require their own acceptance tests."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn krate(name: &str, path: &str, dependencies: &[&str], no_std: bool) -> Crate {
        Crate {
            name: name.into(),
            path: path.into(),
            dependencies: dependencies.iter().map(|s| (*s).into()).collect(),
            no_std,
            responsibility: "test".into(),
        }
    }

    #[test]
    fn production_host_boundary_does_not_depend_on_no_std_flag() {
        let plan = Plan {
            design_revision: "test".into(),
            edge_direction: "consumer -> dependency".into(),
            workspace: vec![
                krate("production", "crates/production", &["tool"], false),
                krate("tool", "tools", &[], false),
            ],
        };
        assert!(validate_plan(&plan).is_err());
    }

    #[test]
    fn indirect_domain_dependency_is_rejected() {
        let plan = Plan {
            design_revision: "test".into(),
            edge_direction: "consumer -> dependency".into(),
            workspace: vec![
                krate("nepl3-doc-core", "crates/doc", &["helper"], true),
                krate("helper", "crates/helper", &["nepl3-math-core"], true),
                krate("nepl3-math-core", "crates/math", &[], true),
            ],
        };
        assert!(validate_plan(&plan).is_err());
    }

    #[test]
    fn renamed_build_dependency_is_checked_by_package_name() -> Result<()> {
        // Cargo emits name=actual regardless of the import alias.
        let package: Package = serde_json::from_str(
            r#"{"id":"p","name":"consumer","manifest_path":"x","dependencies":[{"name":"actual","rename":"innocent","kind":"build","path":"x"}]}"#,
        )?;
        let policy = Crate {
            name: "consumer".into(),
            path: "consumer".into(),
            dependencies: vec![],
            no_std: false,
            responsibility: "test".into(),
        };
        assert!(actual_edges(&package, &policy, &BTreeSet::from(["actual", "consumer"])).is_err());
        Ok(())
    }

    #[test]
    fn development_edges_are_reported_separately() -> Result<()> {
        let package: Package = serde_json::from_str(
            r#"{"id":"p","name":"consumer","manifest_path":"x","dependencies":[{"name":"test-helper","kind":"dev","path":"x"}]}"#,
        )?;
        let policy = Crate {
            name: "consumer".into(),
            path: "consumer".into(),
            dependencies: vec![],
            no_std: false,
            responsibility: "test".into(),
        };
        assert_eq!(
            actual_edges(&package, &policy, &BTreeSet::new())?,
            vec!["consumer -> test-helper"]
        );
        Ok(())
    }
}
