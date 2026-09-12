# Performance and efficiency evidence

Performance rules require a workload, a baseline, and a decision threshold.
They do not follow from source size, a code-style guide, or an unreviewed claim
that a change is faster.

## Lifecycle input

The performance-verification contract applies to declared capacity, latency,
throughput, resource-efficiency, or CUDA optimization claims. It requires a
comparable baseline, benchmark or load test, environment, threshold, and
profile or telemetry.

## Source synthesis

NVIDIA uses an Assess, Parallelize, Optimize, Deploy cycle and pairs
optimization with profiling and numerical verification.[1] AWS calls for KPIs,
load tests, monitoring, automation, and recurring review.[2] Azure requires
measurable goals, production-like conditions, baselines, budgets, and
hypothesis-driven experiments.[3]

## Critical limits

- Test conditions must state relevant hardware, configuration, data, and load.
- Numerical acceptance needs a reference comparison or adopted tolerance.
- Benchmark success does not prove authorization, memory safety, or reliability.
- Production testing needs gradual exposure, safeguards, capacity, and rollback.
- No performance artifact is a policy decision by itself; business objectives
  set thresholds and acceptable trade-offs.

## Sources

1. NVIDIA, [CUDA C++ Best Practices Guide](https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/).
2. AWS, [Performance efficiency](https://docs.aws.amazon.com/wellarchitected/latest/framework/a-performance-efficiency.html).
3. Microsoft Azure, [Architecture strategies for performance testing](https://learn.microsoft.com/en-us/azure/well-architected/performance-efficiency/performance-test).

[1]: https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/
[2]: https://docs.aws.amazon.com/wellarchitected/latest/framework/a-performance-efficiency.html
[3]: https://learn.microsoft.com/en-us/azure/well-architected/performance-efficiency/performance-test
