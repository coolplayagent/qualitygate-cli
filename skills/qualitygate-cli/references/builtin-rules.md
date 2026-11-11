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
and `--source all` for both. An empty definition language scope adds no language
filter; each implementation still determines supported files and capabilities.
Built-in inventory queries need no initialized policy and include all shipped
packages, even unselected ones; listing does not enable enforcement. When IDs
overlap, both origins remain visible and only the selected one can be enabled.

Listing a rule is read-only. Adding or enabling rules changes candidate policy
and must be covered by the task's authorization; do not ask again when it is
already provided. Ordinary initialization follows the automatic missing-config
prerequisite in [repository verification](verification.md); it does not authorize
`--with-checks`, rule enabling, or policy adoption. A rule's archived
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
language-keyed regex arrays. `paths` can narrow files; `languages` must remain a
nonempty subset of the declared languages. An omitted override inherits the
scope; `languages: []` is rejected. Consult `rules describe security-sensitive-api`
for the runtime parameter contract before editing.

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

Select `rulesets: [lang-typescript]` and the desired rule IDs. All fifteen
rules are opt-in and fixed to `languages: [typescript]`, which includes
`.ts`, `.tsx`, `.js`, `.jsx`, `.mjs` and `.cjs`. The original twelve inspect changed UTF-8
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
| `ts-no-with` | warning | with statement |
| `ts-no-var` | warning | var declaration |
| `ts-no-arguments` | warning | arguments object reference |

## Go review signals

Select `rulesets: [lang-go]` and the desired rule IDs. All fifteen rules are
opt-in and fixed to `languages: [go]`. The original twelve inspect changed UTF-8 `.go` lines with
bounded `source-pattern` matching. Those rules accept `paths` and
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
| `go-unsafe-package` | warning | unsafe package usage |
| `go-os-exit` | warning | os.Exit or runtime.Goexit call |
| `go-import-alias` | warning | aliased import |

## C and C++ review signals

Select `rulesets: [lang-c]` and individual rule IDs. The seven original rules
use changed-line `source-pattern` matching with `paths` and
`prohibited_patterns.c` / `.cpp` overrides. These regex signals may match
comments or strings. The three new rules (`c-no-atoi`, `c-no-goto`,
`c-no-process-launch`) instead use bounded `native-syntax` parsing and accept
only `paths` and their fixed `languages: [c, cpp]` scope. `.c` uses the C grammar; `.h`, `.cc`, `.cpp`, `.cxx`, `.hh`, `.hpp`, `.hxx`
use the C++ grammar (which also recognizes C header syntax). This avoids
rejecting C++ declarations in ambiguously named `.h` files.
They ignore comments and literals, and inspect syntax intersecting changed
lines. Direct API calls accept unqualified, global and `std::` names, but
exclude member calls and other namespaces. Unqualified shadowing cannot be
resolved. None of these rules establishes type safety, data flow or CERT
compliance. Use the [GCC SARIF ratchet reference](gcc-analyzer-ratchet.yaml)
for semantic analyzer evidence.
The internal clauses come from the C V5.3 and C++ V5.1 language specifications;
the older C&C++ secure coding V3.0 document is explicitly superseded
and is not the current source.

| Rule | Default | Review signal |
| --- | --- | --- |
| `c-array-safety` | error | variable-bound array declaration |
| `c-assertion-discipline` | warning | increment or decrement inside `assert` |
| `c-control-flow` | warning | empty `for` condition or floating loop declaration |
| `c-expression-safety` | warning | increment or decrement inside `sizeof` |
| `c-file-security` | error | `mktemp` or `tmpnam` call |
| `c-function-safety` | warning | `abort`, `exit`, `realloc`, `alloca`, `pthread_exit` or `ExitThread` call |
| `c-numeric-literal` | error | lowercase `l` integer suffix |
| `c-no-atoi` | warning | `atoi`, `atol`, `atoll` or `atof` call |
| `c-no-goto` | warning | `goto` statement |
| `c-no-process-launch` | warning | `system`, `popen`, `exec*`, `WinExec`, `CreateProcess` or `ShellExecute` call |

## C++ review signals

