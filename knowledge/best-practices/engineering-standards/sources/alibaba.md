# Alibaba

Sources: [Alibaba Java Coding Guidelines](https://github.com/alibaba/Alibaba-Java-Coding-Guidelines)
and [P3C](https://github.com/alibaba/p3c).

Authority: Alibaba's public engineering guidance for Java teams. It divides
recommendations into mandatory, recommended, and reference levels.

## Rule inputs

- Naming and comments provide evidence for `test-naming` and `comment-language`.
- Concurrency guidance calls for bounded thread pools, explicit lock ordering,
  cleanup of thread-local state, and attention to lock contention.
- Input size limits, authorization before accessing user-owned data, output
  escaping/desensitization, and anti-replay controls inform secure review rules.
- Library versioning, dependency declaration, API compatibility, and SQL/index
  performance inform `used-undeclared`, `module-boundary`, and future project
  adapters.

Qualitygate consumes the parts that have stable snapshot evidence. Security
authorization and database performance remain design or ecosystem checks until
the relevant semantic adapter is configured.

P3C is a concrete PMD and IDE implementation of a subset of the guidelines. It
supports a Java static-gate input without making its exact tool behavior a
universal qualitygate rule.
