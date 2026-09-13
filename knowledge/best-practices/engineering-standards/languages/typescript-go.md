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

Qualitygate recognizes TypeScript and Go test structures for generic rules, but
does not ship a dedicated packaged static-analysis rule. The matrix keeps this
as planned. A language name or a vendor rule-set category is not enough to
claim a reliable type, lint, dependency, security, or performance result.

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
