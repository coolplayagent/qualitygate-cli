# C++ and CUDA practice lane

## Source coverage

Google C++ emphasizes self-contained interfaces and direct dependencies. Meta
Velox emphasizes readable, low-coupling C++ APIs. NVIDIA provides a C++ CUDA
guide organized around Assess, Parallelize, Optimize, Deploy and explicitly
connects optimization to profiling and numerical verification.[1] [2] [3]

## Current status

Qualitygate records C++ and CUDA sources in the matrix, but does not ship a C++
or CUDA semantic adapter. Huawei's language-specific categories and Meta Infer
issue types are evidence that tool selection and report normalization must be
language-aware, not a substitute for an adapter.[4] [5]

## Evidence contract

A CUDA performance input needs the GPU model and toolkit context, workload,
baseline, threshold, profile, and numerical acceptance rule. A valid speedup
can still be a correctness regression if parallel evaluation changes floating
point behavior beyond an adopted tolerance.

C++ source hygiene should distinguish include structure, ownership, lifetime,
concurrency, memory safety, and public API coupling. A text heuristic cannot
prove any of these semantic properties.

## Sources

1. Google, [Google C++ Style Guide](https://google.github.io/styleguide/cppguide.html).
2. Meta, [Velox coding style](https://github.com/facebookincubator/velox/blob/main/CODING_STYLE.md).
3. NVIDIA, [CUDA C++ Best Practices Guide](https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/).
4. Huawei Cloud, [CodeArts Check Rule Set](https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html).
5. Meta, [Infer issue types](https://fbinfer.com/docs/all-issue-types/).

[1]: https://google.github.io/styleguide/cppguide.html
[2]: https://github.com/facebookincubator/velox/blob/main/CODING_STYLE.md
[3]: https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/
[4]: https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html
[5]: https://fbinfer.com/docs/all-issue-types/
