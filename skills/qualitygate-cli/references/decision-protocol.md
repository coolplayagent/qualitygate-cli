# Versioned decisions and optional judgment

Use `--envelope --format json` on `check`, `check --feedback`, `rules validate`,
`selfcheck`, policy transitions or pilot summaries to receive a versioned
decision envelope. Export the pinned JSON Schemas with `qualitygate schema
decision`, `qualitygate schema feedback`, or `qualitygate schema project-rule`.
Check `command_kind` and validate the selected payload. An incomplete outcome
always has exit 2 and route `inspect_gap`; review warnings, pending checks and
preview omissions even when a complete gate passes. The `decision_id` binds
the complete envelope except the ID itself. Evidence pointers include digests.

`qualitygate judgment run --report check.json --policy judgment.yaml
--output-dir judgments --format json` optionally sends one warning to an
external provider. The strict version 1 policy declares fixed `argv`,
`version_argv`, input paths, a versioned question, timeout, `shadow` or
`advisory` mode, and optional calibration. Use `qualitygate judgment
question-digest --input question.yaml --format json` to derive a question's
source digest from its exact text, criteria and choices. The provider emits a
tagged deterministic, probabilistic, abstained or execution-gap assessment.
Probability requires a target event, normalized closed choice distribution,
matching evidence and valid in-domain calibration. Missing or invalid output,
timeout, changed scope and stale calibration produce an execution gap. A
successful abstention remains separate from an execution gap. The original
repository gate and findings are retained in both modes.

The run directory retains request, raw response, stderr, policy, report and
decision with digests. `qualitygate judgment pilot --runs judgments --labels
labels.json --audit-seed <at-least-16-byte-seed> --format json` compares retained
runs with independently declared labels in a later time cohort. It reports
calibration metrics, risk coverage, review count, slices and a seeded audit
sample. Missing labels or broken evidence produce an incomplete pilot result.
Provider and reviewer truth claims require external review; this workflow does
not approve or promote active policy. See the repository's
`docs/decision-protocol.md` for policy fields and limits.
