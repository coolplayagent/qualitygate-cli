# Repository architecture contract

The Rust architecture gate follows the ownership boundaries in [AGENTS.md](../AGENTS.md) and the module-graph approach in the reference repository. Run `cargo test --locked --test quality architecture`.

## Allowed module dependencies

The table lists permitted direct dependencies between top-level production owners. Dependencies within one owner are outside this graph's scope. Adding an owner or changing a direction requires changing the explicit contract and reviewing its implications.

| Owner | May depend on |
|---|---|
| `domain` | No other repository owner |
| `env` | No other repository owner |
| `paths` | `env` |
| `net` | `domain`, `env` |
| `runner` | `domain`, `env` |
| `snapshot` | `domain`, `net`, `paths`, `runner` |
| `config` | `domain`, `env`, `paths` |
| `adapters` | `config`, `domain`, `paths`, `snapshot` |
| `application` | `adapters`, `config`, `domain`, `env`, `net`, `paths`, `runner`, `snapshot` |
| `interfaces` | `application`, `config`, `domain`, `snapshot` |
| Binary entry point | `interfaces` |

`lib.rs` declares physical owners and does not hide dependencies behind root re-exports. The CLI delegates candidate rule changes to `config::rule_management`; configuration validation, confined atomic replacement and permission preservation remain owned by configuration code. `config::enable_rule` delegates to the same transaction implementation for reusable callers.

File inventory assertions extend the project-rule schema and adapter; bounded
scans consume captured files on the blocking rule worker. Diagnostic ratchets
use a pure domain comparison and the existing paired report execution flow.
Both extensions preserve the owner table and dependency directions above.

Shell source recognition is a pure `domain::language` decision: `.sh` and
`.bash` paths are explicit, while an extensionless file needs a bounded
first-line `sh` or `bash` shebang. The `adapters` owner applies changed-line
regular expressions and the separate first-line/consecutive-comment checks
to immutable snapshots. A missing shebang on an extensionless file cannot
establish its language. These source forms do not add a dependency edge.

C and C++ source-pattern review uses the pure `domain::language::for_text_source`
extension classifier for `.c`, `.h`, `.cc`, `.cpp`, `.cxx`, `.hh`, `.hpp`, and
`.hxx`. It does not register C or C++ syntax capabilities. The existing
`adapters::rules` changed-line scanner consumes this label, while external
analyzer SARIF remains within the existing report adapter and paired runner.
The owner graph and dependency directions stay the same.

The `lang-cpp` package reuses that text classifier and source-pattern adapter.
Its Clang reference policy runs through the existing paired command and SARIF
report path, so no new production dependency edge is introduced.

Policy approval/promotion/rollback storage entry points are public across the
separate Bazel owner crates. They perform transactional consistency checks;
application callers authenticate signatures and live trust before publication.
Active-policy reads authenticate again. Public storage records and transactions
are not an authorization boundary. `policy_promotion` integration tests retain
tampering/revocation rejection; the affected Bazel owner tests verify linkage.

Acceptance fixtures and protected-input loading share the public, bounded
`config::policy_acceptance::parse_suite` entry point. Strict YAML parsing and
suite validation stay within configuration ownership; the generic YAML helper
remains private. The suite-parser unit test rejects duplicate keys and oversized
inputs, existing `evolution-suite-*` fixtures retain their goldens, and the
application Bazel target checks that the separate owner crates link correctly.

Dynamic category schemas, parameter contracts and mutation validation belong to
`config`. Its project-rule inventory uses bounded blocking threads for file
reads and schema parsing, then combines results in deterministic path order.
The interfaces layer invokes discovery and mutations through `spawn_blocking`.
No owner dependency direction changes. See [rule management](rule-management.md).

The Issue 9 built-ins keep parameter validation in `config` and Java syntax,
literal-file matching and Maven-fact consumption in `adapters`. The adapter's
bounded CPU workers parse independent changed files and scan added files; they
join before publishing ordered evidence. No owner dependency direction changes.

