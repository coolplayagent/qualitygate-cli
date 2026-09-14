# Typical source shapes

`cases.json` includes retained Python/JUnit test modules, edits to a 100-line
Rust module, preservation of an existing CRLF file, review patterns after
retained comments, and Java/Python/Rust/TypeScript/Go import and API forms.
Historical findings and a real staged/worktree disagreement test change scope.

Project facts and analyzer reports are synthetic normalized inputs. Agreement
with [the typical golden](../golden/typical.json) does not prove that a real
compiler, dependency resolver or framework can produce those inputs.

Seventeen policy-evolution cases cover cost/activation accounting, absent and
skipped oracles, temporary producer paths, signed rejection, all five candidate
lifecycle states, multi-category/scalar context, and native serial/parallel
promotion plus signed rollback. Promotion uses retained report artifacts after
the original execution directory has been removed.
