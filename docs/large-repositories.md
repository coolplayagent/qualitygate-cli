# Large repository acquisition

Rule catalog discovery has a separate [bounded parallel loading and query
performance contract](rule-management.md#large-repository-performance-contract).

[Issue #2](https://github.com/coolplayagent/qualitygate-cli/issues/2) reported a
16 MiB Git capture failure with approximately 18,000 tracked files and a 9 KiB
diff. The old implementation requested every blob in one `cat-file --batch`
process, collected all output, and only then checked its 12 MiB content budget.
Thus a small change failed because of the complete tree's size.

Snapshot acquisition now reads the selected tree/index inventory, checks blob
identities, types and sizes using `cat-file --batch-check`, then requests content
in batches of at most 4 MiB and 1,024 objects. Each response must match the
requested immutable object identities, declared sizes and framing. Missing
objects, oversized files, truncated data and process failures stay incomplete.
The general runner's 16 MiB per-stream limit is unchanged, including for Git
metadata and configured tools.

Base and target acquisition overlap. A shared semaphore bounds content readers
across both trees; worktree reads use blocks of at most 64 files on blocking
workers. Every filesystem read is byte-bounded and charged to the total budget
before allocation. Pending tasks are bounded; cancellation kills Git children
through the existing process group/Windows job guard and signals filesystem
workers to stop between reads. Parsing, hashing and change mapping run off
async orchestration workers. Removed contents are indexed by digest once to
avoid scanning the full baseline for every rename; byte equality still verifies
matches, and sorted paths preserve tie-breaking. Policy loading shares the
candidate snapshot instead of cloning all its contents.

## Caller budgets

| Control | Default | Accepted range / boundary |
|---|---|---|
| `--snapshot-max-mib` | 256 MiB per tree | 1–1,024 MiB; checked before Git content reads |
| `--snapshot-jobs` | 4 | 1–16 concurrent content readers, shared by base and target |
| `--snapshot-timeout-secs` | 120 seconds per acquisition | 1–3,600 seconds, including metadata, bytes and change mapping |
| Files per tree | 100,000 | Fixed fail-closed bound |
| Individual file | 2 MiB | Fixed fail-closed bound |
| Process stream | 16 MiB | Fixed runner bound; each Git content batch fits below it |

The byte budget counts file contents per path, including repeated blobs. Both
trees remain in memory; materialized build inputs and guards can require more
copies. Peak memory is a multiple of the per-tree budget, plus bounded buffers
per active reader and metadata. Choose budgets for the host's available memory.
The deadline applies independently to initial capture, trusted policy capture
and final revalidation. Each Git process also retains its 30-second deadline.
Git declaration history and provenance replay have their own documented total
budgets and use default snapshot acquisition limits.

```bash
qualitygate check --staged --path src --profile quick --snapshot-jobs 4
qualitygate check --worktree --path src/lib.rs --profile quick
qualitygate check --diff HEAD~1..HEAD --path src --profile quick --snapshot-max-mib 512
```

`--path` filters feedback inside the chosen snapshot. The complete input tree
is retained for policy validation, source bindings, report normalization,
unchanged dependencies and command execution. A scoped result remains `scope:
path` and cannot establish full delivery readiness. Recheck commands retain the
selector, path and acquisition budgets. Policy changes and final snapshot
mismatches remain blocking even when they occur outside the feedback path.

## Reproducible performance and regression checks

Run `cargo test --all-features --test large_repository -- --nocapture`.
The Rust harness creates an isolated repository with 18,000 unique 2 KiB blobs,
over 32 MiB of contents and a one-file delta. It compares serial and four-reader
captures for identical digests, changed lines and commit messages, prints their
times and requires each capture to finish within 60 seconds. It exercises
staged, worktree, commit-diff and standalone-path quick checks with a 120-second
per-check threshold, plus an explicit 16 MiB snapshot budget that must return
incomplete instead of passing. Small integration cases verify composed paths,
selector bytes and recheck budgets. The same suite runs in native OS CI.

`cargo test --all-features --lib snapshot::` separately covers size preflight,
batch wire-size bounds, invalid object identity/type/size/framing, missing
objects, file counts, symlinks, modes, invalid budgets, timeout and cancellation
entry, and immutable materialization. Existing policy and execution integration
tests retain final mismatch and policy-change checks.

These thresholds catch gross regressions on the controlled fixture. Parallel
speedup depends on storage, Git object packing, cache state and CPU contention;
it is measured rather than asserted as a universal ratio. Linux observations
do not establish performance on Windows or the issue reporter's repository.

The 2026-09-14 release-build observation on the shared Linux development host
used reader counts in the order 1, 4, 4, 1. Single-reader capture took 1.748 and
1.033 seconds; four-reader capture took 0.397 and 0.498 seconds. All four
captures had identical content digests, change maps and commit messages. The
complete CLI/regression test passed in 10.20 seconds. Other validation jobs
were active, so these are observations with cache and host-load variation,
not a universal speedup guarantee. Raw output is retained locally in
`target/issue2-performance.log`; rerun the same fixture with
`cargo test --release --test large_repository -- --nocapture`.
