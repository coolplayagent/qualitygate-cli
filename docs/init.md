# Discovering a candidate policy

`qualitygate init` scans the selected working directory, creates a new configuration candidate when absent, and reports project roots, languages, available rule packages, marker options, command suggestions and capability gaps. Discovery does not execute scripts, build tools, package installation or tests.

```bash
qualitygate init --format json
qualitygate init --with-checks --config policy/qualitygate.yaml --format markdown
```

The default candidate enables `line-ending` and records detected languages. `--with-checks` also copies suggestions marked `ready_for_review` into a **new** candidate, with actual version probes and relevant verification assets. Test suggestions that need report mappings remain in the discovery output. Language-specific naming, markers and dependency rules still require explicit team policy.

Existing configuration bytes are preserved, including with `--with-checks`. The result sets `created: false` and `with_checks_applied: false` while displaying current discovery information. New publication is atomic and refuses to replace a concurrently created file. Selected configuration paths and parent directories stay inside the repository.

## Discovery output

JSON retains `schema_version`, `source`, `config` and `status: candidate`, adding `created`, `with_checks_applied` and `discovery`. Table and Markdown present the same metadata.

| Field in discovery | Meaning |
|---|---|
| `scope`, `commands_executed` | Working-directory discovery with no commands executed; this is not a checked Git snapshot or delivery result |
| `languages` | Language, source-file count, available syntax capabilities and marker binding options |
| `projects` | Repository-relative root, ecosystem, manifest, metadata inspection status and available project capabilities |
| `inputs` | SHA-256 digests of inspected manifests and local ignore files |
| `available_rulesets` | Relevant embedded package/rule IDs, versions and required capabilities |
| `suggested_checks` | Copyable command configuration, purpose, source, review requirements and adoption status |
| `gaps` | Unsupported metadata/languages, missing project-fact configuration, missing test/report conventions and provenance limits |

Metadata inspection derives suggestions; native build tools must validate actual projects. Maven/Python dependency capabilities still require successful facts producers. Marker options describe declarations; AI-only scope and reliable commit-to-entity attribution need trusted external records. The shared syntax table also controls runtime DSL applicability: required Shell imports/test-method assertions remain incomplete even when no entity matches.

## Suggested commands

| Evidence | Suggestion | Review requirements |
|---|---|---|
| `Cargo.toml` | `cargo check`, `cargo test`; `--workspace` for workspaces and `--locked` when a local lockfile exists | Select features, members and lock policy; summaries must show actual executed tests |
| `pom.xml` | Local wrapper when present, otherwise `mvn`; local `settings.xml` becomes a required argument | `compile` covers main-source compilation. `verify` needs fresh Surefire/Failsafe JUnit paths and test-count requirements |
| Python metadata declaring pytest, or `[tool.pytest]` | Interpreter-based pytest with JUnit and `minimum_tests: 1` | Select Python 3.11+ with project/test dependencies installed; assertion exit 1 is a finding |
| `package.json` scripts | Existing `build`, `lint`, `test` scripts using package-manager/lockfile evidence | Build/lint give exit status. Tests need framework-specific report mapping; conflicting managers produce a gap |
| `go.mod` | `go build ./...`, `go test ./...` | Tests need a converter or supported test-count report; select tags and workspace |
| Shell files | Syntax capability and absence of project test conventions | Configure the team's existing verification command |

Project-fact setup is documented for [Python](python-projects.md) and [Maven](projects.md). Initialization does not invent Maven suite filenames, Node reporters or Go converters. Review nested workspace suggestions to avoid redundant ancestor/member runs. A candidate covering only compilation or lint does not establish successful tests or task completion; add required test mappings and a task contract before delivery.

## Inventory and limits

Discovery respects local `.gitignore` files and nested overrides. It does not read global/parent ignore configuration, `.ignore` or Git's private exclude file. Ignored paths are omitted even if separately tracked. Excluded directory names are disclosed: `.git`, `target`, `node_modules`, `.venv`, `venv`, `dist`, `.qualitygate`, `.coverage` and `__pycache__`.

The `.git` metadata entry is excluded whether it is a directory or a file, so linked Git worktrees can initialize their own policy without traversing the shared Git metadata.

Non-ignored symlinks/special files, unreadable inputs, invalid ignore patterns and budget overruns fail without publishing a candidate. Limits are 20,000 entries, 64 directory levels, 512 manifests, 2 MiB per inspected input, 12 MiB total inspected contents, 2,048 suggestions and 30 seconds. Source-file counts describe extension matches; source contents are not parsed or executed.

Malformed manifests and recognized unsupported ecosystems remain gaps. Legacy Python setup code and Gradle/Ruby/PHP/.NET manifests are detected without execution; semantic support is not implied. The [Rust CLI fixture](../tests/init.rs) verifies candidate execution, failed assertions, repair, zero-test rejection and existing-file preservation. It does not replace the real-repository pilot.

Conventions follow [Cargo test](https://doc.rust-lang.org/cargo/commands/cargo-test.html), [pytest JUnit output](https://docs.pytest.org/en/stable/how-to/output.html#creating-junitxml-format-files) and the Rust [ignore parser](https://docs.rs/ignore/0.4.33/ignore/gitignore/).
