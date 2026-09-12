# Google

Sources: [C++ Style Guide](https://google.github.io/styleguide/cppguide.html),
[Java Style Guide](https://google.github.io/styleguide/javaguide.html),
[Python Style Guide](https://google.github.io/styleguide/pyguide.html),
[TypeScript Style Guide](https://google.github.io/styleguide/tsguide.html),
[The Standard of Code Review](https://google.github.io/eng-practices/review/reviewer/standard.html),
and the [Google SRE books](https://sre.google/books/).

Authority: Google style guides for Google-originated open-source projects.

## Rule inputs

- C++ headers should be self-contained and include what they use; small public
  definitions keep APIs readable and reduce avoidable build cost.
- Include ordering, naming, comments, and formatting are suitable for syntax
  or formatter backed checks.
- Python guidance requires linting, explicit exception handling, small
  `try`/`except` scopes, and no assertions as application precondition checks.
- Naming and test structure are useful inputs for language adapters; linter
  and formatter results are stronger evidence than a text-only heuristic.
- Java and TypeScript provide language-specific source and type-system inputs.
  The review guide sets a conflict rule: evidence and code health outweigh
  personal preference, while noncritical polish should not silently block work.
- The SRE books extend the archive from code review into reliability, capacity,
  monitoring, and secure operation across the software lifecycle.

Qualitygate maps the stable parts to line endings, comments, naming and future
include/dependency adapters. It does not claim that a generic regex proves C++
header self-containment or Python exception semantics.
