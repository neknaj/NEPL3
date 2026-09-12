use crate::{Result, evidence, json, read, repository::local_path};
mod acceptance;
use acceptance::ids as acceptance_ids;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TaskFile {
    pub design: String,
    pub tasks: Vec<Task>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Task {
    pub id: String,
    title: String,
    depends_on: Vec<String>,
    acceptance: Vec<String>,
    deliverable: String,
    spec: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatusFile {
    design: String,
    phase: String,
    tasks: Vec<State>,
    pub acceptance: Vec<State>,
    pub implemented_crates: Vec<String>,
    note: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    id: String,
    status: String,
    evidence: Vec<String>,
}

#[derive(Deserialize)]
struct Review {
    findings: Vec<Finding>,
}

#[derive(Deserialize)]
struct Finding {
    id: String,
    status: String,
    affected_tasks: Vec<String>,
    evidence: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskEvidence {
    task_id: String,
    checks: Vec<String>,
    commands: Vec<String>,
    targets: Vec<String>,
    result: String,
    excluded_acceptance_portions: Vec<String>,
}

fn task_evidence(evidence: &TaskEvidence, id: &str) -> Result<()> {
    if evidence.task_id != id || evidence.result != "passed" {
        return Err(format!("{id}: evidence must identify the task and a passed result").into());
    }
    for (name, values) in [
        ("checks", &evidence.checks),
        ("commands", &evidence.commands),
        ("targets", &evidence.targets),
    ] {
        if values.is_empty() {
            return Err(format!("{id}: evidence {name} cannot be empty").into());
        }
        unique(values.iter().map(String::as_str), name)?;
    }
    unique(
        evidence
            .excluded_acceptance_portions
            .iter()
            .map(String::as_str),
        "excluded acceptance portions",
    )?;
    Ok(())
}

fn completion(
    task: &Task,
    task_states: &BTreeMap<&str, &str>,
    acceptance_states: &BTreeMap<&str, &str>,
    required: &BTreeSet<&str>,
) -> Result<()> {
    if task_states.get(task.id.as_str()) != Some(&"complete") {
        return Ok(());
    }
    if task
        .depends_on
        .iter()
        .any(|id| task_states.get(id.as_str()) != Some(&"complete"))
    {
        return Err(format!("{}: complete requires complete dependencies", task.id).into());
    }
    // Task acceptance IDs are coverage references, not a requirement to finish
    // later editor/provider behavior before the earlier foundation task.
    if task.id == "T16"
        && required
            .iter()
            .any(|id| acceptance_states.get(id) != Some(&"passed"))
    {
        return Err("T16: complete requires every required catalog acceptance group passed".into());
    }
    Ok(())
}

fn review(root: &Path, ids: &BTreeSet<&str>, status: &StatusFile) -> Result<()> {
    let review: Review = json(root, "design/review.json")?;
    unique(
        review.findings.iter().map(|f| f.id.as_str()),
        "review findings",
    )?;
    let mut open = 0;
    for finding in review.findings {
        if !matches!(finding.status.as_str(), "open" | "corrected") {
            return Err(format!("{}: unknown review status", finding.id).into());
        }
        for id in unique(
            finding.affected_tasks.iter().map(String::as_str),
            "review affected tasks",
        )? {
            if !ids.contains(id) {
                return Err(format!("{}: unknown affected task {id}", finding.id).into());
            }
            if finding.status == "open"
                && status
                    .tasks
                    .iter()
                    .any(|s| s.id == id && s.status == "complete")
            {
                return Err(format!("{id}: cannot complete while {} is open", finding.id).into());
            }
        }
        for evidence in &finding.evidence {
            local_path(root, evidence)?;
        }
        if finding.status == "open" {
            open += 1;
        }
    }
    println!(
        "Independent design review: {open} open findings; affected tasks cannot be marked complete."
    );
    Ok(())
}

pub(crate) fn unique<'a>(
    items: impl IntoIterator<Item = &'a str>,
    context: &str,
) -> Result<BTreeSet<&'a str>> {
    let mut set = BTreeSet::new();
    for item in items {
        if item.is_empty() || !set.insert(item) {
            return Err(format!("{context}: empty or duplicate identifier {item}").into());
        }
    }
    Ok(set)
}

pub(crate) fn dag(graph: &BTreeMap<&str, Vec<&str>>, context: &str) -> Result<()> {
    let mut completed = BTreeSet::new();
    for (node, deps) in graph {
        unique(deps.iter().copied(), context)?;
        for dep in deps {
            if !graph.contains_key(dep) {
                return Err(format!("{context}: {node} refers to unknown dependency {dep}").into());
            }
        }
    }
    loop {
        let before = completed.len();
        for (node, deps) in graph {
            if deps.iter().all(|dep| completed.contains(dep)) {
                completed.insert(*node);
            }
        }
        if completed.len() == graph.len() {
            return Ok(());
        }
        if completed.len() == before {
            return Err(format!("{context}: dependency cycle").into());
        }
    }
}

fn states(
    root: &Path,
    entries: &[State],
    expected: &BTreeSet<&str>,
    allowed: &[&str],
) -> Result<()> {
    let actual = unique(entries.iter().map(|s| s.id.as_str()), "status")?;
    if &actual != expected {
        return Err("status IDs do not exactly cover specification IDs".into());
    }
    for entry in entries {
        if !allowed.contains(&entry.status.as_str()) {
            return Err(format!("{}: unknown status {}", entry.id, entry.status).into());
        }
        unique(entry.evidence.iter().map(String::as_str), "evidence")?;
        let unexecuted = matches!(entry.status.as_str(), "not-implemented" | "not-run");
        if unexecuted && !entry.evidence.is_empty() {
            return Err(format!(
                "{}: unexecuted state cannot carry execution evidence",
                entry.id
            )
            .into());
        }
        if matches!(entry.status.as_str(), "complete" | "passed" | "failed")
            && entry.evidence.is_empty()
        {
            return Err(format!("{}: executed state requires evidence", entry.id).into());
        }
        for path in &entry.evidence {
            if !path.starts_with("conformance/results/") || !local_path(root, path)?.is_file() {
                return Err(format!(
                    "{}: evidence must be a file in conformance/results/: {path}",
                    entry.id
                )
                .into());
            }
        }
    }
    Ok(())
}

pub(crate) fn load(root: &Path) -> Result<(TaskFile, StatusFile)> {
    let tasks: TaskFile = json(root, "design/tasks.json")?;
    let status: StatusFile = json(root, "implementation-status.json")?;
    if tasks.design != status.design
        || tasks.design.is_empty()
        || status.phase.is_empty()
        || status.note.is_empty()
    {
        return Err("design/status revision mismatch or missing status metadata".into());
    }
    let ids = unique(tasks.tasks.iter().map(|t| t.id.as_str()), "tasks")?;
    if ids.is_empty() {
        return Err("task catalog is empty".into());
    }
    for id in &ids {
        let bytes = id.as_bytes();
        if bytes.len() != 3 || bytes[0] != b'T' || !bytes[1..].iter().all(u8::is_ascii_digit) {
            return Err(format!("invalid task ID: {id}").into());
        }
    }
    let acceptance_text = read(root, "doc/spec/11-conformance.md")?;
    let acceptance_owned = acceptance_ids(&acceptance_text)?;
    let acceptance: BTreeSet<&str> = acceptance_owned.iter().map(String::as_str).collect();
    if acceptance.is_empty() {
        return Err("acceptance catalog is empty".into());
    }
    let catalog = evidence::catalog(root, &tasks.design)?;
    let catalog_ids = unique(
        catalog.groups.iter().map(|g| g.id.as_str()),
        "acceptance catalog",
    )?;
    if acceptance != catalog_ids {
        return Err("acceptance catalog IDs differ from specification definitions".into());
    }
    let graph = tasks
        .tasks
        .iter()
        .map(|t| {
            (
                t.id.as_str(),
                t.depends_on.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    dag(&graph, "task graph")?;
    let mut covered = BTreeSet::new();
    for task in &tasks.tasks {
        if task.title.is_empty()
            || task.deliverable.is_empty()
            || !task.spec.starts_with("doc/spec/")
        {
            return Err(format!(
                "{}: missing task metadata or noncanonical spec reference",
                task.id
            )
            .into());
        }
        local_path(root, &task.spec)?;
        for id in unique(
            task.acceptance.iter().map(String::as_str),
            "task acceptance",
        )? {
            if !acceptance.contains(id) {
                return Err(format!("{}: unknown acceptance {id}", task.id).into());
            }
            covered.insert(id);
        }
    }
    if covered != acceptance {
        return Err("tasks do not cover every acceptance group".into());
    }
    states(
        root,
        &status.tasks,
        &ids,
        &["not-implemented", "in-progress", "complete"],
    )?;
    states(
        root,
        &status.acceptance,
        &acceptance,
        &["not-run", "passed", "failed", "blocked"],
    )?;
    let task_states: BTreeMap<_, _> = status
        .tasks
        .iter()
        .map(|s| (s.id.as_str(), s.status.as_str()))
        .collect();
    let acceptance_states: BTreeMap<_, _> = status
        .acceptance
        .iter()
        .map(|s| (s.id.as_str(), s.status.as_str()))
        .collect();
    let required = catalog
        .groups
        .iter()
        .filter(|g| g.required)
        .map(|g| g.id.as_str())
        .collect();
    for task in &tasks.tasks {
        completion(task, &task_states, &acceptance_states, &required)?;
    }
    if status
        .acceptance
        .iter()
        .any(|s| matches!(s.status.as_str(), "passed" | "failed") || !s.evidence.is_empty())
    {
        let snapshot = evidence::identity(root)?;
        for state in &status.acceptance {
            evidence::acceptance(
                root,
                &catalog,
                &snapshot,
                &state.id,
                &state.status,
                &state.evidence,
            )?;
        }
    }
    for state in &status.tasks {
        if state.status == "complete" {
            for path in &state.evidence {
                let evidence: TaskEvidence = json(root, path)?;
                task_evidence(&evidence, &state.id)?;
            }
        }
    }
    unique(
        status.implemented_crates.iter().map(String::as_str),
        "implemented crates",
    )?;
    review(root, &ids, &status)?;
    Ok((tasks, status))
}

pub(crate) fn generate(
    root: &Path,
    tasks: &TaskFile,
    status: &StatusFile,
    write: bool,
) -> Result<()> {
    let mut output = BTreeMap::new();
    let mut index = format!(
        "# 実装タスク\n\n<!-- Generated by nepl3-tools tasks --write; do not edit. -->\n\n設計: `{}`。定義の正本は [design/tasks.json](../design/tasks.json)、状態の正本は [implementation-status.json](../implementation-status.json)。\n\nタスクは依存順に実装し、[独立設計レビュー](../design/review.json)の該当する未解決事項を解消して受入試験の実行証拠を記録する。リポジトリ検査の成功は言語実装の完成を意味しない。\n\n| ID | 内容 | 状態 | 依存 |\n| --- | --- | --- | --- |\n",
        tasks.design
    );
    for task in &tasks.tasks {
        let state = status
            .tasks
            .iter()
            .find(|s| s.id == task.id)
            .ok_or("missing task status")?;
        let dependencies = if task.depends_on.is_empty() {
            "なし".to_owned()
        } else {
            task.depends_on.join(", ")
        };
        index.push_str(&format!(
            "| [{}]({}.md) | {} | {} | {} |\n",
            task.id, task.id, task.title, state.status, dependencies
        ));
        output.insert(format!("{}.md", task.id), format!("# {}: {}\n\n<!-- Generated by nepl3-tools tasks --write; do not edit. -->\n\n- 設計: `{}`\n- 状態: `{}`\n- 依存: {}\n- 仕様: [{}](../{})\n- 受入条件: {}（[条件定義](../doc/spec/11-conformance.md)）\n\n## 成果物\n\n{}\n\n## 完成の記録\n\n受入IDはcoverage参照。T16以外は担当範囲の成果物・失敗系・境界系・roundtrip・対象環境を検証し、段階証拠と未検証部分を `implementation-status.json` に記録する。依存タスクの完了と該当する設計blockerの解消が必要。\n\nT16は [受入catalog](../design/acceptance.json) の全必須群に、現在の仕様・source identityと対象環境に一致する受入証拠を要求する。段階証拠から全体受入成功を推定せず、未実行は未実行とし、後段を成功stubで覆わない。\n", task.id, task.title, tasks.design, state.status, dependencies, task.spec, task.spec, task.acceptance.join(", "), task.deliverable));
    }
    output.insert("README.md".to_owned(), index);
    let directory = root.join("tasks");
    if directory.exists() {
        local_path(root, "tasks")?;
    }
    if write {
        fs::create_dir_all(&directory)?;
    }
    // Extra task files are errors, never silently deleted by the generator.
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() {
            return Err("generated tasks must not be symlinks".into());
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "non-UTF-8 task filename")?;
        if name.ends_with(".md") && !output.contains_key(&name) {
            return Err(format!("obsolete or unexpected task document: tasks/{name}").into());
        }
    }
    for (name, content) in output {
        let path = directory.join(&name);
        if write {
            fs::write(path, content.as_bytes())?;
        } else if fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?
            != content.as_bytes()
        {
            return Err(format!("tasks/{name} is stale; run tasks --write").into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_task_completion_does_not_pretend_global_editor_acceptance_passed() -> Result<()> {
        let task = Task {
            id: "T01".into(),
            title: "foundation".into(),
            depends_on: vec![],
            acceptance: vec!["E03".into()],
            deliverable: "source types".into(),
            spec: "doc/spec/02-foundation.md".into(),
        };
        let evidence = TaskEvidence {
            task_id: "T01".into(),
            checks: vec!["reject non-scalar source spans".into()],
            commands: vec!["cargo test -p nepl3-core".into()],
            targets: vec!["x86_64-pc-windows-msvc".into()],
            result: "passed".into(),
            excluded_acceptance_portions: vec!["E03 editor definition navigation not run".into()],
        };
        task_evidence(&evidence, "T01")?;
        completion(
            &task,
            &BTreeMap::from([("T01", "complete")]),
            &BTreeMap::from([("E03", "not-run")]),
            &BTreeSet::from(["E03"]),
        )?;
        Ok(())
    }

    #[test]
    fn final_completion_requires_all_global_acceptance_passed() {
        let task = Task {
            id: "T16".into(),
            title: "final".into(),
            depends_on: vec!["T15".into()],
            acceptance: vec!["A01".into()],
            deliverable: "all acceptance".into(),
            spec: "doc/spec/11-conformance.md".into(),
        };
        assert!(
            completion(
                &task,
                &BTreeMap::from([("T16", "complete"), ("T15", "complete")]),
                &BTreeMap::from([("A01", "passed"), ("E03", "not-run")]),
                &BTreeSet::from(["A01", "E03"])
            )
            .is_err()
        );
    }

    #[test]
    fn terminal_task_uses_new_required_groups_and_not_optional_groups() -> Result<()> {
        let task = Task {
            id: "T16".into(),
            title: "terminal".into(),
            depends_on: vec![],
            acceptance: vec!["S06".into()],
            deliverable: "all required".into(),
            spec: "doc/spec/11-conformance.md".into(),
        };
        let tasks = BTreeMap::from([("T16", "complete")]);
        let states = BTreeMap::from([("S06", "passed"), ("U08", "not-run"), ("J04", "not-run")]);
        assert!(completion(&task, &tasks, &states, &BTreeSet::from(["S06", "U08"])).is_err());
        completion(&task, &tasks, &states, &BTreeSet::from(["S06"]))?;
        Ok(())
    }

    #[test]
    fn scoped_evidence_rejects_empty_checks_or_wrong_task() {
        let evidence = TaskEvidence {
            task_id: "T02".into(),
            checks: vec![],
            commands: vec!["test".into()],
            targets: vec!["native".into()],
            result: "passed".into(),
            excluded_acceptance_portions: vec![],
        };
        assert!(task_evidence(&evidence, "T01").is_err());
        assert!(task_evidence(&evidence, "T02").is_err());
    }

    #[test]
    fn dependency_graph_rejects_cycle_missing_and_duplicate_edges() {
        for graph in [
            BTreeMap::from([("A", vec!["B"]), ("B", vec!["A"])]),
            BTreeMap::from([("A", vec!["B"])]),
            BTreeMap::from([("A", vec![]), ("B", vec!["A", "A"])]),
        ] {
            assert!(dag(&graph, "test").is_err());
        }
        assert!(dag(&BTreeMap::from([("A", vec![]), ("B", vec!["A"])]), "test").is_ok());
    }

    #[test]
    fn acceptance_ids_read_annotated_list_definitions() -> Result<()> {
        let ids = acceptance_ids(
            "- X01\\: [外部](guide.md)\n\n<!-- -->\n\n- G01\\: <ruby>文法<rt>ぶんぽう</rt></ruby>\n- G02\\: second\n",
        )?;
        assert_eq!(ids.len(), 3);
        assert!(ids.contains("X01") && ids.contains("G01") && ids.contains("G02"));
        Ok(())
    }

    #[test]
    fn acceptance_ids_are_spec_definitions_only() -> Result<()> {
        let ids = acceptance_ids("# A01\nA01: first\nW03: second\nsee A02: elsewhere\n")?;
        assert_eq!(ids, BTreeSet::from(["A01".into(), "W03".into()]));
        assert!(acceptance_ids("A01: one\nA01: duplicate").is_err());
        Ok(())
    }

    #[test]
    fn executed_status_requires_evidence() {
        let entries = [State {
            id: "A01".into(),
            status: "passed".into(),
            evidence: vec![],
        }];
        assert!(
            states(
                Path::new("."),
                &entries,
                &BTreeSet::from(["A01"]),
                &["passed"]
            )
            .is_err()
        );
    }

    #[test]
    fn status_cannot_omit_an_acceptance_group() {
        let entries = [State {
            id: "A01".into(),
            status: "not-run".into(),
            evidence: vec![],
        }];
        assert!(
            states(
                Path::new("."),
                &entries,
                &BTreeSet::from(["A01", "A02"]),
                &["not-run"]
            )
            .is_err()
        );
    }
}
