# Internal Python, Go, Shell and TypeScript standards

Source IDs:

- `internal-python-coding`
- `internal-go-coding`
- `internal-shell-coding`
- `internal-typescript-coding`

| Source ID | Document | Effective date | Notes |
| --- | --- | --- | --- |
| `internal-python-coding` | Internal Python Language Coding Specification V3.4 | 2025-12-30 | Python 3.10 and later; replaces earlier general and secure Python guidance. |
| `internal-go-coding` | Internal Go Language Coding Specification V3.4 | 2025-12-30 | Consolidates style, error handling, concurrency, resource and security guidance. |
| `internal-shell-coding` | Internal Shell Language Coding Specification V2.2 | 2025-12 | Covers style, secure scripting, signals, permissions and command usage. |
| `internal-typescript-coding` | Internal TypeScript and JavaScript Language Coding Specification V1.0 | 2023-06 | Terminal and HarmonyOS development guidance; many clauses require type evidence. |

These are internal company standards. The registry uses `example.invalid`
placeholder URLs so the archive does not disclose internal locations. The
archive records only reviewed summaries and the clauses used by bounded rules;
it does not copy the full specifications.

Rule adoption is deliberately narrow. Only same-line lexical clauses are
mapped to `source-pattern` rules. Style, type-system, ownership, control-flow,
resource and data-flow clauses that cannot be decided from one line remain
capability gaps. A matching line is a review signal, not proof of conformance.
