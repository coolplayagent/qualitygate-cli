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
| `no-printf-log` | C, C++ | error | `printf` or `fprintf` call patterns in added native files |
| `no-unsafe-string` | C, C++ | error | `strcpy`, `sprintf`, `strcat`, or `gets` call patterns in added native files |
| `no-test-sleep` | all languages | error | `sleep`, `usleep`, or `nanosleep` call patterns in added test paths |
| `no-bare-except` | Python | error | bare exception handler patterns in added files |
| `no-os-path` | Python | error | selected `os.path` calls in added files |
| `no-print` | Python | warning | `print` calls in added files |
| `no-emoji` | all languages | error | configured emoji Unicode ranges in added UTF-8 files |
| `python-test-naming` | Python | error | lower snake case added pytest test names |
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

The three Issue 10 rules are also opt-in. `no-printf-log` and
`no-unsafe-string` inspect added C/C++ files with a fixed `languages: [c, cpp]`
scope, including `.h` headers. `no-test-sleep` uses an empty language scope
and defaults to common test directory and filename globs; configure `paths`
for a repository's test layout. All three scan UTF-8 file lines with bounded
regular expressions. Matches in comments and strings can be false positives;
they do not establish a parsed call or prove that a replacement API is safe.
Files outside the selected paths, modified files, and nonmatching languages
are not inspected. Unreadable selected bytes or exhausted analysis budgets
make the check incomplete.

The six Issue 11 rules are opt-in: the four added-file rules above, the
`python-test-naming` syntax rule, and core `commit-message-format`.
The latter checks added commit subjects against a configurable subset of
Conventional Commits. `python-test-naming` sees `test`-prefixed Python
functions, including names like `test1`, then applies
the stricter default `test_` lower snake case pattern. Configure `paths` to
select pytest test files; syntax parsing alone cannot prove file collection.
`no-print` defaults
to warning; its finding does not block an otherwise complete gate.
The file rules scan UTF-8 text line by line and can match comments or
strings. `no-emoji` covers only the configured ranges in files, not commit
messages or every emoji sequence. PEP 8 allows narrow bare exception
handlers, and PEP 428 does not require every project to replace `os.path`.

## TypeScript and JavaScript review signals

Select `rulesets: [lang-typescript]` and the desired rule IDs. All twelve
rules are opt-in and fixed to `languages: [typescript]`, which includes
`.ts`, `.tsx`, `.js`, `.jsx`, `.mjs` and `.cjs`. They inspect changed UTF-8
lines with bounded `source-pattern` matching and accept `paths` and
`prohibited_patterns.typescript` overrides. A pattern can match a comment or
string and cannot establish data flow, receiver type or production reachability.
The two comment rules cover only same-line syntax; the contact marker rule
does not classify personal data. Use the [ESLint reference](eslint-ratchet.yaml)
for semantic lint results.

| Rule | Default | Review signal |
| --- | --- | --- |
| `ts-no-eval` | error | direct eval call |
| `ts-no-debugger` | error | standalone debugger statement |
| `ts-no-alert` | warning | browser dialog call |
| `ts-no-implied-eval` | error | string passed to timer callback |
| `ts-no-new-function` | error | dynamic Function constructor |
| `ts-eqeqeq` | warning | loose equality operator |
| `ts-no-extend-native` | error | direct native prototype assignment |
| `ts-no-prototype-builtins` | warning | direct prototype method call |
| `ts-secure-randomness` | warning | Math.random call for review |
| `ts-no-unsafe-postmessage` | warning | postMessage call for origin review |
| `ts-no-commented-code` | warning | code-like line comment |
| `ts-no-personal-info-in-comments` | warning | contact marker in same-line comment |

## Go review signals

Select `rulesets: [lang-go]` and the desired rule IDs. All twelve rules are
opt-in, fixed to `languages: [go]`, and inspect changed UTF-8 `.go` lines with
bounded `source-pattern` matching. They accept `paths` and
`prohibited_patterns.go` overrides. Comments and strings can match. SQL
formatting does not establish taint, `math/rand` may serve nonsecurity uses,
the panic rule does not determine function visibility, and the cgo rule does
not prove a missing free. Use the [golangci-lint reference](golangci-lint-ratchet.yaml)
for semantic lint results.

| Rule | Default | Review signal |
| --- | --- | --- |
| `go-sql-injection` | error | SQL keyword in same-line fmt.Sprintf or string concatenation |
| `go-insecure-randomness` | warning | math/rand import or common call |
| `go-tls-insecure-skip-verify` | error | literal InsecureSkipVerify true |
| `go-hardcoded-credentials` | error | literal credential assignment |
| `go-ssh-insecure-ignore-host-key` | error | InsecureIgnoreHostKey call |
| `go-file-permission-creation` | warning | literal 0666 or 0777 mode in file write call |
| `go-panic-in-exported-function` | warning | panic call for visibility review |
| `go-cgo-cstring-without-defer-free` | warning | C.CString or C.CBytes allocation call |
| `go-relative-import-path` | warning | relative import path |
| `go-dot-import` | warning | dot import outside reviewed exceptions |
| `go-sensitive-info-in-log` | warning | sensitive variable name in log call |
| `go-float-loop-counter` | warning | floating literal for-loop initializer |

## C and C++ review signals

