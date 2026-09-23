//! Results are small, status-owned records. Historical bytes remain in Git.
use crate::{Result, evidence, repository::local_path, task};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path},
};

const PREFIX: &str = "conformance/results/";
const MAX_RECORD_BYTES: u64 = 64 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StageHistory {
    schema: String,
    task_id: String,
    records: Vec<GitRecord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GitRecord {
    revision: String,
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
struct Status {
    tasks: Vec<State>,
    acceptance: Vec<State>,
}

#[derive(Deserialize)]
struct State {
    id: String,
    status: String,
    evidence: Vec<String>,
}

fn hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn history(bytes: &[u8], owner: &State) -> Result<()> {
    let record: StageHistory = serde_json::from_slice(bytes)?;
    if record.schema != "nepl3.stage-history/1"
        || record.task_id != owner.id
        || owner.status != "in-progress"
        || record.records.is_empty()
        || record.records.len() > 16
    {
        return Err("invalid stage-history identity, owner state or record count".into());
    }
    let mut identities = BTreeSet::new();
    for source in &record.records {
        if !hex(&source.revision, 40)
            || !hex(&source.sha256, 64)
            || source.path.is_empty()
            || source.path.contains('\\')
            || source.path.contains(':')
            || Path::new(&source.path)
                .components()
                .any(|p| !matches!(p, Component::Normal(_)))
            || !identities.insert((&source.revision, &source.path))
        {
            return Err("invalid or duplicate historical Git record".into());
        }
    }
    Ok(())
}

pub(super) fn check(root: &Path, files: &BTreeSet<String>) -> Result<()> {
    let records: Vec<_> = files
        .iter()
        .filter(|p| p.to_ascii_lowercase().starts_with(PREFIX))
        .collect();
    if records.is_empty() {
        return Ok(());
    }
    let status: Status = crate::json(root, "implementation-status.json")?;
    for name in records {
        if !name.starts_with(PREFIX) || !name.ends_with(".json") {
            return Err(format!("results accepts only typed JSON records: {name}").into());
        }
        let path = local_path(root, name)?;
        if fs::metadata(&path)?.len() > MAX_RECORD_BYTES {
            return Err(format!("result record exceeds 64 KiB: {name}").into());
        }
        let bytes = fs::read(path)?;
        let mut owners = 0;
        for owner in &status.tasks {
            if !owner.evidence.contains(name) {
                continue;
            }
            owners += 1;
            match owner.status.as_str() {
                "complete" => task::validate_record(&bytes, &owner.id)?,
                "in-progress" => {
                    // Both representations have closed schemas. Historical
                    // pointers cannot satisfy the completed-task schema.
                    if history(&bytes, owner).is_err() {
                        task::validate_record(&bytes, &owner.id)?;
                    }
                }
                _ => return Err("unimplemented task cannot own execution evidence".into()),
            }
        }
        for owner in &status.acceptance {
            if owner.evidence.contains(name) {
                owners += 1;
                evidence::validate_record(&bytes, &owner.id, &owner.status)?;
            }
        }
        if owners != 1 {
            return Err(format!("result record requires exactly one status owner: {name}").into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Fixture;
    use serde_json::json;

    #[test]
    fn record_limit_and_closed_git_record_schema() -> Result<()> {
        let files = Fixture::new()?;
        let path = "conformance/results/stages/T01.json";
        let state = json!({"tasks":[{"id":"T01","status":"in-progress","evidence":[path]}],"acceptance":[]});
        files.json("implementation-status.json", &state)?;
        let record = json!({"schema":"nepl3.stage-history/1","task_id":"T01","records":[{"revision":"a".repeat(40),"path":"old/result.json","sha256":"b".repeat(64)}]});
        let inventory = BTreeSet::from([path.to_owned()]);
        let mut bytes = serde_json::to_vec(&record)?;
        bytes.resize(usize::try_from(MAX_RECORD_BYTES)?, b' ');
        files.write(path, &bytes)?;
        check(files.root(), &inventory)?;
        bytes.push(b' ');
        files.write(path, &bytes)?;
        let error = check(files.root(), &inventory).expect_err("one byte above bound");
        assert!(error.to_string().contains("exceeds 64 KiB"));
        for (pointer, value) in [
            ("/task_id", json!("T02")),
            ("/records/0/revision", json!("main")),
            ("/records/0/sha256", json!("0")),
            ("/records/0/path", json!("../outside")),
            ("/records/0/path", json!("/absolute")),
            ("/records/0/path", json!("old\\source.rs")),
            ("/records", json!([])),
        ] {
            let mut bad = record.clone();
            *bad.pointer_mut(pointer).ok_or("fixture pointer")? = value;
            files.json(path, &bad)?;
            assert!(check(files.root(), &inventory).is_err(), "{pointer}");
        }
        let mut bad = record.clone();
        bad["records"][0]["source"] = json!("print('snapshot')");
        files.json(path, &bad)?;
        assert!(check(files.root(), &inventory).is_err());
        bad = record.clone();
        bad["records"] = json!([record["records"][0], record["records"][0]]);
        files.json(path, &bad)?;
        assert!(check(files.root(), &inventory).is_err());
        files.json(path, &record)?;
        let mut duplicate = state.clone();
        duplicate["tasks"] = json!([state["tasks"][0], state["tasks"][0]]);
        files.json("implementation-status.json", &duplicate)?;
        assert!(check(files.root(), &inventory).is_err());
        files.json("implementation-status.json", &state)?;
        check(files.root(), &inventory)?;
        assert!(
            check(
                files.root(),
                &BTreeSet::from(["Conformance/Results/hidden.json".to_owned()])
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn scoped_execution_record_is_valid_in_progress_or_complete() -> Result<()> {
        let files = Fixture::new()?;
        let path = "conformance/results/task.json";
        let record = json!({"task_id":"T01","checks":["synthetic condition"],"commands":["synthetic runner"],"targets":["native"],"result":"passed","excluded_acceptance_portions":[]});
        files.json(path, &record)?;
        let inventory = BTreeSet::from([path.to_owned()]);
        for state in ["in-progress", "complete"] {
            files.json(
                "implementation-status.json",
                &json!({"tasks":[{"id":"T01","status":state,"evidence":[path]}],"acceptance":[]}),
            )?;
            check(files.root(), &inventory)?;
        }
        let mut bad = record;
        bad["source"] = json!("arbitrary repository snapshot");
        files.json(path, &bad)?;
        assert!(check(files.root(), &inventory).is_err());
        Ok(())
    }

    #[test]
    fn history_is_typed_bounded_and_owned() -> Result<()> {
        let files = Fixture::new()?;
        let path = "conformance/results/stages/T01.json";
        let status = json!({"tasks":[{"id":"T01","status":"in-progress","evidence":[path]}],"acceptance":[]});
        files.json("implementation-status.json", &status)?;
        let record = json!({"schema":"nepl3.stage-history/1","task_id":"T01","records":[{"revision":"a".repeat(40),"path":"conformance/results/old/result.json","sha256":"b".repeat(64)}]});
        files.json(path, &record)?;
        let inventory = BTreeSet::from([path.to_owned()]);
        check(files.root(), &inventory)?;
        let mut changed = record.clone();
        changed["source"] = json!("fn main() {}");
        files.json(path, &changed)?;
        assert!(check(files.root(), &inventory).is_err());
        changed = record.clone();
        changed["schema"] = json!("unknown");
        files.json(path, &changed)?;
        assert!(check(files.root(), &inventory).is_err());
        files.json(path, &record)?;
        let mut changed_status = status.clone();
        changed_status["tasks"][0]["evidence"] = json!([]);
        files.json("implementation-status.json", &changed_status)?;
        assert!(check(files.root(), &inventory).is_err());
        changed_status = status;
        changed_status["tasks"][0]["status"] = json!("complete");
        files.json("implementation-status.json", &changed_status)?;
        assert!(check(files.root(), &inventory).is_err());
        for forbidden in [
            "run.py",
            "probe.rs",
            "Cargo.toml",
            "ci.yml",
            "source.rs.fixture",
            "stdout.log",
        ] {
            let forbidden = format!("{PREFIX}{forbidden}");
            // A valid record and matching owner make the filename rule the
            // sole rejection condition. Removing that rule must fail this test.
            files.json(&forbidden, &record)?;
            files.json("implementation-status.json", &json!({"tasks":[{"id":"T01","status":"in-progress","evidence":[forbidden]}],"acceptance":[]}))?;
            let error = check(files.root(), &BTreeSet::from([forbidden]))
                .expect_err("executable/fixture/log filename must be rejected");
            assert!(error.to_string().contains("only typed JSON records"));
        }
        Ok(())
    }
}
