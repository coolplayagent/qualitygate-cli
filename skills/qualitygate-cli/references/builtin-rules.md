# Bundled built-in rule guide

The YAML files in `references/rules/` are the versioned built-in rule assets read by the
matching Qualitygate CLI at runtime. Read this guide and the relevant YAML
before proposing a rule. Then use `qualitygate rules list --format json` to
verify what the installed executable can enforce. Do not edit the Skill assets
to change a repository policy; a repository policy selects and configures
rules, while the executable owns evaluation.

## Selection boundary

Use `rules list --language java --source builtin --format json` for built-ins,
`--source project` for `qualitygate/rules` (or an explicit legacy policy path),
and `--source all` for both. Empty definition language scopes apply to every
language. Inventory queries need no initialized policy and include all shipped
packages, even unselected ones; listing does not enable enforcement. When IDs
overlap, both origins remain visible and only the selected one can be enabled.

Listing a rule is read-only. Adding a rule to `qualitygate.yaml`, running
`qualitygate init`, or using `qualitygate rules enable` changes a candidate
policy and needs the user's explicit authorization. A rule's archived
`standard_refs` and `lifecycle_inputs` are provenance for the packaged
definition, not an implicit mandate for every repository.

Prefer the smallest rule set that matches the user's request. A warning can
produce diagnostics while leaving the overall gate non-blocking; it still
needs a reported disposition. Missing syntax, policy, snapshot, or required
execution evidence is incomplete validation, never a clean result.

## General language rules

| Rule | Languages | Default | Use it for |
| --- | --- | --- | --- |
| `test-naming` | Java, Python, Rust, TypeScript, Go | error | added test discovery names |
| `test-naming-strict` | Java | error | added test names matching a team pattern |
| `test-annotation-dependency` | Java | error | annotated added tests and resolved Maven dependency facts |
| `no-hardcoded-secrets` | Java | error | literal credential patterns in added files |
| `parameterized-tests` | Java, Python, Rust, TypeScript, Go | warning | repeated added test shapes |
| `comment-language` | supported parsed comments | warning | an explicitly selected comment-language convention |
| `security-sensitive-api` | Java, Python, Rust, TypeScript, Go | warning | changed uses of configured high-risk API patterns |
| `todo-marker` | supported source files | warning | changed `TODO`, `FIXME`, or `XXX` work markers |

`security-sensitive-api` uses bounded, single-line regex patterns on changed
source lines: Java process launch APIs, Python `eval`/`exec` and pickle loads,
Rust `unsafe` blocks, TypeScript dynamic evaluation/process execution, and Go
process execution. A match is a review signal, not proof of an exploitable
path. Override `parameters.prohibited_patterns` only with reviewed,
language-keyed regex arrays; `paths` and `languages` can scope it.

`todo-marker` uses the same bounded changed-line engine. It asks for an
explicit decision about deferred work rather than assuming that every marker is
a defect or should be deleted.

`commit-message-convention` is a core rule with a ticket-prefixed default
subject pattern. Configure `parameters.pattern` for the team's convention.
`test-naming-strict` defaults to `^should_.+_when_.+` on added Java tests.
`test-annotation-dependency` defaults to `@Test` and
`org.junit.jupiter:junit-jupiter-api`; set `annotation`, `group`, and
`artifact` to the actual project contract, and provide a snapshot-bound Maven
facts producer through `depends_on`. Missing facts are incomplete validation
when an annotated added test is selected; an empty selection passes without a
configured producer prerequisite. Configured prerequisites still run.
`no-hardcoded-secrets` checks added Java files with a configurable single-line
regex and does not prove that every secret or live credential was found.
These Java rules are opt-in and keep a fixed Java scope.

## Architecture rules

`import-boundary` is packaged for Java, Python, Rust, TypeScript, and Go, but
is disabled by default. It checks only added syntax imports and deliberately
requires an explicit `forbidden_imports` mapping when enabled. It cannot prove
resolved dependency direction, runtime reachability, or a complete threat
model. For Java Maven dependency facts, use the separate `module-boundary` and
`used-undeclared` rules with their documented project prerequisites.

Go imports are represented once per import spec, including grouped imports.
Patterns see `import ` followed by the optional alias and quoted package path;
diagnostic ranges identify the individual spec. Existing grouped imports do
not become new findings merely because another entry is added to the group.

After the user authorizes a policy change and supplies the intended boundary,
the candidate configuration can contain:

```yaml
rules:
  import-boundary:
    enabled: true
    required: true
    severity: error
    parameters:
      forbidden_imports:
        rust: ['^use crate::interfaces::internal']
        typescript: ['^import .* from legacy-internal/']
```

The mapping permits supported language names or `all`; each value is a bounded
array of valid regular expressions. Enabling `import-boundary` without this
mapping is a configuration error rather than an empty successful check.

## Agent workflow

1. Read the relevant file under `references/rules/` and this guide.
2. Run `rules list --format json` using the selected bundled executable.
3. Explain the rule's snapshot evidence, default severity, and limits.
4. Ask for authorization before changing the candidate policy; do not select a
   severity or architecture pattern on the user's behalf.
5. Run the requested snapshot check and report warnings, violations, and
   incomplete evidence separately.