Category memberships and evidence/candidate/version/transition records belong
to `domain`. `config::policy_store` owns confined content-addressed objects,
bounded reads and atomic archive-index publication; `policy_candidates` validates
and archives policy inputs and mutations. Application code acquires the chosen
Git parent and passes borrowed bytes for only policy-owned files. Context reads
parse the selected immutable Git bytes on a blocking worker without materializing
the repository. CLI parsing
delegates these operations without filesystem access. No dependency direction
changes; see the [policy evolution contract](policy-evolution.md).

Paired scheduling and signed activation/rollback orchestration belong to
`application`; Ed25519/DSSE authentication stays in `adapters`. Configuration
owns strict external acceptance schemas, immutable publication, lifecycle
selection and bounded history reads. Pure oracle decisions and longitudinal
measurements belong to `domain`. The scheduler shares one immutable snapshot
between baseline and candidate, bounds live cases and global execution permits,
and archives completed pairs incrementally. `runner` owns executable identity
hashing; `env` owns inherited-environment enumeration. All blocking reads and
serialization run outside asynchronous orchestration. See the
[validation boundary](policy-validation.md).

`snapshot::io_workers` owns bounded parallel materialization and input-guard
I/O, with four workers per executing check and a shared 30-second deadline.
It joins all workers before returning and preserves ordered results; path
confinement remains in `paths`, without a dependency on configuration loading.

`config` calls `env` only to resolve deployment-owned Skill rule assets. Project policy files remain owned by the selected configuration snapshot and confined through `paths`; this keeps runtime rule discovery separate from repository policy loading.

Every top-level owner has an explicit directory below src/qualitygate:
domain, env, paths, net, runner, snapshot, config, adapters, application, and
interfaces. This keeps the ownership name in the physical source path and lets
each owner define its Bazel target in its own BUILD.bazel file. Nested areas
remain within their owner path, such as adapters/reports and
config/discovery; they do not become unowned top-level buckets.
Snapshot history is an explicit cross-owner data contract for declaration
adapters; its consumer validates ancestry, tree digests, and comparison
identity before producing evidence.

Snapshot acquisition owns caller budgets and a semaphore shared by base/target
Git batches and worktree content reads. Object preflight and batch framing stay
in `snapshot`; process lifetime, stream limits and cancellation remain in
`runner`. Filesystem reads, parsing, change mapping and hashing use blocking
workers, with bounded task queues. The CLI passes budget/path options through
the application to snapshot acquisition and revalidation. No owner direction
changes; see [large repository acquisition](large-repositories.md).

Selfcheck fixture/golden loading belongs to `config`; compiled JSON assets are
declared as Bazel compile data. The root Bazel filegroup includes every
`lifecycle-rule-matrix*.yaml` supplement compiled into `config`, so added
language rule packages remain available in sandboxed Bazel builds.
`application` runs bounded production evaluator
and native temporary-repository scenarios on a blocking worker. Pure golden
comparison, result contracts and verification boundaries belong to `domain`;
`interfaces` selects filters and renders those results. Selfcheck adds no owner
or dependency direction and executes no candidate project commands.

Test effectiveness configuration and validation remain in `config`; shared
case/proof contracts and pure comparison belong to `domain`. `snapshot`
composes immutable files without depending on configuration types. The JUnit
adapter normalizes per-case facts, `application` orchestrates two bounded
executions through `runner`, and `interfaces` renders their evidence. Required
text and lower entity counts extend the existing rule adapter. No owner or
allowed dependency direction changes; see [test effectiveness](test-effectiveness.md).

## Checked evidence

The harness follows Rust module declarations from both crate roots. It parses grouped imports, re-exports, aliases, qualified/relative paths, inline modules and qualified references/imports inside macro arguments. File names containing `test` do not exclude production code. Only configurations proven inactive when `test=false` are omitted; platform and feature branches remain in the graph, including `cfg(not(test))`.

