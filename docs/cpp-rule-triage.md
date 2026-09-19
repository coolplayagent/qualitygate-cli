# C++ rule triage

[Issue 19](https://github.com/coolplayagent/qualitygate-cli/issues/19)
collects C++ memory, exception, concurrency, arithmetic, preprocessor and
security checks. Qualitygate's source patterns inspect only changed text lines;
they do not build a C++ translation unit. The following nine rules have a
specific same-line review signal. They are opt-in, fixed to `lang-cpp`, and
their diagnostics do not assert CERT compliance or a confirmed vulnerability.

| Built-in | Review signal | Important limit |
|---|---|---|
| `cpp-no-realloc`, `cpp-no-alloca` | Direct allocator call | Wrappers, ownership and size are unknown. |
| `cpp-no-unsafe-memfunc` | `strcpy`, `strcat`, `sprintf`, `gets` call | Buffer bounds are unknown. |
| `cpp-throw-by-value` | `throw new` expression | Other pointer expressions and ownership are unknown. |
| `cpp-catch-by-reference` | Simple class-like value catch | Complex type names and intentional value catches need review. |
| `cpp-no-direct-mutex` | `.lock()` or `.unlock()` method call | Receiver type and surrounding RAII are unknown. |
| `cpp-no-std-move-local-return` | `return std::move(...)` | The object may be nonlocal; NRVO applicability is unknown. |
| `cpp-no-unsafe-rand` | `rand()` call | Security-sensitive use is unknown. |
| `cpp-no-throw-spec` | Simple same-line `throw()` specification | C++ standard mode and multiline signatures are unknown. |

The prior [C/C++ package](rules.md) already reviews lowercase `l` numeric
suffixes, assertion side effects, termination calls and some array
declarations. `no-unsafe-string` in the shared package overlaps selected C
string calls. Policies should select the rule with the intended severity and
scope; overlapping enabled rules can report the same line twice.

The remaining proposals require facts unavailable to a line matcher:

| Area | Examples needing C++ semantic analysis |
|---|---|
| Memory | `sizeof` on decayed parameters or pointers, uninitialized members, move-source state, null dereference, allocation size, array bounds, string capacity, new/delete pairing and `new` failure handling |
| Exceptions | Catch order, destructor throws, constructor function-try-block lifetime and propagation paths |
| Concurrency | Condition-variable loop predicates, shared signal-handler state and loop termination |
| Arithmetic | Signed overflow, unsigned wrap, division by zero, shift width, signed bitwise operations, mixed signedness and overflow before widening |
| Types and expressions | Virtual destructor need, default-argument overrides, polymorphic arrays, overloaded operators, moved const objects, nested name reuse and expression sequencing |
| Preprocessor | Macro expansion parentheses, keyword redefinition, directive placement, cross-file conditionals and include position within `extern "C"` |
| Security | Secret lifetime, address disclosure in release builds, SQL input flow, security function return handling and destination size |

Run a project-selected C++ analyzer in both immutable snapshots and ratchet
its complete SARIF report. The [Clang reference](../skills/qualitygate-cli/references/clang-static-analyzer-ratchet.yaml)
and [analyzer guide](c-family-ratchet.md) use that existing contract. Clang's
static analyzer does not claim to implement every proposal above; select and
validate analyzer checks according to the project's source and build flags.
[clang-tidy `--export-fixes`](https://clang.llvm.org/extra/clang-tidy/)
produces YAML, and [cppcheck `--xml`](https://cppcheck.sourceforge.io/manual.html)
needs a reviewed converter; labeling either file `checkstyle` or `sarif`
would be invalid.

The existing ratchet already reports `(tool, rule)` base/current counts and
growth in `<path>:ratchet` metadata. `ratchet init` and `ratchet tighten` are
omitted because an editable count file would bypass fresh analysis of the
immutable Git base. `ratchet status` and `ratchet diff` are available through
the normal JSON/Markdown check report and stored raw artifacts. Producer-side
exclusions must be reviewed as analyzer configuration in the selected policy
snapshot. Severity tiers and per-rule exemptions would change gate policy and
need an explicit versioned contract and independent falsification fixtures;
they are not inferred from SARIF result levels or suppressed findings.
