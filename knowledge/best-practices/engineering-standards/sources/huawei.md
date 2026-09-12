# Huawei

Source: [CodeArts Check preset rule sets](https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html)
and the [CodeArts Check user guide](https://support.huaweicloud.com/eu/usermanual-codecheck/codeartscheck-usermanual.pdf)

Authority: Huawei Cloud's public CodeArts Check documentation. It lists
language-specific coding-style and general criterion sets and supports custom
rule sets across mainstream languages.

## Rule inputs

- Rule-set selection is language-specific and should be explicit.
- The published language inventory covers Java, C++, Go, Python, TypeScript,
  JavaScript and other languages; a category does not prove a portable rule
  implementation or severity policy.
- Style, quality, and security checks are separate concerns that can be
  configured and reviewed as a gate.
- Custom rules and report results should retain their rule identity and scope.
- The documented metric thresholds and suppression counts are useful inputs
  for future report and complexity adapters.

Qualitygate uses this source for language routing, static-gate evidence, and
the rule catalog mapping. The public preset list does not expose every rule
implementation, so it is not treated as a complete copy of CodeArts rules.
