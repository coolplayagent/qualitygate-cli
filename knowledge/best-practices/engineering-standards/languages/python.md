# Python practice lane

## Source coverage

Google Python guidance connects style with linting, imports, documentation, and
small exception boundaries. Huawei CodeArts exposes Python critical and general
rule-set categories. Meta's public security-analysis work demonstrates that
taint analysis must preserve its dependency and type information to be useful
in CI.[1] [2] [3]

## Current rule inputs

| Qualitygate rule | Evidence | Decision boundary |
| --- | --- | --- |
| pytest-naming | parsed added pytest tests | violation only when pytest discovery applies |
| test-naming | recognized test entities | generic pattern must not supersede pytest discovery |
| parameterized-tests | similar parsed test shapes | warning because structural similarity is not semantic equivalence |
| comment-language | changed parsed comments | warning with terminology exemptions |

## Security and static analysis

Exception handling, input validation, taint flows, dependency closure, and
framework behavior require semantic evidence. A Pysa-like report must identify
tool version, configuration, selected snapshot, dependency model, and
suppression rationale. A report that omits dependencies or source scope is
incomplete evidence, not a clean security result.

## Sources

1. Google, [Google Python Style Guide](https://google.github.io/styleguide/pyguide.html).
2. Huawei Cloud, [CodeArts Check Rule Set](https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html).
3. Meta, [Pysa GitHub Action](https://github.com/facebook/pysa-action) (archived; historical CI evidence only).

[1]: https://google.github.io/styleguide/pyguide.html
[2]: https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html
[3]: https://github.com/facebook/pysa-action
