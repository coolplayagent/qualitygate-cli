# Pilot evidence summaries

Use `qualitygate pilot summarize --input <observations.json> --format json` for
read-only aggregation of a predeclared pilot inventory. Start from
`templates/pilot/observation-v1.json`; fill assignments before collecting
outcomes and keep report paths relative to the manifest's external archive.

The command verifies report bytes and their task, policy, snapshot, environment,
tool and required-check bindings. It retains missing, failed, timed-out and
abandoned assignments in denominators. Null metrics mean that evidence or a
denominator is missing; do not replace them with zero. Different model,
permission, tool, environment, cache, origin, task type and workflow declarations
remain separate groups.

The output is descriptive. Reviewer identities, canonical issue truth, prices,
sealing and authority are caller declarations. Do not treat a summary or met
threshold as signed trial approval, and do not infer causal benefit from groups
without the same declared input inventory and conditions. See
`docs/pilot-phase-c.md` in the development repository for the full contract.