Select `rulesets: [lang-cpp]` and individual rule IDs. These rules
are opt-in with fixed `languages: [cpp]`; `cpp-disabled-test` is described under
[historical review signals](#historical-review-signals). The original nine use
same-line `source-pattern` checks and accept `paths` / `prohibited_patterns.cpp`.
The seven new rules from `cpp-no-reinterpret-cast` through
`cpp-no-using-namespace-in-header` use `native-syntax`, accept only `paths`
and the fixed language scope, and also parse `.h` as C++.

Syntax rules skip comments, ordinary/raw strings and character literals.
They handle multiline syntax and report nodes intersecting diff-added lines;
unchanged syntax is not reported. The header rule covers changed `.h`, `.hh`,
`.hpp`, `.hxx` files, including existing headers. Smart-pointer checks recognize
explicit `std::unique_ptr` / `std::shared_ptr` construction with a direct `new`
argument, using parentheses or braces. Numeric macros require an object-like
macro whose replacement is a single numeric literal (optionally signed or
parenthesized), including trailing comments; expression/function macros are
outside that rule's scope.

Parsing is limited to 4 MiB, 200,000 syntax nodes and two seconds per file,
within the shared 30-second rule budget and diagnostic output budget.
Invalid UTF-8, syntax errors, missing snapshot files or exhausted budgets
produce incomplete execution, never a successful empty result. Macro expansion,
conditional-compilation evaluation, typedefs, namespace aliases, shadowing and
symbol/type resolution are not performed; syntax in conditional branches can
be reported regardless of build configuration. Warning findings require review,
not automatic replacement. Use the [Clang SARIF reference](clang-static-analyzer-ratchet.yaml)
or [GCC reference](gcc-analyzer-ratchet.yaml) for semantic analyzer evidence.

| Rule | Default | Review signal |
| --- | --- | --- |
| `cpp-no-realloc` | warning | direct `realloc` call |
| `cpp-no-alloca` | warning | direct `alloca` call |
| `cpp-no-unsafe-memfunc` | error | unbounded C string call |
| `cpp-throw-by-value` | warning | `throw new` expression |
| `cpp-catch-by-reference` | warning | simple class-like value catch |
| `cpp-no-direct-mutex` | warning | direct `.lock()` or `.unlock()` call |
| `cpp-no-std-move-local-return` | warning | `return std::move(...)` expression |
| `cpp-no-unsafe-rand` | warning | `rand()` call |
| `cpp-no-throw-spec` | warning | simple empty `throw()` specification |
| `cpp-no-reinterpret-cast` | warning | `reinterpret_cast` expression |
| `cpp-no-const-cast` | warning | `const_cast` expression |
| `cpp-no-delete-this` | warning | `delete this` expression |
| `cpp-prefer-make-smart-pointer` | warning | `unique_ptr` or `shared_ptr` constructed from `new` |
| `cpp-use-nullptr` | warning | `NULL` token in C++ code |
| `cpp-no-macro-constant` | warning | numeric `#define` constant |
| `cpp-no-using-namespace-in-header` | warning | `using namespace` in a changed header |

## Python review signals

Select `rulesets: [lang-python]` and the desired rule IDs. All sixteen rules
are opt-in and fixed to `languages: [python]`. Twelve inspect changed UTF-8 `.py`
lines with bounded `source-pattern` matching. Those rules accept `paths` and
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
| `py-sql-string-format` | error | formatted first argument of an execute/executemany call |
| `py-hardcoded-credentials` | error | credential-like name assigned a literal |
| `py-tempfile-mktemp` | error | tempfile.mktemp call |
| `py-bare-except` | warning | bare except clause |
| `py-mutable-default-argument` | warning | list, dict or set default in same-line function |
| `py-assert-in-production` | warning | assert statement for context review |
| `py-sensitive-info-in-log` | warning | credential-like name in logging call |
| `py-pickle-load` | error | pickle, shelve or marshal load call |
| `py-none-comparison` | warning | equality comparison against None |
| `py-walrus-operator` | warning | assignment expression |
| `py-lambda-assignment` | warning | lambda assigned to a name |

## Structured language reviews

The sixteen rules `py-none-comparison`, `py-walrus-operator`,
`py-lambda-assignment`, `go-unsafe-package`, `go-os-exit`, `go-import-alias`,
`ts-no-with`, `ts-no-var`, `ts-no-arguments`, `shell-eval`, `shell-alias` and
`shell-chmod-read`, plus `py-sql-string-format`, `shell-comparison-spaces` and
`shell-trap-uncapturable` and `shell-assignment-spaces`, use `language-syntax`.
They accept `paths` and their fixed
`languages` only; regex overrides are rejected. Tree-sitter parses immutable
selected source, including multiline syntax. Comments and literal text do not
trigger findings, while expressions inside interpolated strings and command
substitutions are inspected. Findings require changed syntax tokens.

Python checks adjacent equality operands against `None`, assignment expressions,
and named lambda assignments. Go resolves actual standard-library import paths,
including grouped, named and dot imports, with file-local lexical shadowing.
Changed imports also review retained calls; changed unsafe imports are reported
once. Named import aliases exclude blank and dot imports. TypeScript checks
`with`, `var` and implicit function `arguments`, including inherited arrow use;
properties, explicit bindings and type-only parameters are excluded.
Shell checks actual commands with static quoted names and `command`/`builtin`
wrappers. Alias queries are excluded. `chmod` examines its symbolic mode operand
for removal of read permission from all classes, not filenames or final effective
permissions; recursion is not required. Dynamic command names/modes, runtime
aliases, cross-file type resolution and ambiguous Go keyed-field names are outside
this analysis. Their packaged severities apply; findings do not establish safety.

`py-sql-string-format` inspects the first positional argument of calls named
`execute` or `executemany`, including attribute calls. It recognizes f-string
interpolation, a direct `.format(...)` call, or a Python `%` operation, including
multiline syntax. Literal DB-API placeholders such as `%s` and `%(name)s` with
separately bound parameters do not match. It does not resolve database receiver
types, query variables, keyword query arguments or taint; an f-string without
interpolation does not produce a finding.

`shell-comparison-spaces` checks the delimiter boundaries of Bash-parsed test
commands. Array subscripts, glob arguments, comments and quoted examples do not
match. Only this rule permits a specific parser recovery: a missing `]]` node
whose two closing characters already exist as unquoted words attached to the
preceding operand. Every other parser error or missing node, including errors
elsewhere in the same file, remains incomplete. This recovery reports the
missing spaces in `[[$a == $b]]` without treating arbitrary broken syntax as a
complete analysis.

`shell-trap-uncapturable` checks static signal operands after a static trap
action. `KILL` and `STOP` are case-insensitive and permit the `SIG` prefix;
numeric `9` is also recognized. Signal numbers such as `19` are platform-specific
and are not assumed to mean STOP. Action text, query forms (`-p`, `-P`, `-l`) and
reset forms do not match. Dynamic actions/signals are unresolved, and the rule
does not resolve shell functions shadowing builtins. The same static quoted
command names and `command`/`builtin` wrappers apply.

Parsing is bounded to 4 MiB per file, 200,000 traversal/binding work steps and a
2-second per-file deadline, within shared rule time and diagnostic budgets.
Invalid UTF-8, malformed syntax outside the specific comparison recovery, and
exhausted budgets produce incomplete execution. Version 1 frozen definitions of
the four migrated rules retain their original `source-pattern` implementation
and regex parameters; version 2 definitions reject regex overrides.

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

All twenty-one Shell rules are opt-in and use a fixed `shell` scope. Source
patterns inspect changed lines in `.sh`, `.bash`, and extensionless scripts
with a recognized `sh` or `bash` shebang. Each accepts `paths`; the thirteen
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

`shell-assignment-spaces` version 2 reviews actual commands whose bare identifier
name is followed by an unquoted argument beginning with `=` or `+=`. It also
covers inline commands and static `command`/`builtin` wrappers. Empty assignments
and command prefixes such as `IFS= read -r line` are valid. `VALUE= command`
cannot be proven wrong from whitespace alone and does not match. Neither do
test operands, comments, heredoc text or quoted examples, including multiline
text. Declaration commands such as `local VALUE = bad` remain outside this rule.
This is an assignment-like command heuristic: commands such as `echo = bad`
can match legitimately because runtime command intent is unresolved. Review the
context before changing code. Frozen version 1 definitions retain their old
line pattern.

| Rule | Default | Review signal |
| --- | --- | --- |
| `shell-hardcoded-secret` | error | Flag quoted credential assignments |
| `shell-debug-mode` | warning | Review xtrace and verbose shell execution |
| `shell-env-dump` | warning | Review environment dump commands |
| `shell-weak-crypto` | warning | Review legacy algorithm tokens in shell scripts |
| `shell-password-echo` | warning | Review password reads without silent mode |
| `shell-sql-injection` | error | Flag database client commands with interpolated variables |
| `shell-exec-terminates` | warning | Review exec replacing the current shell |
| `shell-assignment-spaces` | error | Review assignment-like command names followed by =/+= arguments |
| `shell-comparison-spaces` | error | Flag comparison delimiters missing inner spaces |
| `shell-line-start-operator` | warning | Review leading pipe or logical operators |
| `shell-stream-merge-position` | warning | Review stream merges followed by conflicting redirections |
| `shell-trap-uncapturable` | error | Flag static trap signal operands naming KILL/STOP or 9 |
| `shell-trap-numeric-signal` | warning | Review numeric trap signal identifiers |
| `shell-trap-double-quotes` | warning | Review double-quoted trap handlers |
| `shell-tilde-path` | warning | Review tilde path components |
| `shell-temp-file-hardcoded` | warning | Review predictable temporary paths |
| `shell-missing-shebang` | error | first-line interpreter directive on added Shell scripts |
| `shell-commented-dead-code` | warning | three or more consecutive code-like comment lines |
| `shell-eval` | warning | eval usage in a script |
| `shell-alias` | warning | alias definition in a script |
| `shell-chmod-read` | warning | chmod removing read permission |

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
4. Confirm that the task covers the candidate-policy change; use existing
   authorization and ask only for a missing material choice or required approval.
   Do not invent a severity or architecture boundary.
5. Follow [repository verification](verification.md) and report warnings,
   violations, and incomplete evidence separately.

The optional `lang-java` package adds twelve fixed-Java source-pattern reviews for direct concurrency, resource, locale, exception and security signals. See the repository Java rule triage guide for exact scope; comments, strings, aliases and multiline forms limit lexical findings.

## Historical review signals

These three rules require explicit selection and default to `required: false`,
`severity: warning`. Each inspects added lines of supported source files only.

| Rule | Scope | Default signal |
| --- | --- | --- |
| `cpp-disabled-test` | C++ | `DISABLED_` marker; review reason, tracking issue and replacement coverage |
| `c-ignored-atomic-cas` | C and C++ | `(void)` discard of `atomic_compare_exchange_weak/strong`, including `_explicit`; review failure handling |
| `workaround-marker` | Supported source languages | `workaround`, `temporary fix`, `暂时规避`, `临时方案`, `临时绕过`; review root-cause issue, owner/expiry and removal plan |

Matches are review signals, not proof of semantic defects or missing metadata.
Adding a reason, issue or replacement-test reference does not suppress a marker.
Restoring the test, handling the result or removing the temporary implementation
can remove its signal; deleting explanatory comments alone does not prove repair.
Comments/literals can match, multiline/indirect calls can escape detection, and
`.h` is classified as C (outside `cpp-disabled-test`). Markdown is not scanned.

Use the [review cards](review-pack.md) for contextual review and
[design review](design-review.md) for Clean Code, Clean Architecture, DRY and SOLID
tradeoffs. Literal diagnostic message/fix metadata belongs to the built-in YAML;
it provides guidance without proving the concern or changing the verdict.
Existing parameter configuration can select an ecosystem-specific CAS API; this override replaces
the default patterns, so retain standard patterns explicitly if both are needed:

```yaml
schema_version: 1
rulesets: [lang-c]
rules:
  c-ignored-atomic-cas:
    enabled: true
    required: false
    severity: warning
    parameters:
      prohibited_patterns:
        c: ['\(\s*void\s*\)\s*ngx_atomic_cmp_set\s*\(']
        cpp: ['\(\s*void\s*\)\s*ngx_atomic_cmp_set\s*\(']
```

This configured symbol does not imply cross-language or general CAS coverage.
C++ gtest test-entity extraction and naming remain deferred; a name regex can
express `_when_`, but cannot supply the missing entity producer.
