# Project facts and dependency pairing

Java dependency assertions consume fresh Maven output from an explicitly named prerequisite. Effective models supply declarations and source roots; resolved trees supply artifact identities and scopes. Import spelling does not establish package ownership.

## Maven producer

```yaml
schema_version: 1
custom_rules: team-rules
rules:
  declared-tests:
    depends_on: [maven-facts]
checks:
  - id: maven-facts
    argv:
      - mvn
      - -B
      - -ntp
      - -N
      - org.apache.maven.plugins:maven-help-plugin:3.5.1:effective-pom
      - -Doutput=target/effective.xml
      - org.apache.maven.plugins:maven-dependency-plugin:3.8.1:tree
      - -DoutputType=json
      - -DoutputFile=target/tree.json
    timeout_seconds: 300
    tools:
      - {id: maven, argv: [mvn, --version]}
      - {id: java, argv: [java, -version]}
    projects:
      - root: .
        effective_pom: target/effective.xml
        dependency_tree: target/tree.json
profiles:
  quick:
    include: [declared-tests]
```

The custom definition must exist under `team-rules`. Add the repository's Maven settings, profiles, wrapper inputs and mandatory arguments. Pin plugin coordinates in the actual invocation. Maven/Java probes record installed tools; retained logs record plugin execution. Trusted policy must declare the real toolchain and relevant inputs; this does not seal external Maven caches or discover every nested executable.

Output paths are repository-relative, including when `cwd` names a module. For each module, use its own `cwd`, `projects[].root` and distinct output paths. Each effective POM must contain one project. Use separate non-recursive Maven invocations, or a trusted repository command producing distinct pairs. Reactor-wide effective POMs are rejected.

The adapter follows Maven's [effective POM](https://maven.apache.org/plugins/maven-help-plugin/effective-pom-mojo.html) and [JSON dependency tree](https://maven.apache.org/plugins/maven-dependency-plugin/examples/tree-mojo.html) formats. Live acceptance verifies Maven 3.9.16, Help Plugin 3.5.1 and Dependency Plugin 3.8.1 with JDK 21.

## Assertions and retention

Combine these fields with a custom definition's ID, version, source mapping and fix:

```yaml
language: [java]
applies_to:
  paths: ['src/test/**/*.java']
  provenance_scope: all_added_tests
requires_capabilities: [test_methods, annotations, dependency_resolution]
binding:
  marker: {type: annotation, name: Generated, fields: [author]}
when: {entity: test_method, change: added}
then:
  require_marker: true
  require_dependency: {group: junit, artifact: junit}
```

`require_dependency` requires the exact group/artifact to be effectively declared and resolved as an unclassified JAR on the test compile classpath (`compile`, `provided`, `system` or `test`). Solely transitive or `runtime` dependencies do not satisfy it. Diagnostics identify the test, module, manifest and producer.

Dependency validation also visits current marked tests within the configured scope, including unmodified files. Removing only their POM dependency fails. Marker assertions retain the obligation on a surviving or moved test that had a baseline declaration; removing it cannot bypass an `added` filter. Naming/text assertions retain their explicit change selection. Declarations do not prove actual AI provenance.

## Completeness and evidence

Rules and commands share one dependency graph. `depends_on` is supported on rule settings, command checks and task verifications. Profiles include transitive prerequisites. Unknown/repeated dependencies and cycles are configuration errors; disabled prerequisites cannot be scheduled. A prerequisite must complete with a passing verdict before its consumer runs. `plan.execution_order` lists the planned order; omitted delivery checks remain visible.

Maven producers require exit code zero, actual version probes and fresh bounded outputs. Stale outputs are removed first. Raw XML/JSON artifacts, hashes, process logs, timings, input guards and normalized `metadata.projects` bind facts to that execution and snapshot. Rules consume facts only from explicit passing prerequisites.

Malformed, missing, mismatched or filtered outputs remain incomplete. Model and tree must agree on project identity and declared dependency versions/scopes; unsupported differences, including unresolved expressions or incompatible version-range representations, remain incomplete. Effective absolute source roots must stay inside the configured module and materialized repository. Extra roots added dynamically by plugins are not inferred: tests outside resolved test roots lack module evidence. Missing/ambiguous owners, foreign snapshots and unsupported ecosystems remain incomplete.

This adapter implements Maven test dependency pairing. Used-but-undeclared bytecode analysis, module boundaries, Gradle/Python project facts and compatibility integrations remain in [the ledger](implementation.md).

## Verification

Parser and failure fixtures run in ordinary Rust suites. The independent `maven-project` CI job executes real Maven using an isolated temporary artifact repository:

```bash
cargo test --locked --test maven -- --ignored --nocapture
```

`QUALITYGATE_TEST_MAVEN=/absolute/path/to/mvn` selects a local installation. This test is explicitly ignored in toolchain-free Rust runs and required by its dedicated job; absence of Maven there fails. It verifies resolution, dependency deletion, repair, marker removal and runtime scope. These controlled fixtures do not establish the real-repository pilot or human-review measurements of §9.2.
