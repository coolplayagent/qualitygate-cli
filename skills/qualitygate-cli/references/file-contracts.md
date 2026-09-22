# Instruction budgets and required files

Use this workflow when the requested policy must measure unchanged instruction
files or detect deletion of explicitly named owner/test files. It uses the
project-rule DSL and `files` capability, without adding a command or built-in
rule ID. Read [rule authoring](rule-authoring.md) for source binding, schema
compatibility, candidate generation and adoption.

## Author the requested contract

The CLI selects per-file findings only for changed files in the chosen snapshot.
A delivery pass does not establish that unchanged files meet per-file contracts.
Unlocated inventory totals (`min_count`, `max_count`, `max_total_words`) remain
aggregate constraints over the configured unexcluded inventory; a changed-line
filter cannot establish those totals from changed files alone. Policy `exclude`
removes resources before acquisition, so required build inputs must be retained.

Inspect `rules list --source project --format json` and the actual normative
section first. Take paths, budgets and severity from that requirement. The
following candidate illustrates the fields; replace its source block with the
exact result of `rules source` and adapt its assertions to the reviewed policy:

```yaml
id: instruction-inventory
schema_version: 1
version: 1
source:
  document: AGENTS.md
  section: Instruction maintenance
  content_hash: 'sha256:<exact section digest from rules source>'
requires_capabilities: [files]
applies_to:
  paths: [AGENTS.md, AGENTS.project.md, CLAUDE.md]
when: {entity: file, change: all}
then:
  min_count: 3
  max_lines: 300
  max_total_words: 4000
  required_paths: [src/net/client.rs, tests/network_contract.rs]
fix: Keep instructions within the reviewed budget and restore required owner or test files
```

| Assertion | Meaning |
|---|---|
| `min_count` / `max_count` | Minimum / maximum number of matching files; `min_count` can fail on zero matches |
| `max_lines` | Per-file text lines; LF/CRLF delimit lines, a final newline adds no empty line, empty files have zero lines |
| `max_total_words` | Combined Unicode whitespace-separated segments in matching UTF-8 files, including code and comments |
| `required_paths` | Up to 256 unique normalized literal repository-relative files that must exist in the captured snapshot |

`max_lines`, `max_total_words` and `required_paths` require
`when: {entity: file, change: all}`. Zero is a valid numeric limit;
`min_count` cannot exceed `max_count`. Omitted `change` means `added`, while
`any` remains limited to changed files and cannot enforce these assertions.

Text inventory respects `applies_to` language/path filters and CLI `--path`.
Required paths are checked against the complete captured snapshot even outside
those filters or with no matching text. They are files, not directories,
symbols or globs; the snapshot rejects symlinks. Scope this rule to intended
text files: invalid UTF-8 is incomplete execution. Selection and measurement
each have a 30-second deadline, with at most 50,000 files and 32 MiB selected
text; snapshot budgets apply independently.

Validate and generate through the [authoring sequence](rule-authoring.md#extraction-sequence).
Generation does not enable the rule. Adoption needs the selected project-rule
directory, enabled ID/profiles and the repository's bound source review.
Missing/stale review remains incomplete; a newly computed hash cannot replace
review. Do not edit the installed built-in rule assets to add a project rule.

## Check, repair and recheck

Run the caller's selector/profile using [operations](operations.md). Inspect
the rule's `metadata.file_inventory`: `files`, `lines`, `words`, `word_unit`,
`required_paths` and `snapshot_digest`. Use each diagnostic's assertion,
evidence, fix and pinned recheck to locate excess text or a missing file.

Repair the requested content while preserving its obligations, or restore the
actual required implementation/test. Do not create an empty file solely to
satisfy presence or raise budgets to make a failure pass. A staged check still
reads index bytes after a worktree repair; retain the requested selector and
report when the repair has not been staged. Recheck the same policy and scope;
path-scoped or quick feedback does not establish full delivery readiness.

Retain failures and incomplete evidence separately. Whitespace segments are
neither model tokens nor linguistic words. File presence does not prove
ownership, agent compliance or test effectiveness. Behavioral obligations need
the project's actual tests, rule evidence and review.

## Required text and nonempty scans

`then.require_pattern` requires a Rust regex match in every triggered entity's
text. Pair it with `then.min_count` when an empty scan must fail. The minimum
supports all valid entity/change combinations and counts triggered entities;
retained marker obligations do not satisfy it. Source bindings, review records
and explicit adoption still apply. Missing capabilities and failed parsing are
incomplete, not zero matches. For a reviewed contract-field rule, prove the
missing-text and empty-inventory failures separately before adoption.
