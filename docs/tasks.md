# Task plans and selected policies

A task contract records an identifier, observable acceptance descriptions and their command or manual checks. `qualitygate check --task task.yaml --profile full` merges those checks with the repository policy. A full profile must include every required check; quick and path checks describe only their selected scope and retain pending delivery checks. Without `--task`, the report covers repository policy and cannot establish task completion.

```yaml
schema_version: 1
task_id: required-behavior
acceptance:
  - id: behavior
    description: The requested behavior passes its regression test
    required: true
    severity: error
    verification:
      kind: command
      check_id: behavior-test
      argv: [cargo, test, --test, behavior]
      timeout_seconds: 300
```

The command must exercise the stated behavior. Recognized Cargo, pytest, Maven and other supported test runners require actual nonzero execution counts; report-producing wrappers need explicit [test report requirements](reports.md). Command exits, required arguments, prerequisites, tool probes, report mappings, project producers and compatibility checks use the same validation as repository commands. See [signed manual acceptance](manual-acceptance.md) for external decisions. Merely naming a command as a test does not prove its behavior or test coverage.

## Selecting the task authority

Without `--policy-ref`, configuration, custom rules and the task contract come from the selected code snapshot. `--staged` uses index bytes even when working files differ; worktree mode uses current captured bytes. These policies retain the `local_candidate` trust label.

With `--policy-ref <reference>`, the CLI resolves the reference to a commit before reading it. Configuration, custom definitions and the requested task file all come from that same commit. The selected contract defines the plan and task digest. The candidate snapshot is compared against those selected assets, including file contents and executable mode. Changed, deleted or malformed candidate task files cannot remove original acceptance items from the plan. Removing the candidate configuration likewise retains the selected policy's requirements. Such changes leave the gate incomplete and prevent configured commands and manual acceptance from completing; internal rule diagnostics remain visible.

The task must exist and be valid in the selected policy snapshot. A candidate-only task cannot be treated as approved by a reference that does not contain it. A malformed selected configuration or task remains an invalid configuration. Parsing and planning run on a blocking worker, within the existing 1 MiB limits for each configuration and task file.

The reusable `Plan::build` API validates task semantics even when a caller constructs `TaskContract` directly instead of using the YAML loader. Unsupported schema versions, empty task IDs or acceptance inventories, invalid/duplicate acceptance IDs and empty descriptions are rejected for both quick and full plans. Check configurations still pass the shared command and dependency validation.

CI controls the expected task path and policy reference and verifies that the selected commit is protected and approved. Supplying a reference locally retains the `caller_supplied_ref` label; resolution to a commit does not authenticate approval. The CLI does not discover an omitted task contract or authorize a new task because it exists in the candidate tree. Teams must establish the initial task and subsequent changes through their existing review workflow.

## Report and recheck evidence

- `policy.source` retains the requested policy reference or candidate configuration path. `policy.resolved_commit`, when present, identifies the actual selected policy commit.
- `policy.task_contract_source` names the requested task path; `task_contract_digest` hashes the selected contract bytes. No task means no task source and a null task digest.
- `policy.changes` lists each differing policy/verification asset once in sorted order, even when it is also matched by an explicit `verification_assets` glob. Globs remain necessary to protect project-specific verification inputs beyond the configuration, requested task and custom rule definitions.
- `plan.task_id`, `plan.acceptance` and `plan.acceptance_descriptions` retain task identity, acceptance-to-check associations and expected behavior. Table and Markdown reports also show these descriptions.
- Each selected configured check retains `metadata.command_definition`, including the selected arguments, expected exits, report requirements and prerequisites, even when blocked. This is the acceptance configuration. `execution` and its artifacts separately record what actually ran.
- Diagnostic `recheck.argv` pins `--policy-ref` to the resolved commit. Moving the original branch cannot silently change the policy used by that replay. Selecting a newly approved policy is a new invocation under the caller's workflow.

These optional report fields are additive to JSON schema version 1. Earlier successful reports keep the same task digest because their candidate and selected contracts matched. Earlier incomplete reports could describe the changed candidate contract; current reports preserve the selected contract instead. Existing signed manual/provenance subject fields are unchanged and still bind the selected configuration, rule and task digests. A report or digest alone does not establish trusted execution.

## Requirement-to-test evidence

| Requirement | Authoritative fixtures |
|---|---|
| §3.4 and §6.6 deletion cannot erase selected task requirements | `tests/policy.rs::selected_policy_preserves_the_original_task_plan_after_candidate_deletion_or_corruption` |
| §3.5 staged isolation and §6.6 verification asset identity | `tests/policy.rs::staged_tasks_and_executable_mode_changes_use_their_selected_inputs` |
| §8.2 policy source identity and reproducible recheck | `tests/policy.rs::recheck_pins_the_selected_policy_commit_after_a_branch_moves` |
| §3.4 full/quick completeness and actual expected command exits | `tests/cli.rs::task_acceptance_is_merged_and_quick_reports_pending_delivery`, `tests/execution.rs::task_contracts_preserve_expected_exit_codes_and_tool_evidence` |
| §3.4 reusable core cannot bypass task validation | `config/plan.rs::reusable_planning_validates_task_contracts_without_requiring_yaml_loading` constructs task contracts directly and checks both profiles |
| §6.3 concurrent violations and incomplete policy evidence | The pinned-recheck fixture retains the original error diagnostic while the policy mismatch returns exit 2 |
| §9.1 text/JSON consistency and retained acceptance conditions | Policy fixtures exercise JSON, table and Markdown after deleting candidate acceptance items; the manual acceptance suite verifies signed task composition |

These fixtures use isolated temporary Git repositories. They do not establish the team-selected real-repository pilot or its human review measurements.
