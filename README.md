# qualitygate-cli

A Rust CLI for repository policy and task acceptance checks. Results distinguish violations from incomplete execution and bind evidence to the code and policy that were checked.

The implementation follows [REQUIREMENTS.md](REQUIREMENTS.md). Implementation and verification progress is tracked in [docs/implementation.md](docs/implementation.md); a requirement is complete only when its behavior and tests exist.
The [acceptance evidence audit](docs/acceptance-evidence.md) distinguishes the repeatable repository tests from the team-owned real-repository pilot evidence.

## Development

```bash
cargo build --locked
cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

The CLI, checks and test harnesses are implemented in Rust. Git is required for repository snapshots; configured build and test commands require their own project toolchains.

See [quality gates](docs/quality.md) for CI, coverage and deep verification.
See [selfcheck](docs/selfcheck.md) for bundled fixture regression and explicit verification boundaries.
See [Bazel builds](docs/bazel.md) for cached Bzlmod builds and Cargo-aligned dependency checks.
See [external engineering standards](knowledge/best-practices/engineering-standards/README.md) for the reviewed Alibaba, Google, Huawei, NVIDIA, AWS, Azure, Cloudflare and Meta source archive, lifecycle matrix, and rule mapping.
See [initialization](docs/init.md) for nested project discovery, capability gaps and command suggestions.
See [manual acceptance](docs/manual-acceptance.md) for signed review records and caller-controlled trust.
See [task plans](docs/tasks.md) for acceptance contracts, selected policy versions and reproducible rechecks.
For Agent integration, see [bounded repair feedback](docs/agent-feedback.md),
the [external Rust loop](docs/agent-loop.md), and [phase-B evidence](docs/pilot-phase-b.md).
Use [pilot evidence summaries](docs/pilot-phase-c.md) for protected case
provenance, assignment-based aggregation and the read-only `pilot summarize` command.
Use [pilot plan sealing](docs/pilot-phase-d.md) to freeze the complete model/workflow
assignment matrix before recording observations.
Use [pilot start authorization](docs/pilot-phase-e.md) to bind that sealed plan to
an external human-owner DSSE/Ed25519 signature before the observation window.
Use [pilot final acceptance](docs/pilot-phase-f.md) to bind completed observations
and threshold results to an independent reviewer decision.
Use [structured pilot budgets](docs/pilot-phase-g.md) to seal a v2 total cap and
include all attempts and human review time in the final acceptance subject.
Use [stratified pilot sampling](docs/pilot-phase-h.md) to seal eight independent
tasks with a declared 4 bug-fix / 4 refactor mix before observation.
Use [task-source artifacts](docs/pilot-phase-i.md) to bind each new pilot input
to a bounded, digest-verified issue or commit record in the external archive.
Use [pilot run-order auditing](docs/pilot-phase-j.md) to seal alternating
workflow order and record actual start positions before final acceptance.
Use [pilot repair-attempt auditing](docs/pilot-phase-k.md) to bind initial full
reports and enforce retry, elapsed-time and no-progress stop rules.
Use [pilot model evidence](docs/pilot-phase-l.md) to archive each observed run's
requested configuration, capture time and reported or explicitly unknown actual model.
Use [attempt model evidence](docs/pilot-phase-m.md) for new plans so every retry,
including failed and timed-out attempts, has a separately checked model record.
Use [attempt execution evidence](docs/pilot-phase-n.md) for new plans to bind
archived start/end times to retry budgets and declared run order.
Use [unpriced pilot acceptance](docs/pilot-phase-o.md) for new v10 plans to
seal without hourly rates or prices while retaining eight required checks.
See [pilot readiness](docs/pilot-readiness.md) for the remaining real-task,
governance and independent-review inputs needed to finish the trial.
See [agent-run provenance](docs/provenance.md) for authenticated transformation histories and AI-only test scope.
See [Git trailer bindings](docs/git-trailers.md) for declarations associated with actual test-changing commits.
See [test effectiveness](docs/test-effectiveness.md) for explicit old-code counterexamples from independent test files.
See [tool reports](docs/reports.md) for test counts, coverage, analyzer output and incremental comparison.
See [coverage reports](docs/coverage.md) for JaCoCo, Cobertura and coverage.py completeness and repair evidence.
See [SARIF analysis](docs/sarif.md) for indexed locations, baseline identities and real Clippy acceptance.
See [Java interface compatibility](docs/compatibility.md) for paired snapshot builds and binary/source API checks.
See [built-in rules](docs/rules.md) for Skill-owned rule assets, language adapters and team-specific configuration.

Use [rule management](docs/rule-management.md) for dynamic categories,
progressive discovery, enable/disable, parameter descriptions and atomic
configuration changes. Large rule directories use bounded parallel loading.
See [custom rules](docs/custom-rules.md) for rule packages, the versioned DSL and its capability limits.
See [source reviews](docs/source-reviews.md) for reviewed mappings from normative sections to executable rules.
See [project facts](docs/projects.md) for Maven dependency pairing and rule/command prerequisites, and [Python project facts](docs/python-projects.md) for installed dependency verification.
See [merge requests](docs/merge-requests.md) for GitHub/GitLab comparisons, credentials and snapshot evidence.
See the [skill-over-CLI release package](docs/skill-package.md) for the
versioned agent bundle, platform assets, and publication boundary.

## Current command surface

```bash
cargo run -- init
cargo run -- init --with-checks --format json
cargo run -- check --worktree --profile quick --format json
cargo run -- check --staged --format markdown
cargo run -- check --diff HEAD~1..HEAD --format json
cargo run -- check --mr https://github.com/owner/repository/pull/123 --format markdown
cargo run -- rules list
cargo run -- rules list --language rust --source all --format json
cargo run -- rules schema
cargo run -- rules enable commit-message
cargo run -- config --show
cargo run -- selfcheck
cargo run -- selfcheck --fixture minimal --rule commit-message
cargo run -- pilot seal --input pilot-plan.json --format json
cargo run -- pilot authorization-subject --input observations.json --format json
cargo run -- pilot acceptance-subject --input observations.json --trust-store /external/trust.json --authorization /external/start.dsse.json --format json
cargo run -- pilot summarize --input observations.json --format json
```

`check` returns 0 for a complete passing gate, 1 for blocking violations, and 2 for incomplete validation. `quick` and `--path` cover only their selected scope and cannot establish delivery readiness. Generated logs and reports live under `QUALITYGATE_HOME` (default: a qualitygate directory in the OS temporary directory); `--output-dir` overrides the evidence location. Commands run against a disposable materialization of the selected snapshot.

Snapshots support 100,000 files and default to 256 MiB total contents per tree, with 2 MiB per file and 16 MiB per captured process stream. Git objects are size-checked before content acquisition and read in bounded parallel batches. Use `--snapshot-max-mib` (1–1024), `--snapshot-jobs` (1–16, default 4), and `--snapshot-timeout-secs` (1–3600, default 120) to set acquisition budgets. Exceeding a limit reports incomplete validation. Git symlinks, submodules and unresolved index conflicts also require explicit handling and currently fail closed. See [large repositories](docs/large-repositories.md) for scope, memory and performance evidence.
