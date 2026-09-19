# C and C++ analyzer reports

Qualitygate's existing SARIF 2.1.0 `ratchet` mode compares fresh diagnostics
from the same analyzer run on the base and current immutable source snapshots.
It counts each `(tool, rule)` bucket, retains both raw reports and executable
version probes, and fails when a bucket grows. Invalid source locations,
missing reports, malformed SARIF, failed analysis and unexpected exits are
incomplete execution. Read the full [SARIF report contract](sarif.md) before
adopting another producer.

The packaged [GCC reference policy](../skills/qualitygate-cli/references/gcc-analyzer-ratchet.yaml)
analyzes one tracked C translation unit with `-fanalyzer` and writes the
SARIF output at `sample.c.sarif`. Change the source path and report path
together. [GCC documents](https://gcc.gnu.org/onlinedocs/gcc/Diagnostic-Message-Formatting-Options.html)
that `-fdiagnostics-format=sarif-file` writes a SARIF file derived from the
source name. Check the actual filename when changing `-o`; the reference's
`sample.o` produces `sample.c.sarif`, while a different object stem can alter
it. The `-c` compilation step is intentional: GCC's analyzer can produce
different results with `-fsyntax-only`. For C++, use `g++`, the C++ path and
its matching SARIF path. Pin the compiler in the project toolchain and review
the `--version` probe. Keep output objects and reports untracked. For many
translation units,
configure separate checks or a bounded project-owned producer that merges
complete per-unit SARIF without dropping findings; validate the merged report
and preserve source paths.

[Clang documents](https://clang.llvm.org/docs/analyzer/user-docs/CommandLineUsage.html)
`clang --analyze --analyzer-output sarif -o report.sarif source.c` for a
single translation unit. Set `argv`, report `path` and `baseline` to the
actual command and output path, and include a `clang --version` tool probe.
The packaged [C++ Clang reference](../skills/qualitygate-cli/references/clang-static-analyzer-ratchet.yaml)
uses this form for `sample.cpp`. Its policy parser contract is tested; the
live C++ ratchet acceptance case uses GCC SARIF on both snapshots.
Other tools can be used when they emit complete, self-contained SARIF 2.1.0
within this [supported profile](sarif.md). A tool's XML or JSON file does not
become SARIF by renaming its extension. In particular, clang-tidy's ordinary
text/YAML output and cppcheck XML need a reviewed converter or a separate
validated adapter; neither is implicitly accepted by this policy.

`tests/c_family_ratchet.rs` runs real GCC C and C++ reports in isolated Git
repositories. It proves historical findings are retained, a new finding grows
the correct bucket, repair passes, and a source compile failure stays
incomplete. The CI `c-family-project` job executes the live test. The seven
optional [C/C++ text rules](rules.md) give fast changed-line review signals;
the analyzer supplies the semantic evidence needed for deeper issues.
