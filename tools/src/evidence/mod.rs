mod identity;

pub(crate) use identity::{Identity, Snapshot, identity};

use crate::{Result, json, repository::local_path, task::unique};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Catalog {
    schema: String,
    pub design_revision: String,
    targets: BTreeMap<String, Target>,
    pub groups: Vec<Group>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    kind: TargetKind,
    description: String,
}

#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum TargetKind {
    Command,
    Review,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Group {
    pub id: String,
    pub spec: String,
    pub required: bool,
    required_targets: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    schema: String,
    acceptance_id: String,
    design_revision: String,
    identity: Identity,
    result: Outcome,
    runs: Vec<Run>,
}

#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Outcome {
    Passed,
    Failed,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Run {
    Command {
        command: String,
        target: String,
        result: Outcome,
        exit_code: i32,
        checks: Vec<String>,
        environment: Environment,
        log: String,
        log_sha256: String,
    },
    Review {
        target: String,
        reviewer: String,
        independent: bool,
        decision: Decision,
        scope: Vec<String>,
        log: String,
        log_sha256: String,
    },
}

#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Decision {
    Approved,
    Rejected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Version {
    name: String,
    version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Environment {
    runner: Version,
    tools: Vec<Version>,
}

impl Run {
    fn validate<'a>(
        &'a self,
        catalog: &Catalog,
        id: &str,
    ) -> Result<(&'a str, &'a str, &'a str, bool)> {
        let (target, log, digest, failure, kind) = match self {
            Self::Command {
                command,
                target,
                result,
                exit_code,
                checks,
                environment,
                log,
                log_sha256,
            } => {
                if command.trim().is_empty() {
                    return Err(format!("{id}: missing execution command").into());
                }
                nonempty_items(checks, "run checks")?;
                if (*result == Outcome::Passed) != (*exit_code == 0) {
                    return Err(format!("{id}: run result contradicts exit code").into());
                }
                if environment.tools.is_empty() {
                    return Err(format!("{id}: execution tool versions are required").into());
                }
                unique(
                    environment.tools.iter().map(|v| v.name.as_str()),
                    "execution tools",
                )?;
                for version in std::iter::once(&environment.runner).chain(&environment.tools) {
                    if version.name.trim().is_empty() || version.version.trim().is_empty() {
                        return Err(format!(
                            "{id}: actual runner/tool names and versions are required"
                        )
                        .into());
                    }
                }
                (
                    target,
                    log,
                    log_sha256,
                    *result == Outcome::Failed,
                    TargetKind::Command,
                )
            }
            Self::Review {
                target,
                reviewer,
                independent,
                decision,
                scope,
                log,
                log_sha256,
            } => {
                if reviewer.trim().is_empty() || !independent {
                    return Err(format!(
                        "{id}: review requires an identified independent reviewer"
                    )
                    .into());
                }
                nonempty_items(scope, "review scope")?;
                (
                    target,
                    log,
                    log_sha256,
                    *decision == Decision::Rejected,
                    TargetKind::Review,
                )
            }
        };
        let expected = catalog
            .targets
            .get(target)
            .ok_or_else(|| format!("{id}: unknown target {target}"))?;
        if expected.kind != kind {
            return Err(format!("{id}: evidence kind does not match target {target}").into());
        }
        Ok((target, log, digest, failure))
    }
}

fn nonempty_items(items: &[String], context: &str) -> Result<()> {
    if items.is_empty() || items.iter().any(|s| s.trim().is_empty()) {
        return Err(format!("{context}: concrete nonempty entries required").into());
    }
    unique(items.iter().map(String::as_str), context)?;
    Ok(())
}

pub(crate) fn catalog(root: &Path, design: &str) -> Result<Catalog> {
    let catalog: Catalog = json(root, "design/acceptance.json")?;
    if catalog.schema != "nepl3.acceptance-catalog/1" || catalog.design_revision != design {
        return Err("acceptance catalog schema or design revision mismatch".into());
    }
    if catalog.targets.is_empty() || catalog.groups.is_empty() {
        return Err("acceptance catalog cannot be empty".into());
    }
    for (id, target) in &catalog.targets {
        if id.trim().is_empty() || target.description.trim().is_empty() {
            return Err("target ID and description must be nonempty".into());
        }
    }
    unique(
        catalog.groups.iter().map(|g| g.id.as_str()),
        "acceptance catalog groups",
    )?;
    for group in &catalog.groups {
        if !group.spec.starts_with("doc/spec/") || !local_path(root, &group.spec)?.is_file() {
            return Err(format!("{}: invalid acceptance spec", group.id).into());
        }
        if group.required_targets.is_empty() {
            return Err(format!("{}: no required targets", group.id).into());
        }
        for target in unique(
            group.required_targets.iter().map(String::as_str),
            "required targets",
        )? {
            if !catalog.targets.contains_key(target) {
                return Err(format!("{}: unknown target {target}", group.id).into());
            }
        }
    }
    Ok(catalog)
}

