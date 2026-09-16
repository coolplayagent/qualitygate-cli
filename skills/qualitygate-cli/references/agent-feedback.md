# Bounded Agent feedback

This surface is a development addition. Check the selected runtime's
`check --help` for `--feedback`; an older 0.4.0 release can lack it. Keep the
runtime/Skill pair explicit. Do not silently install a development build.

For an authorized check, preserve the caller's task, snapshot and policy:

```bash
"$QUALITYGATE_BIN" --root "$REPOSITORY_ROOT" check --worktree \
  --base "$BASE_COMMIT" --policy-ref "$TRUSTED_COMMIT" \
  --task tasks/request.yaml --profile full --feedback
```

`--feedback` is JSON regardless of `--format`. Its default budget is 32768
bytes including newline; `--feedback-max-bytes` accepts 4096–262144. The complete
unfiltered report is saved first. Verify `full_report` path, bytes and digest
before following its JSON Pointer references or original artifact paths.

- Interpret the original `gate` and exit codes: 0 pass, 1 violations, 2
  incomplete. A missing configuration may return the ordinary incomplete
  error object instead of an `agent_feedback` envelope.
- Keep `findings`, `execution_gaps` and `pending_delivery_checks` distinct.
  Never present diagnostics from an incomplete producer as confirmed defects.
- Respect per-category `total`, `filtered`, `omitted`, `items`, and text
  truncation. Filtered or omitted findings do not change the gate.
- Use only complete `recheck.argv` or `delivery_recheck.argv` arrays. If omitted,
  read the corresponding full report context. Final delivery uses full task
  scope; recheck the actual combined snapshot after any merge or input change.
  Replays include `--expect-base`; a moved implicit base remains incomplete
  until the caller explicitly chooses a new comparison.
- Fixed caller-selected policies remain fixed. Do not remove rules, lower
  severity, delete acceptance items or invent trusted references to pass.
- Full report locations and artifact references do not promise continued
  availability. Missing evidence remains a gap. A model's claim is not a gate.

The CLI never calls a model. An external loop may manage retries, resource
limits, no-progress termination, patches and human review records. Preserve all
failed/unfinished attempts and original full reports; compact feedback is not
an evidence replacement. Output bytes alone do not establish token savings or
business benefit.
