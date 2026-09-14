# Golden assertion contract

Each JSON object maps a fixture ID to JSON Pointer assertions on its observed
output. Values compare exactly, including explicit null versus an absent
field. A single-key object `{"$contains":"text"}` asserts a nonempty substring
of a string, for variable native error paths. Array indexes and escaped path
segments use JSON Pointer semantics (`~1` for `/`, `~0` for `~`).

Rule pairs assert execution status, verdict and diagnostic count; violation
cases locate the affected input, and selected cases pin source ranges or
evidence. Native boundary goldens pin the expected rejection where portable.
Missing, duplicate or orphan fixture IDs and empty assertions are errors.

Never regenerate these assertions from observed output to make a regression
green. Change an expectation only when the intended contract changes and the
new fixture and requirement are reviewed together. The mutation integration
test deliberately weakens the active commit rule while keeping these goldens.
The policy mutation test substitutes the line-ending evaluator while retaining
the protected CRLF oracle and promotion golden. Native policy goldens check
actual child execution status, lifecycle/activation state, preserved parent and
original revision, and signed authorization outcomes. Synthetic paired cases
separately pin gate verdicts and oracle conclusions.
