# Python project facts

Python dependency assertions combine static `pyproject.toml` declarations, a successful pip installation report and installed Core Metadata. Distribution names, environment markers and extras use PEP 508/440 semantics.

## Producer and rule

```yaml
schema_version: 1
custom_rules: team-rules
rules:
  declared-tests:
    depends_on: [python-facts]
checks:
  - id: python-facts
    cwd: .
    argv:
      - python3
      - -E
      - -P
      - -m
      - pip
      - --isolated
      - install
      - --ignore-installed
      - --no-cache-dir
      - --no-compile
      - --target
      - target/python
      - --report
      - target/pip.json
      - '.[test]'
    timeout_seconds: 300
    tools:
      - {id: python, argv: [python3, -E, -P, --version]}
      - {id: pip, argv: [python3, -E, -P, -m, pip, --version]}
    projects:
      - ecosystem: python
        root: .
        source_root: src
        test_source_root: tests
        install_target: target/python
        install_report: target/pip.json
        extras: [test]
profiles:
  quick:
    include: [declared-tests]
```

`team-rules` must contain a source-mapped custom definition such as:

```yaml
id: declared-tests
version: 1
source:
  document: AGENTS.md
  section: Test dependencies
  content_hash: 'sha256:<replace with the actual section digest>'
language: [python]
requires_capabilities: [test_methods, comments, dependency_resolution]
applies_to:
  paths: ['tests/**/*.py']
  provenance_scope: all_added_tests
binding:
  marker: {type: comment, name: Generated, fields: [author]}
when: {entity: test_method, change: added}
then:
  require_marker: true
  require_dependency: {artifact: pytest}
fix: Restore the honest declaration and declared test dependency, then rerun
```

Python assertions accept an omitted `group` or `group: pypi`; `artifact` is a normalized distribution name. `pytest` and `PyTest` identify the same distribution. A package must be directly declared in base requirements or a selected optional extra, active for the interpreter environment, resolved with a compatible version, and installed. Global packages and solely transitive dependencies do not satisfy the assertion.

The project needs static PEP 621 `name` and `version`, and may declare `dependencies`, `requires-python` and `optional-dependencies`. `extras` selects up to 64 unique normalized names. Install `.` when none are selected, or `.[extra1,extra2]` with names sorted. Dependency extras propagate through the graph until no additional extras activate.

## Scope and evidence

All project paths are repository-relative. `cwd` must equal `root`. Source/test/install roots must be non-overlapping subdirectories of the module; the report must be outside the install target. For nested modules, `--target` and `--report` command arguments are relative to `cwd`. Each Python command supplies one project.

The installation target must be absent in the materialized snapshot. qualitygate creates the report's parent directory and removes stale reports. The command requires the declared interpreter, `-E -P -m pip --isolated install`, `--ignore-installed`, matching target/report paths and project extras. Python/pip probes use the same interpreter flags. [`-E` and `-P`](https://docs.python.org/3/using/cmdline.html#cmdoption-P) prevent Python environment overrides and current-directory module shadowing; this contract needs Python 3.11 or newer. Dry runs, dependency omission, other requested packages, interpreter/platform overrides, global installation switches and unknown options are rejected. Optional flags are `--no-compile`, `--no-warn-script-location`, `--disable-pip-version-check`, `--no-input`, `--no-cache-dir`, and explicit `--index-url`/`--extra-index-url` values.

A report alone cannot prove installation: pip can write it before installation finishes. The producer requires exit code zero and matching `.dist-info/METADATA` files. Raw report and metadata artifacts are retained with command logs, executable identities, Python/pip version probes, timings and snapshot guards. `metadata.projects[].python` records the environment, extras, target and installed metadata digests.

Validation covers report version 1, project name/version and local origin, requested extras, declared requirements, interpreter compatibility and every active transitive dependency. Missing or duplicate distributions, incompatible versions, unreachable packages, inconsistent metadata and foreign/ambiguous test owners leave checks incomplete. String marker comparisons such as `sys_platform == 'win32'` follow PEP 508 string semantics.

Python facts use the common project protocol with `ecosystem: python`; Maven's existing configuration remains readable. Tests route by both language and test root. A mixed-language dependency graph cannot substitute Maven facts for Python facts. Marker retention also checks unchanged marked tests after manifest-only edits.

## Running tests and limits

Installation facts prove dependencies in the target, not successful tests. Add a downstream test command using that target, actual tool probes, and a JUnit report with `minimum_tests: 1`. Set `findings_exit_codes: [1]` for pytest assertion failures. On Unix, the live fixture uses `env PYTHONPATH=target/python python3 -P -m pytest --junitxml=target/junit.xml tests`; use the repository's platform-appropriate launcher elsewhere.

Metadata is limited to 2 MiB per file, 12 MiB total and 4,096 distributions. Scans and resolution run on workers with 30-second budgets; requirement lengths/nesting and graph traversal are bounded. Symlink metadata cannot escape the workspace.

Dynamic metadata, Poetry-only/legacy manifests, dependency groups, direct URLs, additional source roots and import-to-distribution ownership need further support. This adapter does not claim Python used-but-undeclared detection or seal every installed file and external build/cache input. These gaps remain in [the ledger](implementation.md). Source comments declare provenance; they do not prove an agent generated the code.

References: [pyproject specification](https://packaging.python.org/en/latest/specifications/pyproject-toml/), [pip installation reports](https://pip.pypa.io/en/stable/reference/installation-report/), [dependency specifiers](https://packaging.python.org/en/latest/specifications/dependency-specifiers/) and the Rust [pep508_rs library](https://docs.rs/pep508_rs/0.9.2/pep508_rs/).

## Verification

Rust fixtures cover markers, extras, names, transitive closure, metadata, missing/foreign facts, mixed-language routing, configuration migration and failures. The independent `python-project` CI job uses Python 3.12, pip 26.0.1 and pytest 8.4.2:

```bash
cargo test --locked --test python -- --ignored --nocapture
```

`QUALITYGATE_TEST_PYTHON` selects the interpreter. The five-run fixture checks a passing installed test, manifest-only dependency deletion, a failed assertion, repaired code and removed declaration. A failing repository-local `pip.py` confirms that the real installer is used. It is an isolated integration fixture, not the real-repository pilot or human-review measurement required by §9.2.
