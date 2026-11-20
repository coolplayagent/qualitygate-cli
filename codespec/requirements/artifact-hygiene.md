# Added-file artifact checks

This supplement defines the added-file contract for the two packaged
`artifact-hygiene` rules. The rules evaluate immutable snapshot bytes and
apply across programming languages.

| Requirement | Acceptance contract |
| --- | --- |
| ARTIFACT-01 Added binary files | `no-committed-binaries` matches the configured final filename suffix without regard to ASCII case. Its default list is `.class`, `.jar`, `.war`, `.ear`, `.so`, `.dll`, `.dylib`, `.exe`, `.o`, `.a`, `.lib`, `.pyc`, `.pyo` and `.wasm`. When `scan_content` is true, the rule inspects at most the first 4,096 bytes of suffix-unmatched files. NUL or malformed UTF-8 in that prefix yields a content finding. A UTF-8 character extending beyond a partial sample does not count as malformed. |
| ARTIFACT-02 Added file size | `oversized-added-file` compares the full captured length with `max_added_bytes`, which defaults to 1,048,576 bytes. A size finding requires a strictly greater length. A file outside the acquisition budget blocks snapshot acquisition and can yield `snapshot.file_limit` in `doctor`. |
| ARTIFACT-03 Scope and findings | The rules evaluate included `added` paths after acquisition. `allow_paths` exempts matching repository-relative paths from that rule. Each finding identifies the file without a line range and carries `kind`, `extension` and full captured `bytes`. Diagnostic kinds are `binary_extension`, `binary_content` and `oversized_added_file`. A suffix match takes precedence for binary classification. |
| ARTIFACT-04 Activation and bounds | Both shared rules declare `files` and are disabled and optional by default. The binary rule enables its content detector and defaults to error severity. The size rule enables its size detector and defaults to warning severity. Each check is limited to 10,000 eligible files, 32 MiB of inspected prefixes, and the rule deadline. Exhaustion yields an incomplete result and blocks an overall gate that requires the check. |

See the [rule reference](../../docs/en/02-reference/02-rules.md#added-file-artifact-checks)
and the [requirement-to-test evidence](../../docs/en/04-contributor-guide/03-requirements-to-test-evidence.md#added-file-artifact-checks).