/// Validate every supplied report, including its logs, against the current snapshot.
/// One report represents one full attempt; reports cannot combine partial passes.
pub(crate) fn acceptance(
    root: &Path,
    catalog: &Catalog,
    snapshot: &Snapshot,
    id: &str,
    state: &str,
    paths: &[String],
) -> Result<()> {
    let outcome = match state {
        "passed" => Outcome::Passed,
        "failed" => Outcome::Failed,
        "not-run" | "blocked" => {
            if !paths.is_empty() {
                return Err(
                    format!("{id}: unexecuted acceptance cannot carry execution evidence").into(),
                );
            }
            return Ok(());
        }
        _ => return Err(format!("{id}: unknown acceptance state").into()),
    };
    if paths.is_empty() {
        return Err(format!("{id}: executed acceptance requires evidence").into());
    }
    let group = catalog
        .groups
        .iter()
        .find(|g| g.id == id)
        .ok_or("acceptance group absent from catalog")?;
    for path in paths {
        result_path(root, path)?;
        let evidence: Evidence = json(root, path)?;
        if evidence.schema != "nepl3.acceptance-evidence/1"
            || evidence.acceptance_id != id
            || evidence.result != outcome
        {
            return Err(format!("{id}: evidence schema, acceptance ID, or result mismatch").into());
        }
        if evidence.design_revision != snapshot.design_revision
            || evidence.identity != snapshot.identity
        {
            return Err(format!("{id}: stale design/source/spec identity").into());
        }
        if evidence.runs.is_empty() {
            return Err(format!("{id}: evidence has no executed runs").into());
        }
        let mut coverage = BTreeSet::new();
        let mut failure = false;
        for run in &evidence.runs {
            let (target, log_path, log_digest, failed) = run.validate(catalog, id)?;
            if !matches!(
                Path::new(log_path).extension().and_then(|s| s.to_str()),
                Some("txt" | "log")
            ) {
                return Err(format!(
                    "{id}: execution log must be a .txt or .log file, separate from evidence JSON"
                )
                .into());
            }
            let log = fs::read(result_path(root, log_path)?)?;
            if log.is_empty()
                || !identity::is_digest(log_digest)
                || identity::hex(&Sha256::digest(&log)) != log_digest
            {
                return Err(
                    format!("{id}: empty, malformed, or mismatched execution log digest").into(),
                );
            }
            coverage.insert(target);
            failure |= failed;
        }
        if outcome == Outcome::Passed
            && (failure
                || group
                    .required_targets
                    .iter()
                    .any(|target| !coverage.contains(target.as_str())))
        {
            return Err(format!(
                "{id}: passed evidence requires successful runs on every required target"
            )
            .into());
        }
        if outcome == Outcome::Failed && !failure {
            return Err(format!("{id}: failed evidence requires a failing run").into());
        }
    }
    Ok(())
}

