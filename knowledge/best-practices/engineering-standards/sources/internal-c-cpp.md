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

Rule adoption is deliberately narrow. The ten executable rules use bounded
C/C++ syntax nodes, excluding comments and literals. They support multiline
syntax but do not resolve symbols, types, macro expansion or build conditions.
Naming style, header self-containment, include order, switch exhaustiveness,
constructor explicitness, ownership, lifetime, exception safety, integer
overflow and taint flow remain capability gaps. A finding is a review signal;
it does not establish conformance with the specification.
