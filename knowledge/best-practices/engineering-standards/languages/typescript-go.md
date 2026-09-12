# TypeScript and Go practice lane

## Source coverage

Google TypeScript requires tool-enforced type checking and warns against unsafe
assertions and dynamic code evaluation. Huawei CodeArts documents TypeScript
and Go rule-set categories. Cloudflare describes TypeScript lint support and
planned expansion across its common languages.[1] [2] [3]

## Current status

Qualitygate recognizes TypeScript and Go test structures for generic rules, but
does not ship a dedicated packaged static-analysis rule. The matrix keeps this
as planned. A language name or a vendor rule-set category is not enough to
claim a reliable type, lint, dependency, security, or performance result.

## Promotion requirements

An enforceable lane needs a configured tool invocation, pinned identity and
version, complete selected-snapshot scope, normalized report location,
suppression policy, and language-specific severity mapping. Until then, missing
or unverifiable reports stay outside an automated verdict.

## Sources

1. Google, [Google TypeScript Style Guide](https://google.github.io/styleguide/tsguide.html).
2. Huawei Cloud, [CodeArts Check Rule Set](https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html).
3. Cloudflare, [How Cloudflare enforces engineering standards using AI](https://blog.cloudflare.com/engineering-standards-enforcement/).

[1]: https://google.github.io/styleguide/tsguide.html
[2]: https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html
[3]: https://blog.cloudflare.com/engineering-standards-enforcement/
