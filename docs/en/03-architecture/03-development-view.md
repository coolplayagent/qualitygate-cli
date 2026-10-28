# Development view

[简体中文](../../zh/03-architecture/03-development-view.md) · [Volume index](README.md)

The development view maps logical responsibilities to Rust source owners,
allowed imports, test surfaces, and build-time enforcement. It answers “where
does a change belong, and what may it depend on?”

The Rust crate uses explicit top-level owners and an acyclic dependency graph.
The crate root declares owners only; the executable top level delegates to the
interface layer.

```mermaid
flowchart TD
    main --> interfaces
    interfaces --> application
    interfaces --> config
    interfaces --> snapshot
    application --> adapters
    application --> config
    application --> snapshot
    application --> runner
    application --> net
    adapters --> config
    adapters --> snapshot
    snapshot --> net
    snapshot --> paths
    snapshot --> runner
    config --> paths
    paths --> env
    net --> env
    runner --> env
    interfaces --> domain
    application --> domain
    adapters --> domain
    config --> domain
    snapshot --> domain
    net --> domain
    runner --> domain

    classDef pure fill:#eef7ee,stroke:#2f6f3e,color:#183d22
    classDef boundary fill:#fff4df,stroke:#9a6700,color:#4d3500
    classDef orchestration fill:#eaf2ff,stroke:#315f9b,color:#17365d
    class domain pure
    class env,paths,net,runner,snapshot boundary
    class application,interfaces,main orchestration
```

Arrows mean “may depend on.” All owners may use pure domain contracts; only the
named boundary owners may touch their corresponding external capability. The
graph is intentionally asymmetric: for example, adapters may consume snapshot
facts, but snapshots never import an adapter; interfaces call application use
cases, but application never parses CLI arguments.

| Owner | Responsibility |
| --- | --- |
| `domain` | Serializable contracts and pure gate decisions; no process, filesystem, network, or environment I/O. |
| `env` | Environment-variable reads and environment-derived values. |
| `paths` | Confined path validation and safe resolution. |
| `net` | Bounded network ownership and remote acquisition contracts. |
| `runner` | Bounded asynchronous process execution and captured evidence. |
| `snapshot` | Git selectors, immutable file acquisition, change mapping, and materialization. |
| `config` | Strict policy/task loading, validation, and planning. |
| `adapters` | Language/project facts and external report normalization. |
| `application` | Check orchestration, feedback, rechecks, and use-case coordination. |
| `interfaces` | CLI parsing and output rendering. |
| `main` | Process entry point delegating to `interfaces`. |

Shared contracts belong in `domain`, not in the CLI or a concrete adapter.
Interfaces do not perform external I/O directly. Blocking filesystem and parser
work stays isolated from asynchronous orchestration. Network, environment, and
path access have dedicated owners so capability review is explicit.

`config::yaml_budget` owns the shared YAML expansion preflight. It visits values
without materializing their collections or copying alias strings, before the
existing duplicate-key and typed decoders. Configuration, tasks, rules and
validation suites share this boundary. Application policy-trust loading calls
the public, typed `config::policy_acceptance::parse_trust` entry point inside its
existing blocking work. This boundary also works when Bazel compiles owners as
separate crates; the generic YAML decoder remains private to configuration.

`domain::xml_budget` owns pure namespace-work accounting shared by configuration
discovery and adapters. It consumes counts, bounds its scope stack and checks
arithmetic; XML tokenization, tree parsing and profile-specific errors remain in
the consuming owners.

Within `adapters`, `xml_document` owns bounded XML tree parsing for reports,
Maven models and compatibility evidence. Callers enforce their byte budgets;
report-specific DOCTYPE normalization stays in `reports`. Iterative preflight also
limits attributes to 1,024 per element and 100,000 per document, including namespace
declarations, before the tree parser's duplicate-name comparisons. Project and compatibility
adapters depend on this shared leaf without importing report-format machinery.

Configuration discovery owns its advisory Maven manifest validation in
`config::discovery::maven_manifest`. It bounds XML depth, nodes and attributes before deriving
command candidates, and reports unsupported manifests as discovery gaps. This
profile stays within `config`; it neither imports adapters nor produces resolved
project facts. Adapter report failures retain their separate execution semantics.

The architecture test parses every production Rust source, resolves module and
import paths (including grouped, aliased, macro, and conditional references),
checks owner edges and forbidden capabilities, detects cycles, and writes a
reviewable source-digest/line report plus DOT graph. It rejects hidden root
re-exports, ambiguous module files, unknown owners, broad external globs, and
I/O from pure domain code.

Documentation, examples, tests, and build metadata may exercise the public
surface but do not create another production owner. When module ownership or
imports change, run the architecture gate and inspect its reported edges rather
than accepting compilation alone.

This separation supports three independent review questions: whether a decision
is pure and serializable, whether an external action is confined and bounded,
and whether orchestration combines those capabilities without creating a cycle.
It also keeps the reusable library independent from the executable interface.

---

[Previous: Process view](02-process-view.md) · [Contents](../README.md) · [Next: Physical view](04-physical-view.md)
