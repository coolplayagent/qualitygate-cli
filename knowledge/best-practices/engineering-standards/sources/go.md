# Go Project

Sources: [Go Code Review Comments](https://go.dev/wiki/CodeReviewComments)
and [Effective Go](https://go.dev/doc/effective_go).

Authority: Go's public language-ecosystem documentation. `Effective Go` notes
that it is not actively updated for newer language features, so the current
review comments and compiler/tool documentation take precedence for mechanics.

## Rule inputs

- Run `gofmt` for mechanical formatting; this is stronger and cheaper evidence
  than a text-format heuristic.
- Treat contexts, cancellation, deadlines, and credentials as explicit API
  contracts rather than inferred comments or names.
- Preserve Go's package-name and exported-documentation semantics when testing
  naming or documentation conventions.

Qualitygate records Go style and static-gate work as planned until a configured
formatter, type/lint report schema, selected snapshot, and suppression policy
are available. A generic test-name rule must not replace Go test discovery or
pretend to verify context propagation.
