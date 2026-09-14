# Fixture regression workflow

Use the resolved Qualitygate binary and the active Skill rule asset directory:

```bash
"$QUALITYGATE_BIN" selfcheck --format json
"$QUALITYGATE_BIN" selfcheck --fixture minimal --format json
"$QUALITYGATE_BIN" selfcheck --rule commit-message --format json
"$QUALITYGATE_BIN" selfcheck --fixture stress --rule import-boundary --format json
```

No repository or candidate policy is needed. Inputs and independent goldens
are compiled into the binary; rules come from the installed Skill assets.
Native cases use Git and fixed child behaviors of the running CLI inside
temporary directories. No candidate project commands or networked producers
are executed. Synthetic signatures use an isolated public fixture key and
are not actual manual approvals.

The corpus covers minimal, typical and stress shapes. Each built-in rule has
compliant/violating pairs, with additional parser, policy, manual evidence,
compatibility, snapshot and runner boundaries. Full selfcheck runs sequentially
with bounded inputs and execution. A filter provides partial feedback only.

| Exit | Interpretation |
|---|---|
| 0 | Every selected observed result agrees with its golden, including expected violations/incomplete inputs. |
| 1 | A fixture assertion differs from the independent expected result. |
| 2 | Execution/inventory is incomplete, including missing assets, tools or an empty selection. |

On a regression, retain the fixture and suite IDs, input locator and digest,
golden digest, active-rule/corpus digests, and mismatching assertion's expected
and actual values. Diagnose the relevant production evaluator or input before
editing. Do not regenerate goldens from current output or weaken rule assets
to turn the result green. An intended policy change needs its own reviewed
contract and fixtures.

For authorized implementation work, repair the implementation, rerun the
targeted case and then the full corpus plus repository-required gates. Update
the tracked skill with demonstrated behavior and limits, synchronize the
requested installed copy, and rerun its own executable's selfcheck. Preserve
the prior installed runtime for recovery when replacing a development build.

Report the tested shapes and unverified assumptions. Synthetic facts do not
prove live producer behavior; a Linux run does not prove Windows behavior;
unknown frameworks and runtime configuration delivery remain outside the
observed evidence. Ordinary repository checks still require their selected
snapshot, policy and task; selfcheck is not a replacement for delivery checks.
