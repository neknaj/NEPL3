# Original recovery tar validation checkpoint

Independent review at `0299caf` checked six recovery and seven payload tests,
normally and under Python optimization. The real saved 22-file tar keeps its
original SHA-256 and is returned as the same byte object. No filesystem
extraction or replacement of the recovered bytes occurs.

Initial independent review reproduced file/directory prefix collisions that
filesystem snapshots had prevented but raw tar input could express. The common
file-set boundary now rejects those collisions and reserved-manifest case/path
collisions. Original counterexamples, corrected probes, earlier five-test runs,
final six-test runs and source snapshots are preserved separately.

This proves local raw-tar verification, not LKG identity, immutable release
storage, current deployment identity or restore authorization. Those remain
publisher contracts. No task/acceptance status or public setting changed.

Remove only `.fixture` suffixes to restore the original review bytes; original
line endings are retained. The reviewer manifest and archive seal bind all files.
