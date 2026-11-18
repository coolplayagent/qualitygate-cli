# Project deprecation policies

The policies on this page opt in to `reports[].mode: project`. A complete
finding in the configured project can block even when its source line was
not edited. Keep the actual compiler targets, dependency versions and analyzer
inputs in the selected snapshot. Choose only the checks for the languages and
targets the project uses, and merge them into the existing policy.

| Language | Policy and observed evidence |
| --- | --- |
| Java | The [JDK 8](java-deprecation-jdk8.yaml) and [JDK 17/21](java-deprecation.yaml) policies check javac warnings. [Forbidden annotations](java-forbidden-annotations.yaml) use the [PMD ruleset](java-forbidden-annotations.xml) and a shared source list for the compile and analysis commands. Set PMD's `--use-version` to the project's source level. [Detailed Java setup](diagnostic-ratchets.md#java-deprecation-and-forbidden-annotations). |
| Rust | [Clippy policy](rust-deprecation.yaml) consumes compiler `deprecated` diagnostics. The command forces that lint at warning level so the gate can report it, including when a local `allow(deprecated)` is present. Select the appropriate feature and target inventory. |
| C and C++ | Use the [C](c-deprecation.yaml) and [C++](cpp-deprecation.yaml) GCC SARIF examples for each compiled translation unit. Replace the file and add the production include paths, language standard and preprocessor settings. The policy blocks diagnostic findings reported by these focused invocations. Leave deprecation at warning level so compilation can complete and SARIF remains usable. |
| Go | [golangci-lint policy](go-deprecation.yaml) selects Staticcheck `SA1019`. Copy the [analyzer configuration](golangci-deprecation.yaml) to `config/golangci-deprecation.yaml`. Staticcheck resolves documented cross-package uses and deprecated imports under the selected Go version and build tags. Same-package symbol references are outside `SA1019` coverage. |
| TypeScript and JavaScript | [Typed ESLint policy](typescript-deprecation.yaml) checks resolved JSDoc `@deprecated` uses with `@typescript-eslint/no-deprecated`. Copy the [flat config](typescript-deprecation-eslint.config.mjs) to `eslint.config.mjs` and use or adapt the [type project](typescript-deprecation-tsconfig.json) as `tsconfig.json`. Keep both languages in the selected project. Import-only references and dynamic values can fall outside the rule. |
| Python | [Pyright policy](python-deprecation.yaml) checks typed PEP 702 uses. Copy the [checker configuration](python-deprecation-pyrightconfig.json) to `pyright-deprecation.json`, and set the project Python version and type roots. The sample targets Python 3.13 `warnings.deprecated`. Coverage requires visible deprecation metadata and static symbol resolution. |
| Bash | [ShellCheck policy](shell-deprecation.yaml) checks configured known legacy syntax with `SC2006` and `SC2007`, after a Bash syntax check. Set both commands to the same script. This policy covers those two forms in that script. |

Provision the configured tools before running Qualitygate. Ignored tool
installations inside the source checkout are absent from snapshot execution.
For the typed ESLint example, prepare the pinned packages in an external
directory and provide the executable and module root to the check process.
Use an absolute path controlled by the caller.

```bash
npm install --prefix "$QUALITYGATE_ESLINT_MODULE_ROOT" --no-audit --no-fund --ignore-scripts \
  eslint@10.10.0 typescript-eslint@8.73.0 typescript@6.0.3
export QUALITYGATE_ESLINT_MODULE_ROOT
export PATH="$QUALITYGATE_ESLINT_MODULE_ROOT/node_modules/.bin:$PATH"
```

The pinned Pyright example uses `pyright@1.1.414` on the command path. Treat the
provided type and build configurations as part of the evidence for the chosen
project. A tool failure or missing required report remains incomplete.
