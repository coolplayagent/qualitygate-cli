# NVIDIA

Source: [CUDA C++ Best Practices Guide](https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/)

Authority: NVIDIA's official CUDA performance guidance.

## Rule inputs

- Optimize from measured bottlenecks and use profiling to select work.
- Expose parallelism, map it to the hardware, and maximize useful concurrent
  host/device execution.
- Optimize memory access and instruction throughput, while checking numerical
  behavior and floating-point differences introduced by parallel execution.
- Validate the target architecture and compatibility assumptions when building
  for multiple compute capabilities.
- The APOD cycle explicitly links assess, parallelize, optimize, and deploy;
  numerical reference comparison or tolerated error is distinct from a speedup.

Qualitygate records these as `performance-verification` inputs: benchmark,
profile, target configuration, and regression threshold. A diff-size warning
cannot stand in for a GPU performance result, so no NVIDIA-specific finding is
reported without a configured benchmark or profiler report.
