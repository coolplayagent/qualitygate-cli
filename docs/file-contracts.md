# File inventory contracts

Project rules can inspect unchanged files with
`when: {entity: file, change: all}`. This extends the existing finite DSL with
instruction budgets and required owner/gate files. It introduces no command
interpreter, automatic policy edits or approvals.

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
fix: Keep instructions within the reviewed budget and restore missing owner or gate files
```

These paths and thresholds are examples. Select them from reviewed project
requirements, obtain the real source binding with `rules source`, then follow
the [authoring and source-review workflow](custom-rules.md#schema-guided-generation-and-validation).
Creating a definition does not enable it.

| Assertion | Measurement |
|---|---|
| `min_count` | Minimum matching files; zero matches can therefore fail |
| `max_count` | Existing maximum entity count; `all` covers the full selected inventory |
| `max_lines` | Maximum lines per UTF-8 file; LF/CRLF delimit lines, a final newline adds no empty line, an empty file has zero lines |
| `max_total_words` | Maximum total Unicode whitespace-separated segments, including code and comments, across matching files |
| `required_paths` | Up to 256 unique normalized literal repository-relative files that must exist in the captured snapshot |

The four new assertions require `file` with `change: all`. Zero is a valid
numeric limit; `min_count` cannot exceed `max_count`. Required paths are exact
files, not directories, symbols or globs. They are checked even with no text
matches and may be outside the text filters. The snapshot boundary rejects
symlinks. Removing a required owner or gate file fails without a concurrent
documentation edit; `min_count` detects removal of instruction files.

`all` respects language/path filters and the caller's `--path`. Scoped checks
retain scoped delivery status. Omitted `change` remains `added`; `any` still
selects current entities only in changed files. Staged checks use staged bytes
and inventories even when the worktree contains a repair.

`file_inventory` metadata records files, lines, words, required paths, word unit
and snapshot digest. Each assertion failure has a stable diagnostic and pinned
recheck. Selection and measurement each have a 30-second deadline. File rules
retain at most 50,000 files and 32 MiB of selected text. Invalid UTF-8 or exceeded
budgets produce incomplete execution, never a partial passing inventory. Work
runs on the existing blocking rule worker; snapshot limits apply independently.

Whitespace segments are a reproducible budget unit, not model tokens or a
linguistic word count. Presence does not prove an owner's responsibility or a
test's effectiveness. References are explicitly declared in the rule, not
inferred from prose. Use existing import boundaries, project rules, actual
tests and review for behavioral contracts. Advisory experience can remain in
knowledge documents and the [evidence archive](policy-evolution.md); turning it
into an obligation still requires reviewed source mappings and policy adoption.

| Requirement | Regression evidence |
|---|---|
| Unchanged inventory, combined budget, required-file and instruction deletion | `tests/file_contracts.rs::full_file_contracts_detect_unchanged_debt_missing_files_and_staged_repairs` |
| Staged isolation, CLI path scope, trusted-policy protection | The same integration fixture checks these independently |
| Unicode, empty files and binary input | `file_contracts_reject_binary_input_and_measure_unicode_words_and_empty_files` |
| Strict assertion types, bounds and confined paths | `file_contract_schema_and_semantics_reject_ambiguous_or_unbounded_definitions` |
| Expiration, file-count and byte budgets | `adapters::custom_rules::file_inventory::tests::complete_file_inventory_refuses_expired_and_oversized_work_without_partial_success` |

Run `cargo test --all-features --test file_contracts` separately from the native
unit gate. Controlled fixtures establish these shapes, not agent compliance.