The gate rejects forbidden directions and dependency cycles. It also rejects known filesystem/process/network/environment capabilities in the domain or interface layer, environment access outside `env`, ambiguous/missing module files and unsupported production source redirection/generation. It does not expand arbitrary procedural macros or analyze third-party implementation internals; compilation, native-platform tests, Miri and ASan remain separate evidence.

`target/architecture/report.json` records source SHA-256 digests, file/line dependency references and violations. `target/architecture/graph.dot` presents the observed owner graph. The independent CI architecture job uploads both artifacts, including on test failure when artifacts were produced. A source or policy change requires rerunning the gate.

## Documentation gate

Run `cargo test --locked --test quality documentation`. A CommonMark parser extracts rendered headings, inline/reference/image links and code blocks, so examples inside code do not become document links. Local checks cover files, same-file and cross-file Markdown anchors, percent-encoded paths, Chinese headings, inline formatting, duplicate heading suffixes, basic explicit HTML anchors and unclosed backtick/tilde fences.

Undefined link references, missing targets/anchors, repository escapes and unsupported local fragment formats fail with a file and, where available, line. HTML link/anchor values containing entities and fragments on non-Markdown files currently require explicit support. External HTTP(S), mail and data links are recognized without network availability checks. The gate does not claim a complete GitHub renderer.

Authored files retain the 1,000-line limit (`Cargo.lock` is exempt); file inventory and input sizes are bounded and symlinks are rejected. Repository YAML files must parse as configuration mappings. Parser fixtures exercise both accepted Markdown and deliberate broken-link/fence cases.

Production implementation sources are Rust. BUILD.bazel files below src are
declared Bazel package metadata and are excluded from the Rust parser check.

Reference semantics: [CommonMark parsing](https://docs.rs/pulldown-cmark/0.13.4/pulldown_cmark/) and [GitHub section links](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#section-links).

## Agent feedback ownership

`domain::feedback` is a pure bounded projection of a full report.
`application::feedback` computes its reference to the already persisted JSON
on a blocking worker; `interfaces::cli` selects the view. Report context records
original and full delivery replay commands. No owner gains model/network work.
The optional `examples/agent_loop.rs` is an external application using the public
CLI and bounded runner; it does not implement gate decisions. See
[Agent feedback](agent-feedback.md) and [external loop](agent-loop.md).

## Pilot evidence ownership

`domain::case_provenance` and `domain::pilot` contain serializable contracts and
pure lineage/metric decisions. `config` owns bounded archive, manifest and report
reads; `application::pilot` moves those blocking reads off async orchestration;
`interfaces` only parses pilot commands and renders the result. The
`adapters::pilot_authorization` and `adapters::pilot_acceptance` authenticate
DSSE/Ed25519 owner and independent-reviewer records,
while `application::external` owns bounded repository-external reads and rechecks.
No domain or interface I/O
and no owner dependency direction changes were introduced. See
[phase C](pilot-phase-c.md), [phase D](pilot-phase-d.md),
[phase E](pilot-phase-e.md), [phase F](pilot-phase-f.md) and
[phase G](pilot-phase-g.md), [phase H](pilot-phase-h.md) and
[phase I](pilot-phase-i.md), [phase J](pilot-phase-j.md) and
[phase K](pilot-phase-k.md), [phase L](pilot-phase-l.md) and
[phase M](pilot-phase-m.md), [phase N](pilot-phase-n.md) and
[phase O](pilot-phase-o.md). The v2–v9 budget,
tenth threshold, v3+ task strata, v4+ source roster, v5 run-order audit,
v6 attempt audit, v7 run-level model audit, v8 attempt-level model audit and
v9+ execution-time audit and v10 eight-check nonfinancial assessment
remain pure `domain::pilot` calculations. `config::pilot`
owns bounded source, initial report, model-record and execution-record reads; `application::pilot` rechecks them on a
blocking worker before output. No owner direction changes.