Select `rulesets: [lang-c]` and the desired rule IDs. These seven rules are
opt-in and fixed to `languages: [c, cpp]`. They match changed UTF-8 source
lines with bounded regular expressions. `paths`, `prohibited_patterns.c`,
and `prohibited_patterns.cpp` may be configured, but both language keys must
remain present. `.h` is classified as C text; `.hh`, `.hpp` and `.hxx` as C++
text. No C/C++ syntax parser is implied. These signals can match comments or
strings and cannot establish type safety, data flow or CERT compliance. Use
the [GCC SARIF ratchet reference](gcc-analyzer-ratchet.yaml) for analyzer
evidence. The repository's `docs/c-family-ratchet.md` explains adoption.

| Rule | Default | Review signal |
| --- | --- | --- |
| `c-array-safety` | error | variable-bound array declaration |
| `c-assertion-discipline` | warning | increment or decrement inside `assert` |
| `c-control-flow` | warning | empty `for` condition or floating loop declaration |
| `c-expression-safety` | warning | increment or decrement inside `sizeof` |
| `c-file-security` | error | `mktemp` or `tmpnam` call |
| `c-function-safety` | warning | `abort`, `exit`, `realloc`, `alloca`, `pthread_exit` or `ExitThread` call |
| `c-numeric-literal` | error | lowercase `l` integer suffix |

## Python review signals

Select `rulesets: [lang-python]` and the desired rule IDs. All thirteen rules
are opt-in, fixed to `languages: [python]`, and inspect changed UTF-8 `.py`
lines with bounded `source-pattern` matching. They accept `paths` and
`prohibited_patterns.python` overrides. Calls and names are review signals:
they do not prove tainted input, credential validity, production use, safe YAML
Loader selection or sensitive log content. Use the [Ruff reference](ruff-ratchet.yaml)
for semantic lint results.

| Rule | Default | Review signal |
| --- | --- | --- |
| `py-eval-exec` | error | direct eval or exec call |
| `py-shell-equals-true` | error | same-line subprocess call with shell=True |
| `py-insecure-randomness` | warning | common random module call |
| `py-tls-verify-disabled` | error | literal verify=False assignment |
| `py-yaml-unsafe-load` | error | yaml.load call for Loader review |
| `py-sql-string-format` | error | same-line formatted execute argument |
| `py-hardcoded-credentials` | error | credential-like name assigned a literal |
| `py-tempfile-mktemp` | error | tempfile.mktemp call |
| `py-bare-except` | warning | bare except clause |
| `py-mutable-default-argument` | warning | list, dict or set default in same-line function |
| `py-assert-in-production` | warning | assert statement for context review |
| `py-sensitive-info-in-log` | warning | credential-like name in logging call |
| `py-pickle-load` | error | pickle, shelve or marshal load call |

## Rust review signals

All three Rust rules are opt-in source patterns with fixed `languages: [rust]`.
They inspect changed UTF-8 lines and allow `paths` and
`prohibited_patterns.rust` overrides. They do not establish FFI safety,
attacker-controlled paths or macro expansion safety. The macro rule covers
same-line definitions only; comments and strings may match. Use the
[Clippy ratchet reference](clippy-ratchet.yaml) for semantic lint analysis.

| Rule | Default | Review signal |
| --- | --- | --- |
| `rust-extern-without-abi` | warning | extern block opening without an ABI string |
| `rust-untrusted-dynamic-library-loading` | error | nonliteral dynamic library path argument |
| `rust-unsafe-block-in-macro-definition` | warning | unsafe block in a same-line macro definition |

## Shell conventions

All eighteen Shell rules are opt-in and use a fixed `shell` scope. Source
patterns inspect changed lines in `.sh`, `.bash`, and extensionless scripts
with a recognized `sh` or `bash` shebang. Each accepts `paths`; the sixteen
source-pattern rules accept `prohibited_patterns.shell` overrides. They are
lexical review signals, not proof of an exploit or broken execution. Comments,
strings, valid uses of `exec`, checksums, signal numbers, and continued pipelines
can match. The password-read pattern covers simple forms, not every flag
combination. The stream-merge pattern targets a following conflicting
redirection. `shell-missing-shebang` checks added scripts and changed first
lines; a file with no extension or shebang cannot be identified as Shell.
`shell-commented-dead-code` requires three consecutive code-like comments
with at least one added line. Selected invalid UTF-8 and exceeded budgets
remain incomplete.

| Rule | Default | Review signal |
| --- | --- | --- |
| `shell-hardcoded-secret` | error | Flag quoted credential assignments |
| `shell-debug-mode` | warning | Review xtrace and verbose shell execution |
| `shell-env-dump` | warning | Review environment dump commands |
| `shell-weak-crypto` | warning | Review legacy algorithm tokens in shell scripts |
| `shell-password-echo` | warning | Review password reads without silent mode |
| `shell-sql-injection` | error | Flag database client commands with interpolated variables |
| `shell-exec-terminates` | warning | Review exec replacing the current shell |
| `shell-assignment-spaces` | error | Flag spaces adjacent to Shell assignment operators |
| `shell-comparison-spaces` | error | Flag comparison delimiters missing inner spaces |
| `shell-line-start-operator` | warning | Review leading pipe or logical operators |
| `shell-stream-merge-position` | warning | Review stream merges followed by conflicting redirections |
| `shell-trap-uncapturable` | error | Flag traps for SIGKILL or SIGSTOP |
| `shell-trap-numeric-signal` | warning | Review numeric trap signal identifiers |
| `shell-trap-double-quotes` | warning | Review double-quoted trap handlers |
| `shell-tilde-path` | warning | Review tilde path components |
| `shell-temp-file-hardcoded` | warning | Review predictable temporary paths |
| `shell-missing-shebang` | error | first-line interpreter directive on added Shell scripts |
| `shell-commented-dead-code` | warning | three or more consecutive code-like comment lines |

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
