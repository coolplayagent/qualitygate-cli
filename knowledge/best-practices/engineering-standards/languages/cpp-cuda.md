# C, C++ and CUDA practice lane

## Source coverage

The internal C V5.3 and C++ V5.1 language specifications are the current
company standards for native code. They consolidate style, secure coding and
Clean Code expectations and explicitly supersede the older C&C++ secure coding
V3.0 document. Google C++ emphasizes self-contained interfaces and direct
dependencies. Meta Velox emphasizes readable, low-coupling C++ APIs. NVIDIA
provides a C++ CUDA guide organized around Assess, Parallelize, Optimize,
Deploy and explicitly connects optimization to profiling and numerical
verification.[1] [2] [3] [4] [5]

## Current status

Qualitygate records C, C++ and CUDA sources in the matrix, but does not ship a
C or C++ semantic adapter. Internal language categories and Meta
Infer issue types are evidence that tool selection and report normalization
must be language-aware, not a substitute for an adapter.[1] [2] [6]

The built-in C and C++ packages contain bounded line-pattern review signals.
They cannot evaluate type safety, ownership, lifetime, exception safety,
templates, include order, switch exhaustiveness or data flow. Those clauses
remain review guidance or require compiler/analyzer evidence.

## Evidence contract

A CUDA performance input needs the GPU model and toolkit context, workload,
baseline, threshold, profile, and numerical acceptance rule. A valid speedup
can still be a correctness regression if parallel evaluation changes floating
point behavior beyond an adopted tolerance.

C and C++ source hygiene should distinguish include structure, ownership,
lifetime, concurrency, memory safety, integer behavior, exception behavior and
public API coupling. A text heuristic cannot prove any of these semantic
properties.

## Sources

1. Internal C Language Coding Specification V5.3, DKBA 12271-2023.12 (internal standard).
2. Internal C++ Language Coding Specification V5.1, DKBA 12977-2022.12 (internal standard).
3. Google, [Google C++ Style Guide](https://google.github.io/styleguide/cppguide.html).
4. Meta, [Velox coding style](https://github.com/facebookincubator/velox/blob/main/CODING_STYLE.md).
5. NVIDIA, [CUDA C++ Best Practices Guide](https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/).
6. Meta, [Infer issue types](https://fbinfer.com/docs/all-issue-types/).

[1]: https://example.invalid/standards/c-v5.3
[2]: https://example.invalid/standards/cpp-v5.1
[3]: https://google.github.io/styleguide/cppguide.html
[4]: https://github.com/facebookincubator/velox/blob/main/CODING_STYLE.md
[5]: https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/
[6]: https://fbinfer.com/docs/all-issue-types/
