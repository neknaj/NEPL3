use super::*;
use crate::doc::source::{budget, err};
use std::io::Cursor;
struct Chunks {
    inner: Cursor<Vec<u8>>,
    chunk: usize,
    interruptions: usize,
    calls: usize,
    fail: Option<usize>,
    interrupt_at: Option<usize>,
    interrupt_eof: bool,
}
impl Read for Chunks {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.interruptions > 0 {
            self.interruptions -= 1;
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.interrupt_at == Some(self.calls)
            || (self.interrupt_eof && self.inner.position() == self.inner.get_ref().len() as u64)
        {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.fail == Some(self.calls) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let len = out.len().min(self.chunk);
        self.inner.read(&mut out[..len])
    }
}
fn reader(bytes: &[u8], chunk: usize, interruptions: usize) -> Chunks {
    Chunks {
        inner: Cursor::new(bytes.to_vec()),
        chunk,
        interruptions,
        calls: 0,
        fail: None,
        interrupt_at: None,
        interrupt_eof: false,
    }
}
#[test]
fn bounded_reader_checks_partial_interrupted_and_exact_eof() -> Result<(), LoadError> {
    for interrupts in [0, 2] {
        let mut r = reader(b"abc", 1, interrupts);
        let mut out = [0; 3];
        let mut b = budget();
        read_bounded(&mut r, &mut out, 7, &mut b)?;
        assert_eq!(&out, b"abc");
        assert_eq!(r.calls, 4 + interrupts);
        assert_eq!(b.usage().work, 8 + interrupts as u64);
        assert_eq!(b.usage().source_bytes, 0);
        assert_eq!(b.usage().output_bytes, 0);
        assert_eq!(b.usage().allocation_units, 0);
    }
    let mut eof_interrupted = reader(b"abc", 3, 0);
    eof_interrupted.interrupt_at = Some(2);
    read_bounded(&mut eof_interrupted, &mut [0; 3], 7, &mut budget())?;
    assert_eq!(eof_interrupted.calls, 3);
    let mut empty = reader(b"", 1, 0);
    read_bounded(&mut empty, &mut [], 7, &mut budget())?;
    assert_eq!(empty.calls, 1);
    assert_eq!(
        read_bounded(&mut reader(b"ab", 1, 0), &mut [0; 3], 7, &mut budget()),
        Err(LoadError::Size { index: 7 })
    );
    assert_eq!(
        read_bounded(&mut reader(b"abcd", 1, 0), &mut [0; 3], 7, &mut budget()),
        Err(LoadError::Size { index: 7 })
    );
    for (call, stage) in [(1, Stage::Read), (2, Stage::Eof)] {
        let mut r = reader(b"abc", 3, 0);
        r.fail = Some(call);
        assert_eq!(
            read_bounded(&mut r, &mut [0; 3], 7, &mut budget()),
            Err(LoadError::Io {
                index: Some(7),
                stage,
                kind: io::ErrorKind::BrokenPipe
            })
        );
    }
    Ok(())
}
#[test]
fn bounded_reader_stops_infinite_interrupts_and_exact_work_boundaries() -> Result<(), LoadError> {
    for cap in [8, 7] {
        let mut limits = budget().limits();
        limits.work = cap;
        let mut b = Budget::new(limits);
        let result = read_bounded(&mut reader(b"abc", 1, 0), &mut [0; 3], 7, &mut b);
        if cap == 8 {
            result?;
        } else {
            assert_eq!(result, Err(LoadError::Stopped(StopReason::WorkLimit)));
            assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        }
    }
    let mut endless = reader(b"abc", 1, usize::MAX);
    let mut limits = budget().limits();
    limits.work = 10;
    let mut b = Budget::new(limits);
    assert_eq!(
        read_bounded(&mut endless, &mut [0; 3], 7, &mut b),
        Err(LoadError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(endless.calls, 6);
    assert_eq!(b.poll(), Err(StopReason::WorkLimit));
    let mut eof_endless = reader(b"abc", 3, 0);
    eof_endless.interrupt_eof = true;
    let mut limits = budget().limits();
    limits.work = 10;
    let mut eof_budget = Budget::new(limits);
    assert_eq!(
        read_bounded(&mut eof_endless, &mut [0; 3], 7, &mut eof_budget),
        Err(LoadError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(eof_endless.calls, 6);
    let mut cancelled = budget();
    cancelled.cancel();
    let mut r = reader(b"abc", 1, 0);
    assert_eq!(
        read_bounded(&mut r, &mut [0; 3], 7, &mut cancelled),
        Err(LoadError::Stopped(StopReason::Cancelled))
    );
    assert_eq!(r.calls, 0);
    Ok(())
}
fn total() -> u64 {
    pins::PINS.iter().map(|p| p.bytes as u64).sum()
}
#[test]
fn preflight_and_budget_caps_precede_missing_root_io() -> Result<(), String> {
    let missing = Path::new("/nepl3-nonexistent-asset-installation");
    assert!(matches!(
        load_installation(missing, total() - 1, &mut budget()),
        Err(LoadError::Assets(super::super::Error::AssetByteLimit))
    ));
    let mut limits = budget().limits();
    limits.allocation_units = 0;
    let mut b = Budget::new(limits);
    assert!(matches!(
        load_installation(missing, total(), &mut b),
        Err(LoadError::Stopped(StopReason::AllocationLimit))
    ));
    assert_eq!(b.poll(), Err(StopReason::AllocationLimit));
    let mut b = budget();
    b.cancel();
    assert!(matches!(
        load_installation(missing, total(), &mut b),
        Err(LoadError::Stopped(StopReason::Cancelled))
    ));
    assert!(matches!(
        load_installation(missing, total(), &mut budget()),
        Err(LoadError::Io {
            index: None,
            stage: Stage::RootMetadata,
            ..
        })
    ));
    Ok(())
}
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn temporary() -> Result<Temp, String> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(err)?
        .as_nanos();
    let path = std::env::temp_dir().join(format!("nepl3-assets-{}-{unique}", std::process::id()));
    fs::create_dir(&path).map_err(err)?;
    Ok(Temp(path))
}
#[test]
#[ignore = "requires the fixed KaTeX package files, not Node execution"]
fn fixed_installation_matches_preparation_and_keeps_allocation_cumulative() -> Result<(), String> {
    let installed = Path::new(env!("CARGO_MANIFEST_DIR")).join("audit/math/node_modules/katex");
    // Test-only trusted fixture reads, outside the measured loader operation.
    let raw = pins::PINS
        .iter()
        .map(|p| fs::read(installed.join(p.source)).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let input = pins::PINS
        .iter()
        .zip(&raw)
        .map(|(p, bytes)| Input {
            path: p.path,
            mime: p.mime,
            bytes,
        })
        .collect::<Vec<_>>();
    let expected = super::super::prepare(&input, total(), &mut budget()).map_err(err)?;
    let mut binary_limits = budget().limits();
    binary_limits.source_bytes = 0;
    binary_limits.output_bytes = 0;
    let mut measured = Budget::new(binary_limits);
    let actual = load_installation(&installed, total(), &mut measured).map_err(err)?;
    assert_eq!(actual.identity(), expected.identity());
    assert_eq!(actual.version(), expected.version());
    assert_eq!(actual.files().len(), 62);
    for (a, e) in actual.files().iter().zip(expected.files()) {
        assert_eq!(
            (a.path(), a.mime(), a.digest(), a.bytes()),
            (e.path(), e.mime(), e.digest(), e.bytes())
        );
    }
    let usage = measured.usage();
    assert_eq!(usage.source_bytes, 0);
    assert_eq!(usage.output_bytes, 0);
    assert!(usage.allocation_units >= total() * 2);
    let again = load_installation(&installed, total(), &mut measured).map_err(err)?;
    assert_eq!(again.identity(), actual.identity());
    assert_eq!(
        measured.usage().allocation_units,
        usage.allocation_units * 2
    );
    assert!(measured.usage().work > usage.work);
    // Allocation accounting is deterministic; filesystem syscall counts need not be.
    let mut limits = budget().limits();
    limits.allocation_units = usage.allocation_units;
    assert_eq!(
        load_installation(&installed, total(), &mut Budget::new(limits))
            .map_err(err)?
            .identity(),
        actual.identity()
    );
    limits.allocation_units -= 1;
    let mut short = Budget::new(limits);
    assert!(matches!(
        load_installation(&installed, total(), &mut short),
        Err(LoadError::Stopped(StopReason::AllocationLimit))
    ));
    assert_eq!(short.poll(), Err(StopReason::AllocationLimit));
    Ok(())
}
#[test]
#[ignore = "requires the fixed KaTeX package files, not Node execution"]
fn filesystem_failures_are_indexed_and_extra_files_are_ignored() -> Result<(), String> {
    let installed = Path::new(env!("CARGO_MANIFEST_DIR")).join("audit/math/node_modules/katex");
    let temp = temporary()?;
    let root = &temp.0;
    for pin in pins::PINS {
        let destination = root.join(pin.source);
        fs::create_dir_all(destination.parent().ok_or("parent")?).map_err(err)?;
        fs::copy(installed.join(pin.source), destination).map_err(err)?;
    }
    fs::write(root.join("unrelated.txt"), b"ignored").map_err(err)?;
    let baseline = load_installation(root, total(), &mut budget())
        .map_err(err)?
        .identity();
    for index in [0, 1, pins::PINS.len() - 1] {
        let path = root.join(pins::PINS[index].source);
        let original = fs::read(&path).map_err(err)?;
        fs::remove_file(&path).map_err(err)?;
        assert!(
            matches!(load_installation(root,total(),&mut budget()),Err(LoadError::Io {index:Some(i),stage:Stage::PathMetadata,kind:io::ErrorKind::NotFound}) if i==index)
        );
        fs::write(&path, &original[..original.len() - 1]).map_err(err)?;
        assert!(
            matches!(load_installation(root,total(),&mut budget()),Err(LoadError::Size {index:i}) if i==index)
        );
        let mut longer = original.clone();
        longer.push(0);
        fs::write(&path, &longer).map_err(err)?;
        assert!(
            matches!(load_installation(root,total(),&mut budget()),Err(LoadError::Size {index:i}) if i==index)
        );
        let mut corrupt = original.clone();
        corrupt[0] ^= 1;
        fs::write(&path, &corrupt).map_err(err)?;
        assert!(
            matches!(load_installation(root,total(),&mut budget()),Err(LoadError::Assets(super::super::Error::Digest {index:i})) if i==index)
        );
        fs::remove_file(&path).map_err(err)?;
        fs::create_dir(&path).map_err(err)?;
        assert!(
            matches!(load_installation(root,total(),&mut budget()),Err(LoadError::NotRegular {index:i}) if i==index)
        );
        fs::remove_dir(&path).map_err(err)?;
        fs::write(&path, original).map_err(err)?;
    }
    let dist = root.join("dist");
    let saved = root.join("saved-dist");
    fs::rename(&dist, &saved).map_err(err)?;
    assert!(matches!(
        load_installation(root, total(), &mut budget()),
        Err(LoadError::Io {
            index: Some(0),
            stage: Stage::PathMetadata,
            ..
        })
    ));
    fs::write(&dist, b"not a directory").map_err(err)?;
    assert!(matches!(
        load_installation(root, total(), &mut budget()),
        Err(LoadError::NotDirectory { index: Some(0) })
    ));
    fs::remove_file(&dist).map_err(err)?;
    fs::rename(&saved, &dist).map_err(err)?;
    assert!(matches!(
        load_installation(&root.join("LICENSE"), total(), &mut budget()),
        Err(LoadError::NotDirectory { index: None })
    ));
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let link = root.join("root-link");
        symlink(root, &link).map_err(err)?;
        assert!(matches!(
            load_installation(&link, total(), &mut budget()),
            Err(LoadError::Symlink { index: None })
        ));
        fs::remove_file(link).map_err(err)?;
        fs::rename(&dist, &saved).map_err(err)?;
        symlink(&saved, &dist).map_err(err)?;
        assert!(matches!(
            load_installation(root, total(), &mut budget()),
            Err(LoadError::Symlink { index: Some(0) })
        ));
        fs::remove_file(&dist).map_err(err)?;
        fs::rename(&saved, &dist).map_err(err)?;
        let license = root.join("LICENSE");
        let saved = root.join("saved-license");
        fs::rename(&license, &saved).map_err(err)?;
        symlink(&saved, &license).map_err(err)?;
        assert!(matches!(
            load_installation(root, total(), &mut budget()),
            Err(LoadError::Symlink { index: Some(1) })
        ));
        fs::remove_file(&license).map_err(err)?;
        fs::rename(saved, license).map_err(err)?;
    }
    assert_eq!(
        load_installation(root, total(), &mut budget())
            .map_err(err)?
            .identity(),
        baseline
    );
    Ok(())
}
