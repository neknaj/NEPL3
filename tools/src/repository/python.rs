//! Keep repository Python inside the root included by basedpyright.

use crate::Result;
use std::collections::BTreeSet;
use std::path::Path;

pub(super) fn check(root: &Path, files: &BTreeSet<String>) -> Result<()> {
    let mut directories = BTreeSet::new();
    for path in files {
        let lower = path.to_ascii_lowercase();
        if (lower.ends_with(".py") || lower.ends_with(".pyi"))
            && (!path.starts_with("tools/")
                || !(path.ends_with(".py") || path.ends_with(".pyi"))
                || path
                    .split('/')
                    .any(|part| part.starts_with('.') || part.starts_with("__editable__.")))
        {
            return Err(format!(
                "Python source must use .py/.pyi under the checked tools root: {path}"
            )
            .into());
        }
        if path.ends_with(".py") || path.ends_with(".pyi") {
            for parent in Path::new(path).ancestors().skip(1) {
                directories.insert(root.join(parent));
            }
        }
    }
    // Inspect disk as well as Git inventory: an ignored marker can hide tracked source.
    for directory in directories {
        for marker in [
            "bin/activate",
            "Scripts/activate",
            "pyvenv.cfg",
            "conda-meta",
        ] {
            if directory.join(marker).exists() {
                return Err(format!(
                    "virtual-environment marker would exclude Python source: {}",
                    directory.join(marker).display()
                )
                .into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_and_stub_locations_match_the_type_checker_root() -> Result<()> {
        check(
            Path::new("."),
            &BTreeSet::from([
                "tools/new/tool.py".into(),
                "tools/new/adapter.pyi".into(),
                "doc/example.md".into(),
            ]),
        )?;
        for path in [
            "script.py",
            "apps/tool.py",
            "conformance/results/probe.py",
            "tools-other/tool.py",
            "Tools/tool.py",
            "tools/tool.PY",
            "stubs/adapter.pyi",
            "tools/.hidden/probe.py",
            "tools/.hidden.py",
            "tools/__editable__.probe.py",
            "tools/__editable__.package/probe.py",
        ] {
            assert!(
                check(Path::new("."), &BTreeSet::from([path.into()])).is_err(),
                "{path}"
            );
        }
        Ok(())
    }

    #[test]
    fn untracked_environment_markers_cannot_hide_tracked_python() -> Result<()> {
        let root = std::env::temp_dir().join(format!(
            "nepl3-python-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir(&root)?;
        let result = (|| -> Result<()> {
            let files = BTreeSet::from(["tools/package/probe.py".into()]);
            check(&root, &files)?;
            for parent in ["", "tools", "tools/package"] {
                for marker in [
                    "bin/activate",
                    "Scripts/activate",
                    "pyvenv.cfg",
                    "conda-meta",
                ] {
                    let path = root.join(parent).join(marker);
                    std::fs::create_dir_all(path.parent().ok_or("marker parent")?)?;
                    std::fs::write(&path, "environment marker")?;
                    let rejected = check(&root, &files).is_err();
                    std::fs::remove_file(path)?;
                    assert!(rejected, "{parent}/{marker}");
                }
            }
            check(&root, &files)
        })();
        std::fs::remove_dir_all(root)?;
        result
    }
}
