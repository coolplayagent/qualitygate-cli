# Repository architecture contract

The Rust architecture gate follows the ownership boundaries in [AGENTS.md](../AGENTS.md) and the module-graph approach in the reference repository. Run `cargo test --locked --test quality architecture`.

## Allowed module dependencies

The table lists permitted direct dependencies between top-level production owners. Dependencies within one owner are outside this graph's scope. Adding an owner or changing a direction requires changing the explicit contract and reviewing its implications.

| Owner | May depend on |
|---|---|
| `domain` | No other repository owner |
| `env` | No other repository owner |
| `paths` | `env` |
| `net` | `domain`, `env` |
| `runner` | `domain`, `env` |
| `snapshot` | `domain`, `net`, `paths`, `runner` |
| `config` | `domain`, `paths` |
| `adapters` | `config`, `domain`, `paths`, `snapshot` |
| `application` | `adapters`, `config`, `domain`, `env`, `net`, `paths`, `runner`, `snapshot` |
| `interfaces` | `application`, `config`, `domain`, `snapshot` |
| Binary entry point | `interfaces` |

`lib.rs` declares physical owners and does not hide dependencies behind root re-exports. The CLI delegates candidate rule changes to `config::enable_rule`; configuration validation, confined atomic replacement and permission preservation remain owned by configuration code.

## Checked evidence

The harness follows Rust module declarations from both crate roots. It parses grouped imports, re-exports, aliases, qualified/relative paths, inline modules and qualified references/imports inside macro arguments. File names containing `test` do not exclude production code. Only configurations proven inactive when `test=false` are omitted; platform and feature branches remain in the graph, including `cfg(not(test))`.

The gate rejects forbidden directions and dependency cycles. It also rejects known filesystem/process/network/environment capabilities in the domain or interface layer, environment access outside `env`, ambiguous/missing module files and unsupported production source redirection/generation. It does not expand arbitrary procedural macros or analyze third-party implementation internals; compilation, native-platform tests, Miri and ASan remain separate evidence.

`target/architecture/report.json` records source SHA-256 digests, file/line dependency references and violations. `target/architecture/graph.dot` presents the observed owner graph. The independent CI architecture job uploads both artifacts, including on test failure when artifacts were produced. A source or policy change requires rerunning the gate.

## Documentation gate

Run `cargo test --locked --test quality documentation`. A CommonMark parser extracts rendered headings, inline/reference/image links and code blocks, so examples inside code do not become document links. Local checks cover files, same-file and cross-file Markdown anchors, percent-encoded paths, Chinese headings, inline formatting, duplicate heading suffixes, basic explicit HTML anchors and unclosed backtick/tilde fences.

Undefined link references, missing targets/anchors, repository escapes and unsupported local fragment formats fail with a file and, where available, line. HTML link/anchor values containing entities and fragments on non-Markdown files currently require explicit support. External HTTP(S), mail and data links are recognized without network availability checks. The gate does not claim a complete GitHub renderer.

Authored files retain the 1,000-line limit (`Cargo.lock` is exempt); file inventory and input sizes are bounded and symlinks are rejected. Repository YAML files must parse as configuration mappings. Parser fixtures exercise both accepted Markdown and deliberate broken-link/fence cases.

Reference semantics: [CommonMark parsing](https://docs.rs/pulldown-cmark/0.13.4/pulldown_cmark/) and [GitHub section links](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#section-links).
