# Java rule triage

[Issue 20](https://github.com/coolplayagent/qualitygate-cli/issues/20) proposes many Java, cross-language and analyzer rules. The twelve packaged `lang-java` rules are optional reviews of changed `.java` lines. Each matches one bounded text pattern. A match is a prompt for review, not proof of a vulnerability, API misuse or whole-program property. Enable selected rules with `rulesets: [lang-java]` and a `rules` entry; `languages: [java]` cannot be widened.

| Rule | Direct signal | Scope limit |
|---|---|---|
| `java-thread-stop`, `java-thread-yield` | Named `.stop()` receiver or `Thread.yield()` | Aliases and scheduling dependence are unknown. |
| `java-manual-gc`, `java-finalizer` | Explicit GC or `finalize` declaration | Intent and runtime behavior are unknown. |
| `java-implicit-charset`, `java-implicit-locale` | Zero-argument conversion | The value's use and locale/encoding requirements are unknown. |
| `java-insecure-random`, `java-weak-crypto` | `new Random` or named weak algorithm | Security-sensitive use and cryptographic policy are unknown. |
| `java-empty-catch`, `java-finally-exit` | Same-line empty catch or abrupt finally exit | Multiline blocks and control flow are not parsed. |
| `java-sql-concat`, `java-runtime-exec` | SQL call containing `+` or direct Runtime.exec | Input taint, command parsing and sanitization are unknown. |

Comments and string literals can match; aliases, imported static methods and multiline constructs can escape a same-line pattern. The existing rule scanner reports malformed selected text and budget exhaustion as incomplete. Project owners can tune `paths` and `prohibited_patterns.java` while retaining the fixed language scope. Use a pinned semantic analyzer for broader coverage.

The other proposed checks need additional facts and must not be represented as reliable text rules:

| Area | Why an analyzer or project policy is needed |
|---|---|
| Concurrency and collections | Lock identity, release on all paths, double-checked locking, ThreadLocal cleanup and mutation during iteration require control flow or type facts. |
| Control flow and arithmetic | Termination, switch completeness, numeric types, NaN and divide-by-zero depend on parsed expressions and paths. |
| Types and API contracts | Mutable public constants, `equals`/`hashCode`, constructor dispatch, logging ownership and exception layering require symbols or class hierarchies. |
| Resources and serialization | Try-with-resources, file cleanup, process I/O, object graphs and deserialization safety require lifecycle or data flow. |
| Security and IO | SQL/XML/command injection, XXE, reflection, log forging, TLS validation, path canonicalization and Zip Slip require source-to-sink flow and project context. |

The [Checkstyle](../skills/qualitygate-cli/references/checkstyle-ratchet.yaml), [PMD](../skills/qualitygate-cli/references/pmd-ratchet.yaml), and [SpotBugs](../skills/qualitygate-cli/references/spotbugs-ratchet.yaml) references use the existing `ratchet` mode with the producer's real XML format. They are starting policies requiring a pinned producer, tracked analyzer configuration, explicit source or class-file scope, and review of producer exit codes. SpotBugs needs compiled classes for **both** immutable snapshots; a missing build is incomplete. Checkstyle and PMD are source analyzers and have different configuration formats. None of the three automatically implements every proposed check.

Use `format: sarif` only for actual SARIF output. For example, Semgrep's `--json` emits JSON in its own format, so it cannot be labelled SARIF; use its supported SARIF output option with a reviewed producer policy. Ratchet buckets already track `(tool, rule)` baseline/current counts and growth; `<path>:ratchet` metadata and retained raw reports show the trend. There is no editable debt baseline or auto-lock operation: the trusted base is freshly analyzed from the immutable Git comparison. A `(tool, rule, category)` bucket needs stable producer category identity and a separately specified policy contract; adding category labels to a finding would silently change existing ratchets. Enterprise-specific geography, CA, framework and numeric threshold policies remain project-local.

Producer CLI references: [Checkstyle command line](https://checkstyle.org/cmdline.html), [PMD CLI and exit statuses](https://pmd.github.io/pmd/pmd_userdocs_cli_reference.html), [SpotBugs invocation](https://spotbugs.readthedocs.io/en/stable/running.html).
