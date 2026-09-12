# Meta

Sources: [Meta's Zoncolan static analysis](https://engineering.fb.com/2019/08/15/security/zoncolan/),
[Infer issue catalog](https://fbinfer.com/docs/all-issue-types/),
and [Velox coding style](https://github.com/facebookincubator/velox/blob/main/CODING_STYLE.md)

Authority: Meta engineering publications and a Meta-originated project guide.

## Rule inputs

- Codify repeatable security and privacy findings so analysis can run at code
  change volume, while acknowledging that static analysis cannot find every
  issue.
- Static analyzers can cover nullability, resource leaks, deadlocks, thread
  safety, taint, integer overflow, and unnecessary copies when their semantic
  inputs are available.
- Velox uses clang-format, descriptive names, reader-focused comments, small
  public APIs, explicit namespaces, and low coupling.
- Zoncolan validates a candidate rule's initial findings and chooses static
  analysis only for issue classes it can detect; report provenance and
  suppression rationale are therefore mandatory gate evidence.

Qualitygate maps these ideas to static report normalization, architecture and
reviewability inputs. A report with missing scope, version, or code snapshot is
incomplete rather than a pass.