fn result_path(root: &Path, path: &str) -> Result<std::path::PathBuf> {
    if !path.starts_with("conformance/results/") {
        return Err("evidence and logs must be under conformance/results/".into());
    }
    let full = local_path(root, path)?;
    if !full.is_file() || fs::symlink_metadata(&full)?.file_type().is_symlink() {
        return Err("evidence and logs must be regular files".into());
    }
    if fs::metadata(&full)?.len() > 1024 * 1024 {
        return Err("evidence/log exceeds 1 MiB repository limit".into());
    }
    Ok(full)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{task, testing::Fixture};
    use serde_json::{Value, json};

    const REPORT: &str = "conformance/results/A01.json";

    fn fixture() -> Result<(Fixture, Value)> {
        let files = Fixture::new()?;
        files.git()?;
        files.write(".gitignore", ".tmp/\ntarget/\n")?;
        files.json("design/tasks.json", &json!({"design":"test-r1","tasks":[{"id":"T01","title":"fixture","depends_on":[],"acceptance":["A01"],"deliverable":"fixture","spec":"doc/spec/11-conformance.md"}]}))?;
        files.write(
            "doc/spec/11-conformance.md",
            "A01: independent test fixture acceptance\n",
        )?;
        files.json("design/review.json", &json!({"findings":[]}))?;
        files.json("design/acceptance.json", &json!({"schema":"nepl3.acceptance-catalog/1","design_revision":"test-r1","targets":{"native":{"kind":"command","description":"native fixture"},"browser":{"kind":"command","description":"browser fixture"}},"groups":[{"id":"A01","spec":"doc/spec/11-conformance.md","required":true,"required_targets":["native","browser"]}]}))?;
        files.json("implementation-status.json", &json!({"design":"test-r1","phase":"test","tasks":[{"id":"T01","status":"not-implemented","evidence":[]}],"acceptance":[{"id":"A01","status":"passed","evidence":[REPORT]}],"implemented_crates":[],"note":"synthetic tooling test, never runtime evidence"}))?;
        files.write("apps/example/source.rs", "// synthetic source fixture\n")?;
        let log = "Synthetic checker fixture only; no language acceptance execution.\n";
        files.write("conformance/results/native.log", log)?;
        files.write("conformance/results/browser.log", log)?;
        let snapshot = identity(files.root())?;
        let environment = json!({"runner":{"name":"synthetic fixture","version":"1"},"tools":[{"name":"synthetic tool","version":"1"}]});
        let report = json!({"schema":"nepl3.acceptance-evidence/1","acceptance_id":"A01","design_revision":"test-r1","identity":snapshot.identity,"result":"passed","runs":[
            {"kind":"command","environment":environment,"command":"fixture native runner","target":"native","result":"passed","exit_code":0,"checks":["synthetic check"],"log":"conformance/results/native.log","log_sha256":identity::hex(&Sha256::digest(log))},
            {"kind":"command","environment":environment,"command":"fixture browser runner","target":"browser","result":"passed","exit_code":0,"checks":["synthetic check"],"log":"conformance/results/browser.log","log_sha256":identity::hex(&Sha256::digest(log))}
        ]});
        files.json(REPORT, &report)?;
        Ok((files, report))
    }

    #[test]
    fn actual_task_loader_accepts_only_matching_complete_attempt() -> Result<()> {
        let (files, _) = fixture()?;
        task::load(files.root())?;
        Ok(())
    }

    #[test]
    fn actual_task_loader_rejects_wrong_group_result_identity_and_target_coverage() -> Result<()> {
        let (files, report) = fixture()?;
        for (pointer, replacement) in [
            ("/acceptance_id", json!("A02")),
            ("/result", json!("failed")),
            ("/design_revision", json!("old-design")),
            ("/identity/source_sha256", json!("0".repeat(64))),
            ("/identity/spec_sha256", json!("0".repeat(64))),
            ("/runs/1/target", json!("native")),
            ("/runs/1/target", json!("unknown")),
            ("/runs/0/command", json!(" \n\t")),
            ("/runs/0/checks", json!(["  "])),
            ("/runs/0/environment/runner/version", json!("  ")),
            ("/runs/0/environment/runner/name", json!("  ")),
            ("/runs/0/environment/tools/0/version", json!("  ")),
            ("/runs/0/environment/tools", json!([])),
            ("/runs/0/exit_code", json!(1)),
            ("/runs/0/log_sha256", json!("0".repeat(64))),
            ("/runs/0/log", json!(REPORT)),
        ] {
            let mut bad = report.clone();
            *bad.pointer_mut(pointer)
                .ok_or("test report pointer missing")? = replacement;
            files.json(REPORT, &bad)?;
            assert!(task::load(files.root()).is_err(), "must reject {pointer}");
        }
        Ok(())
    }

    #[test]
    fn actual_task_loader_rejects_missing_malformed_and_duplicate_key_reports() -> Result<()> {
        let (files, _) = fixture()?;
        for bad in ["{}", "not JSON", r#"{"schema":"one","schema":"two"}"#] {
            files.write(REPORT, bad)?;
            assert!(task::load(files.root()).is_err());
        }
        fs::remove_file(files.root().join(REPORT))?;
        assert!(task::load(files.root()).is_err());
        Ok(())
    }

    #[test]
    fn failed_attempt_can_record_early_failure_without_claiming_full_coverage() -> Result<()> {
        let (files, mut report) = fixture()?;
        let mut status: Value = crate::json(files.root(), "implementation-status.json")?;
        status["acceptance"][0]["status"] = json!("failed");
        files.json("implementation-status.json", &status)?;
        report["result"] = json!("failed");
        report["runs"] = json!([report["runs"][0].clone()]);
        report["runs"][0]["result"] = json!("failed");
        report["runs"][0]["exit_code"] = json!(1);
        files.json(REPORT, &report)?;
        task::load(files.root())?;
        report["runs"][0]["result"] = json!("passed");
        report["runs"][0]["exit_code"] = json!(0);
        files.json(REPORT, &report)?;
        assert!(task::load(files.root()).is_err());
        Ok(())
    }

    #[test]
    fn independent_review_is_typed_and_cannot_be_faked_as_a_process_run() -> Result<()> {
        let (files, mut report) = fixture()?;
        let command = report["runs"][1].clone();
        let mut policy: Value = crate::json(files.root(), "design/acceptance.json")?;
        policy["targets"]["browser"]["kind"] = json!("review");
        files.json("design/acceptance.json", &policy)?;
        report["identity"] = serde_json::to_value(identity(files.root())?.identity)?;
        report["runs"][1] = json!({"kind":"review","target":"browser","reviewer":"independent synthetic reviewer","independent":true,"decision":"approved","scope":["fixture semantic correspondence"],"log":command["log"],"log_sha256":command["log_sha256"]});
        files.json(REPORT, &report)?;
        task::load(files.root())?;
        for (pointer, replacement) in [
            ("/runs/1/independent", json!(false)),
            ("/runs/1/reviewer", json!(" ")),
            ("/runs/1/scope", json!([])),
            ("/runs/1/decision", json!("rejected")),
        ] {
            let mut bad = report.clone();
            *bad.pointer_mut(pointer)
                .ok_or("review fixture pointer missing")? = replacement;
            files.json(REPORT, &bad)?;
            assert!(task::load(files.root()).is_err(), "{pointer}");
        }
        let mut bad = report.clone();
        bad["runs"][1] = command;
        files.json(REPORT, &bad)?;
        assert!(
            task::load(files.root()).is_err(),
            "process run cannot claim a review target"
        );
        bad = report.clone();
        bad["runs"][1]["exit_code"] = json!(0);
        files.json(REPORT, &bad)?;
        assert!(
            task::load(files.root()).is_err(),
            "review cannot carry fabricated process exit code"
        );
        let mut status: Value = crate::json(files.root(), "implementation-status.json")?;
        status["acceptance"][0]["status"] = json!("failed");
        files.json("implementation-status.json", &status)?;
        report["result"] = json!("failed");
        report["runs"][1]["decision"] = json!("rejected");
        files.json(REPORT, &report)?;
        task::load(files.root())?;
        Ok(())
    }

    #[test]
    fn partial_success_reports_cannot_be_combined_into_a_full_attempt() -> Result<()> {
        let (files, mut first) = fixture()?;
        let mut second = first.clone();
        first["runs"] = json!([first["runs"][0].clone()]);
        second["runs"] = json!([second["runs"][1].clone()]);
        files.json(REPORT, &first)?;
        files.json("conformance/results/A01-second.json", &second)?;
        let mut status: Value = crate::json(files.root(), "implementation-status.json")?;
        status["acceptance"][0]["evidence"] =
            json!([REPORT, "conformance/results/A01-second.json"]);
        files.json("implementation-status.json", &status)?;
        assert!(task::load(files.root()).is_err());
        Ok(())
    }

    #[test]
    fn inventory_addition_source_change_spec_change_and_deleted_input_invalidate() -> Result<()> {
        for changed in [
            "apps/example/source.rs",
            "apps/new.rs",
            "doc/spec/11-conformance.md",
            "design/review.json",
        ] {
            let (files, _) = fixture()?;
            let before = identity(files.root())?;
            let old = fs::read(files.root().join(changed)).unwrap_or_default();
            files.write(changed, [old.as_slice(), b"\n "].concat())?;
            let after = identity(files.root())?;
            assert_ne!(before.identity, after.identity, "{changed}");
            assert!(task::load(files.root()).is_err(), "stale input {changed}");
        }
        let (files, _) = fixture()?;
        fs::remove_file(files.root().join("apps/example/source.rs"))?;
        assert!(task::load(files.root()).is_err());
        Ok(())
    }

    #[test]
    fn evidence_status_and_generated_tasks_do_not_create_hash_self_reference() -> Result<()> {
        let (files, _) = fixture()?;
        let before = identity(files.root())?;
        files.write("conformance/results/extra.log", "new log")?;
        files.write("tasks/T01.md", "generated task")?;
        files.write(
            "implementation-status.json",
            "changed status excluded from identity",
        )?;
        files.write(".tmp/explanation.md", "ignored explanation")?;
        files.write("target/generated", "ignored build output")?;
        assert_eq!(before.identity, identity(files.root())?.identity);
        Ok(())
    }

    #[test]
    fn modifying_excluded_log_still_invalidates_its_bound_report() -> Result<()> {
        let (files, _) = fixture()?;
        let before = identity(files.root())?;
        files.write("conformance/results/native.log", "changed execution output")?;
        assert_eq!(before.identity, identity(files.root())?.identity);
        assert!(task::load(files.root()).is_err());
        Ok(())
    }
}
