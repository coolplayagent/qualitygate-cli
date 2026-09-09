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

This adapter implements Maven test dependency pairing, dependency directions and compiled dependency usage. Gradle/Python project facts and compatibility integrations remain in [the ledger](implementation.md).

## Module dependency boundaries

Activate `lang-java` and configure `module-boundary` with the complete inventory of modules this check promises to cover:

```yaml
rulesets: [lang-java]
rules:
  module-boundary:
    depends_on: [facts-core, facts-infra]
    parameters:
      modules: [core, infra]
      dependency_kind: declared
      forbidden:
        - from: 'com.example:core'
          to: 'com.example:infra'
          scopes: [compile, runtime]
```

The named `facts-core` and `facts-infra` checks must produce their respective Maven project facts as described above. In a reactor, make them depend on a repository build/install step so local artifacts are compiled and resolved from the same snapshot. The live reactor fixture demonstrates this sequence.

`modules` contains unique repository-relative module roots (`.` for the root project), with 1–256 entries. Each module requires exactly one passing facts producer and a present manifest. Duplicate project identities, missing modules, unsupported ecosystems or another snapshot's facts leave the rule incomplete. The explicit inventory prevents an empty or partial fact set from being treated as a complete architecture check.

`from` and `to` are `group:artifact` glob patterns, without versions. `forbidden` requires 1–256 constraints. Omitted or empty `scopes` matches all Maven scopes; otherwise it accepts unique values from `compile`, `provided`, `runtime`, `test` and `system`. For example, a test-only restriction does not prohibit a compile-scope edge.

`dependency_kind: declared` checks direct effective declarations. `resolved` checks the module's resolved dependency set, including transitive classpath reachability. It does not claim a direct declaration for each transitive relationship. Duplicate resolution paths or overlapping forbidden constraints produce one diagnostic per module, target, type, classifier and scope.

This rule uses `full` mode: it examines every configured module and can report existing violations in unmodified manifests. It does not describe those violations as newly introduced. Metadata records mode, dependency kind, module inventory and producers. Diagnostics preserve direction, scope, policy matches and evidence location; fingerprints do not change merely because a version or constraint order changes.

The rule checks dependency directions, not arbitrary method/package access. Existing architecture tools can supply their own check reports for those additional contracts. Analysis runs on a worker with a 30-second budget.

## Used but undeclared Maven dependencies

Enable `used-undeclared` with an explicit inventory of 1–256 module roots. Each prerequisite must produce that module's effective model, resolved tree and completed bytecode usage analysis:

```yaml
schema_version: 1
rulesets: [lang-java]
rules:
  used-undeclared:
    depends_on: [usage-facts]
    parameters:
      modules: ['.']
checks:
  - id: usage-facts
    cwd: .
    argv:
      - mvn
      - -B
      - -ntp
      - -N
      - -Dverbose=false
      - -DscriptableOutput=false
      - -DoutputXML=false
      - -DfailOnWarning=false
      - -Dmdep.analyze.skip=false
      - -Dmdep.analyze.excludedClasses=
      - -Danalyzer=default
      - -Dstyle.color=never
      - clean
      - test-compile
      - org.apache.maven.plugins:maven-help-plugin:3.5.1:effective-pom
      - -Doutput=target/effective.xml
      - org.apache.maven.plugins:maven-dependency-plugin:3.8.1:tree
      - -DoutputType=json
      - -DoutputFile=target/tree.json
      - org.apache.maven.plugins:maven-dependency-plugin:3.8.1:analyze-only
    timeout_seconds: 600
    tools:
      - {id: maven, argv: [mvn, --version]}
      - {id: java, argv: [java, -version]}
    projects:
      - root: .
        effective_pom: target/effective.xml
        dependency_tree: target/tree.json
        dependency_usage: true
profiles:
  quick:
    include: [used-undeclared]
```

`dependency_usage` defaults to false, preserving existing facts producers. When true, each command covers one non-recursive module, with `cwd` equal to its root. Run `clean`, `test-compile`, model/tree generation, and the pinned analysis goal last. The displayed analysis properties are required and cannot have conflicting duplicates. Add your repository's trusted settings and tool inputs. Reactor artifacts must first be built from the same snapshot, as described above.

The adapter consumes the actual captured stdout of the successful command. It recognizes the pinned plugin execution and complete analysis sections, cross-checks reported artifact coordinates against resolved/direct dependencies, and requires the logged main/test compilation counts to match snapshot Java source inventories. Empty logs, skipped analysis, unsupported plugin versions, truncated sections, missing compilation, failed builds and contradictory facts remain incomplete. Logs and normalized usage facts are retained with tool, command and snapshot evidence.

The effective model cannot suppress usage findings with exclusions or a custom analyzer. Compiler source filters and output overrides that prevent complete source accounting are also rejected. Generated sources, additional source roots, alternate compiler logging and repositories without Java sources need further adapter support; this check does not silently accept their incomplete inventories.

The rule examines all compiled Java sources in each configured module in `full` mode. Existing violations in unmodified manifests remain visible. Diagnostics identify the artifact, type, classifier, scope, manifest and producer, with a stable fingerprint independent of artifact version. Repair by declaring the actual dependency directly with the correct scope, or by removing its use and rebuilding. Unused declarations and test-scope suggestions from Maven establish analysis completion but are not violations of this rule.

Maven's [analysis goal](https://maven.apache.org/plugins/maven-dependency-plugin/usage.html) uses bytecode, so reflection and runtime resource loading are outside its proof. The adapter's section/configuration contract is pinned to [Dependency Plugin 3.8.1 source](https://github.com/apache/maven-dependency-plugin/blob/maven-dependency-plugin-3.8.1/src/main/java/org/apache/maven/plugins/dependency/analyze/AbstractAnalyzeMojo.java). It does not infer dependency ownership from import spelling.

## Verification

Parser and failure fixtures run in ordinary Rust suites. The independent `maven-project` CI job executes real Maven using an isolated temporary artifact repository:

```bash
cargo test --locked --test maven -- --ignored --nocapture --test-threads=1
```

`QUALITYGATE_TEST_MAVEN=/absolute/path/to/mvn` selects a local installation. These tests are explicitly ignored in toolchain-free Rust runs and required by the dedicated job; absence of Maven there fails. They verify pairing and a compiled two-module reactor: a forbidden direction fails, deleting only its dependency breaks compilation and blocks the consumer, and repairing the caller plus dependency passes. The usage fixture compiles code that uses a transitive Hamcrest dependency, requires its direct declaration, verifies the repair, and blocks after removing its only provider. These controlled fixtures do not establish the real-repository pilot or human-review measurements of §9.2.
