# Test effectiveness

Use an explicitly configured command or task verification with
`test_effectiveness`. The CLI runs current tests first, then the same command
on old code plus the selected current independent test/support files. Each
added or content-modified test file must contain a matching case that passes
currently and fails an assertion on the old code.

```yaml
id: behavior
argv: [project-test-runner, --junit, target/tests.xml]
findings_exit_codes: [1]
tools:
  - id: producer
    argv: [project-test-runner, --version]
reports:
  - path: target/tests.xml
    format: junit
test_effectiveness:
  source_paths: ['src/**']
  test_paths: ['tests/**']
  support_paths: ['test-data/**']
  assertion_failure_types: [AssertionError]
```

Adopt the actual command, selectors and exact failure types through the caller's
policy workflow. Report-producing wrappers need tool version and input identity
evidence. No guessed runner, title exemption, installed-dependency copying or
inline Rust test extraction is provided. Path groups accept at most 256 globs;
source/test groups and the bounded assertion-type list must be nonempty.

The report profile requires an explicit captured file and nonempty name on each
JUnit testcase, unique `(file, classname, name)` identities, consistent counters
and comparable selected case inventories. A `<failure>` requires an allowed
`type`; `<error>`, absent/unknown types, selected skips, missing cases, compiler
errors, absent reports, timeout and input mutation remain incomplete. The
producer owns these classifications. A weaker JUnit export cannot establish the
requested proof. Ordinary JUnit checks retain their existing meaning.

The old code is the invocation's resolved base. Production selectors cannot
overlap test/support files. The overlay retains additions, removals and file
modes and rejects replacement of protected policy/build inputs. New dependencies
that prevent baseline execution require a separate verification strategy, not a
successful fallback. Both runs share the configured deadline and preserve fresh
report, input guard and tool comparability evidence.

No production change is explicitly not applicable; an empty source scan is
incomplete. Production changes without changed independent tests fail. An
all-skipped plan retains the existing incomplete gate. Quick/path checks cannot
establish delivery readiness.

Inspect `metadata.test_effectiveness`, `current_test_execution`,
`baseline_test_execution` and `test_effectiveness_files`, plus retained artifact
digests and the diagnostic's pinned recheck. Report each file's actual proof;
one effective file cannot compensate for another. Do not broaden failure types,
replace the baseline, remove tests or lower policy requirements to pass.
