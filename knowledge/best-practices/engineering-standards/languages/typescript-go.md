# TypeScript and Go practice lane

## Source coverage

Alibaba's front-end guide supplies TypeScript and engineering-rule input via
its linter packages. Google TypeScript requires tool-enforced type checking and
warns against unsafe assertions and dynamic code evaluation. Huawei CodeArts
documents TypeScript and Go rule-set categories. Go's own review guidance makes
`gofmt` the mechanical baseline and treats contexts, cancellation, and
documentation as explicit language/API concerns. Cloudflare describes
TypeScript lint support and planned expansion across its common languages.[1]
[2] [3] [4] [5]

## Current status

Qualitygate recognizes TypeScript and Go test structures for generic rules.
The optional `lang-typescript` package now supplies twelve changed-line lexical
review signals, while `eslint_json` normalizes a fresh ESLint formatter report
for a paired diagnostic ratchet. The optional `lang-go` package adds twelve
changed-line Go signals and `golangci_json` normalizes the v2 producer for the
same paired ratchet. These patterns do not prove type, data-flow, dependency,
resource lifetime or runtime safety.

## Promotion requirements

An enforceable lane needs a configured tool invocation, pinned identity and
version, complete selected-snapshot scope, normalized report location,
suppression policy, and language-specific severity mapping. Until then, missing
or unverifiable reports stay outside an automated verdict. `gofmt` can
establish formatting only; it cannot prove context propagation, correct
cancellation, dependency risk, or security behavior.

## Sources

1. Alibaba, [Alibaba Front-end Coding Guidelines and Relevant Tools](https://github.com/alibaba/f2e-spec).
2. Google, [Google TypeScript Style Guide](https://google.github.io/styleguide/tsguide.html).
3. Huawei Cloud, [CodeArts Check Rule Set](https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html).
4. Go Project, [Go Code Review Comments](https://go.dev/wiki/CodeReviewComments).
5. Cloudflare, [How Cloudflare enforces engineering standards using AI](https://blog.cloudflare.com/engineering-standards-enforcement/).

[1]: https://github.com/alibaba/f2e-spec
[2]: https://google.github.io/styleguide/tsguide.html
[3]: https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html
[4]: https://go.dev/wiki/CodeReviewComments
[5]: https://blog.cloudflare.com/engineering-standards-enforcement/

For the implemented TypeScript lane, the [ESLint JSON formatter](https://eslint.org/docs/latest/use/formatters/)
and [exit-code contract](https://eslint.org/docs/latest/use/command-line-interface)
define report and command evidence. [ESLint flat config](https://eslint.org/docs/latest/use/configure/configuration-files)
owns lint selection. [MDN's Math.random](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Math/random)
and [postMessage](https://developer.mozilla.org/en-US/docs/Web/API/Window/postMessage)
references support optional review signals. The standards registry and
lifecycle matrix record the narrower scope and adoption limits of each rule.

For Go, the [golangci-lint v2 CLI](https://golangci-lint.run/docs/configuration/cli/#run)
defines JSON output and count-preserving flags. The [Go modules FAQ](https://go.dev/wiki/Modules),
[Go review comments](https://go.dev/wiki/CodeReviewComments), [cgo documentation](https://pkg.go.dev/cmd/cgo),
and [gosec rules](https://github.com/securego/gosec#available-rules) support
specific optional review signals. The registry and lifecycle matrix name the
provenance and lexical limits per rule.
