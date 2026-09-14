# Minimal regressions

Each rule has a small compliant/violating pair in `cases.json`. For example,
`commit-message-pass` contains a valid subject, `commit-message-fail` an invalid
subject, and `commit-empty` an empty subject. These call the active commit rule;
changing it to accept any string must break the golden assertions.

Report parser pairs, policy/gate cases and native process success/failure are
also small. Run `qualitygate selfcheck --fixture minimal` for a quick CI gate.
Expected verdicts and diagnostic locations live in
[the minimal golden](../golden/minimal.json), not in the evaluator.
