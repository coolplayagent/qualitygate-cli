mod common;

use common::{cli, fixture, git, report};
use serde_json::Value;
use std::{fs, path::Path};

const TYPESCRIPT: &str =
    include_str!("../skills/qualitygate-cli/references/typescript-deprecation.yaml");
const SHELL: &str = include_str!("../skills/qualitygate-cli/references/shell-deprecation.yaml");

fn write(root: &Path, file: &str, text: &str) {
    let file = root.join(file);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, text).unwrap();
}

fn delivery(root: &Path, exit: i32) -> Value {
    report(
        &cli(
            root,
            &[
                "check",
                "--worktree",
                "--profile",
                "full",
                "--format",
                "json",
            ],
        ),
        exit,
    )
}

#[test]
fn shipped_eslint_and_shellcheck_policies_load() {
    let fixture = fixture();
    for policy in [TYPESCRIPT, SHELL] {
        write(fixture.path(), "qualitygate.yaml", policy);
        report(
            &cli(fixture.path(), &["config", "--show", "--format", "json"]),
            0,
        );
    }
}

#[test]
#[ignore = "requires pinned type-aware ESLint and Node; mandatory live ESLint CI job"]
fn real_typed_eslint_blocks_unchanged_typescript_and_javascript_use_sites() {
    let fixture = fixture();
    let root = fixture.path();
    write(root, "qualitygate.yaml", TYPESCRIPT);
    write(
        root,
        "eslint.config.mjs",
        include_str!(
            "../skills/qualitygate-cli/references/typescript-deprecation-eslint.config.mjs"
        ),
    );
    write(
        root,
        "tsconfig.json",
        include_str!("../skills/qualitygate-cli/references/typescript-deprecation-tsconfig.json"),
    );
    let declaration = "export declare function old(): number;\n";
    write(root, "src/api.d.ts", declaration);
    let consumer = "import { old } from './api.js';\nexport const result = old();\n";
    write(root, "src/typed-client.ts", consumer);
    write(root, "src/js-client.js", consumer);
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "two clean callers"]);
    assert_eq!(delivery(root, 0)["gate"]["decision"], "pass");

    write(
        root,
        "src/api.d.ts",
        &format!("/** @deprecated Use the current API. */\n{declaration}"),
    );
    let result = delivery(root, 1);
    assert_eq!(
        result["selection"]["changed_files"],
        serde_json::json!(["src/api.d.ts"])
    );
    let diagnostics = result["checks"][0]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 2);
    for file in ["src/typed-client.ts", "src/js-client.js"] {
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic["file"] == file
                && diagnostic["evidence"]["rule"] == "@typescript-eslint/no-deprecated"
        }));
    }

    write(root, "src/api.d.ts", declaration);
    assert_eq!(delivery(root, 0)["gate"]["decision"], "pass");
    write(root, "src/typed-client.ts", "const = ;\n");
    assert_eq!(delivery(root, 2)["gate"]["decision"], "incomplete");
}

#[test]
#[ignore = "requires pinned ShellCheck and Bash; mandatory live ShellCheck CI job"]
fn real_shellcheck_blocks_configured_legacy_bash_syntax_and_recovers() {
    let fixture = fixture();
    let root = fixture.path();
    write(root, "qualitygate.yaml", SHELL);
    write(
        root,
        "scripts/build.sh",
        "#!/bin/bash\nprintf '%s\\n' \"$[1 + 2]\"\n",
    );
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "historical legacy Bash syntax"]);
    write(root, "hello.txt", "unrelated delivery\n");
    let result = delivery(root, 1);
    let diagnostics = result["checks"][1]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["file"], "scripts/build.sh");
    assert!(
        diagnostics[0]["evidence"]["rule"]
            .as_str()
            .unwrap()
            .ends_with("SC2007")
    );

    write(
        root,
        "scripts/build.sh",
        "#!/bin/bash\nprintf '%s\\n' \"$((1 + 2))\"\n",
    );
    assert_eq!(delivery(root, 0)["gate"]["decision"], "pass");
    write(root, "scripts/build.sh", "#!/bin/bash\nif then\n");
    assert_eq!(delivery(root, 2)["gate"]["decision"], "incomplete");
}
