# Skill and rule boundary review

Reviewed 2026-11-11. This record concerns concrete rule counterexamples and
packaged instructions that can change a user's selected policy, scope or inputs.
It complements the [Skill design review](skill-contracts.md) and binds the
[SKILL-BOUNDARY requirements](../../../../codespec/requirements/skill-rule-boundaries.md).

## Sources and observed counterexamples

PEP 249 defines `format` and `pyformat` parameter styles, including `%s` and
`%(name)s`. It also defines binding values supplied separately to `execute`.
A marker inside the SQL string therefore does not establish that Python string
formatting occurred. The rule must examine the query expression rather than
infer an operator from literal text. This is our application of the primary
[Python DB-API specification](https://peps.python.org/pep-0249/), not a claim that
every call named `execute` is a database cursor or that binding alone proves
complete query safety.

The local GNU Bash 5.3.9(1)-release implementation was inspected with
`bash --noprofile --norc -c 'help trap'`, `help read` and `help '[['`.
Its help distinguishes trap action from signal operands, permits signal names
without the `SIG` prefix, describes `read` splitting through `IFS`, and identifies
`[[ ... ]]` as a conditional command. This is implementation evidence for that
build. The GNU online manual lookup timed out and is not recorded as a verified
web source.

The Linux man-pages project's [signal(7)](https://man7.org/linux/man-pages/man7/signal.7.html)
states that SIGKILL and SIGSTOP cannot be caught and documents different signal
numbers across architectures. That supports checking explicit signal names and
retaining a limit for numeric STOP spellings; Linux/x86's 19 is not a portable
STOP identity. This source describes Linux, not every shell or operating system.

| Rule | Counterexample to the previous match | Version-2 correction | Remaining limit |
| --- | --- | --- | --- |
| `py-sql-string-format` | `cursor.execute("SELECT id WHERE id=%s", (user_id,))` and `%(id)s` placeholders were treated as formatting. | Inspect the first positional query argument of `execute`/`executemany` for actual `%`, `.format(...)` or interpolated f-string syntax. Ignore formatting in value arguments and text inside ordinary string literals. | Call names and syntax do not resolve database identity, aliases, data flow or exploitability. |
| `shell-comparison-spaces` | `${items[0]}` and `[ab]*` contain brackets without being conditional-command delimiters. | Inspect test-command boundaries. Retain a narrow recovery for a real unquoted closing delimiter joined to its preceding word. | Recovery is unavailable to other rules and cannot hide unrelated or mixed parser errors. |
| `shell-assignment-spaces` | `IFS= read -r line` is a command with a temporary assignment. A physical-line pattern also matches `VALUE = text` inside a heredoc or multiline quoted string. | Inspect an actual command with a bare identifier name and a separated first operand beginning with unquoted `=` or `+=`; ignore matching text inside data. | Syntax does not establish runtime command intent: invoking a literal command with an `=` operand can be intentional rather than a mistaken assignment. |
| `shell-trap-uncapturable` | `trap 'printf 9' EXIT` matched action text, while `trap cleanup KILL`/`STOP` escaped the old pattern. | Inspect static signal operands after the action; cover KILL/STOP, optional SIG prefixes and case, and numeric 9. Exclude query/reset forms and action text. | Dynamic actions/signals are not resolved; numeric 19 is not assumed to mean STOP. Absence of a finding is not signal-handler verification. |

All four definitions advance to rule version 2 without changing requiredness,
severity or explicit enablement. All four use the bounded language-syntax adapter
to distinguish executable expression/operand shapes from matching text in data.
Frozen version-1 definitions keep their implementation and bytes so existing
signed policy can replay its original semantics. Their old false positives are
retained compatibility behavior, not the expected behavior of the new catalog.

## Instruction corrections

| Problem and counterexample | Correct boundary | Evidence and limitation |
| --- | --- | --- |
| Treating every `config --show` response as the worktree candidate could trigger local init when a signed active package was already selected. | Read `source` and `review_trust`; active authority takes precedence, a conflicting ref is an error, and a ref/snapshot needs its own inspection. Only repairable local absence triggers ordinary automatic init. | The promoted fixture still returns `signed_active_policy` after removing local configuration and when passed a different absent `--config` path; neither file is created. Existing init tests preserve ref errors and index/commit inclusion requirements. |
| Complete lookup was described as a full repository gate although located findings are delivery-filtered. | Required-path lookup can inspect captured files outside the text filter while historical missing paths and unchanged per-file debt remain outside selected findings. Whole-repository obligations need separate adequate evidence. | The file-contract regression contrasts CLI full delivery with reusable-core repository scope and tests changed/deleted/path-filtered findings. Aggregate inventory constraints retain their declared selection semantics. |
| Generation's default output directory could replace an existing legacy package. | Preserve effective `custom_rules`/project-rule selection; make migration explicit and retain the existing inventory and source/review bindings. | Existing custom-rule tests execute the legacy directory and select definitions from the policy snapshot. They do not prove an agent makes the right migration decision. |
| Modern startup commands were reused for a legacy executable, and missing V3 capability became a mandatory installation. | Inspect advertised legacy help/exports using the selected executable; use available authorized capability or report a gap. | Modern startup and V3 fixtures establish their own command behavior. Legacy/no-install decisions require separate forward traces. |
| The bundled pilot's 4+4 inventory and limit 2 became generic instructions. | Preserve the selected and sealed task mix, inventory and bounded no-progress limit. | A two-input 1+1 fixture with limit 3 permits a third attempt after two nonprogress attempts, retains the protocol in summaries, rejects post-seal limit changes and remains incomplete without the other observations. |
| “The CLI never calls a model” also described commands that launch configured providers. | `check --feedback` renders evidence; `agent-run` and judgment execute separately authorized commands. | Feedback, host and judgment tests retain their distinct execution and gate boundaries. None establishes model compliance or independent trial acceptance. |

## Evidence interpretation

The [bilingual evidence index](../../../../docs/en/04-contributor-guide/03-requirements-to-test-evidence.md)
names the targeted syntax, CLI, policy, file-contract and pilot regressions.
Rule findings remain limited to the inspected syntax and selected snapshot.
Malformed input, missing capabilities and incomplete producer evidence cannot
be rewritten as successful empty results.

The retained [pilot and inventory forward traces](../evidence/skill-boundary-forward-tests.json)
preserve nondefault inputs and identify the whole-inventory evidence gap.
The [policy-reference forward traces](../evidence/skill-policy-forward-tests.json)
preserve the selected authority without initialization, including the actual
snapshot-mismatch and invalid-ref incomplete results. These are single qualitative
observations with prompts, actions and source hashes, not reliability benchmarks.
Legacy runtime and existing-package migration decisions remain untested by these traces.

This record maps requirements and repeatable counterexamples. Full stable checks,
coverage and the final snapshot gate require their executed reports.
Neither a fixture nor a Skill edit supplies production pilot outcomes,
native-platform coverage, authenticated provider identity or deployment approval.
