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
    let records: Vec<_> = files.iter().filter(|p| p.starts_with(PREFIX)).collect();
    if records.is_empty() {
        return Ok(());
    }
    let status: Status = crate::json(root, "implementation-status.json")?;
    for name in records {
        if !name.ends_with(".json") {
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
            if owner.status == "complete" {
                task::validate_record(&bytes, &owner.id)?;
            } else {
                history(&bytes, owner)?;
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
            assert!(
                check(
                    files.root(),
                    &BTreeSet::from([format!("{PREFIX}{forbidden}")])
                )
                .is_err()
            );
        }
        Ok(())
    }
}
