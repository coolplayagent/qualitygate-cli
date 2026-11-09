# Internal C and C++ standards

Source IDs: `internal-c-coding`, `internal-cpp-coding`.

The current internal language specifications are separate from public vendor
rule sets:

| Source ID | Document | Effective date | Notes |
| --- | --- | --- | --- |
| `internal-c-coding` | Internal C Language Coding Specification V5.3 | 2023-12-29 | Replaces the earlier C general and secure coding guidance. |
| `internal-cpp-coding` | Internal C++ Language Coding Specification V5.1 | 2022-12-26 | Replaces the earlier C++ general and secure coding guidance; aligns with ISO 5055. |

These are internal company standards. The registry uses `example.invalid`
placeholder URLs so the archive does not disclose the internal location.
The archive records only reviewed summaries and the clauses used by bounded
rules; it does not copy the full specifications.

Rule adoption is deliberately narrow. Only same-line lexical clauses are
mapped to `source-pattern` rules. Naming style, comment placement, brace and
indent style, header self-containment, include order, switch exhaustiveness,
constructor explicitness, ownership, lifetime, exception safety, integer
overflow, taint flow and other clauses that require type, control-flow,
multi-line or semantic evidence remain capability gaps. A matching line is a
review signal; it does not establish conformance with the specification.
