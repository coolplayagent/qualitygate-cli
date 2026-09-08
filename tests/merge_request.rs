mod common;
#[path = "common/http.rs"]
mod http;

use common::*;
use http::{Server, json};
use serde_json::{Value, json as value};
use std::{path::Path, process::Command};

fn git_output(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

struct Branches {
    root: tempfile::TempDir,
    base: String,
    source: String,
    target: String,
}

fn branches() -> Branches {
    let root = fixture();
    let path = root.path();
    std::fs::write(path.join("qualitygate.yaml"), "schema_version: 1\nrules: {line-ending: {enabled: true, required: true, severity: error}}\n").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "policy"]);
    let base = git_output(path, &["rev-parse", "HEAD"]);
    git(path, &["checkout", "-qb", "target"]);
    std::fs::write(path.join("target-only.txt"), "target advance\n").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "target"]);
    let target = git_output(path, &["rev-parse", "HEAD"]);
    git(path, &["checkout", "-qb", "topic", &base]);
    std::fs::write(path.join("hello.txt"), "source violation\r\n").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "source"]);
    let source = git_output(path, &["rev-parse", "HEAD"]);
    git(path, &["checkout", "-q", "target"]);
    git(
        path,
        &[
            "remote",
            "add",
            "origin",
            "ssh://git@127.0.0.1/team/repo.git",
        ],
    );
    Branches {
        root,
        base,
        source,
        target,
    }
}

fn github(source: &str, target: &str) -> String {
    json(
        value!({"number":2,"head":{"ref":"topic","sha":source},"base":{"ref":"target","sha":target,"repo":{"full_name":"team/repo"}}}),
    )
}

fn run(branches: &Branches, server: &Server, format: &str) -> std::process::Output {
    cli(
        branches.root.path(),
        &[
            "check",
            "--mr",
            &format!("{}team/repo/pull/2", server.base),
            "--mr-api-base",
            &server.base,
            "--format",
            format,
        ],
    )
}

#[test]
fn github_compares_merge_base_to_source_and_preserves_user_checkout() {
    let branches = branches();
    let root = branches.root.path();
    std::fs::write(root.join("hello.txt"), "staged user bytes\n").unwrap();
    git(root, &["add", "hello.txt"]);
    std::fs::write(root.join("hello.txt"), "unstaged user bytes\n").unwrap();
    let index = std::fs::read(root.join(".git/index")).unwrap();
    let refs = git_output(root, &["show-ref"]);
    let server = Server::new(vec![github(&branches.source, &branches.target); 2]);
    let result = report(&run(&branches, &server, "json"), 1);
    assert_eq!(result["snapshot"]["mode"], "mr");
    assert_eq!(result["snapshot"]["base"], branches.base);
    assert_eq!(result["snapshot"]["head"], branches.source);
    assert_eq!(
        result["snapshot"]["merge_request"]["target_head"],
        branches.target
    );
    assert_eq!(
        result["snapshot"]["merge_request"]["merge_base"],
        branches.base
    );
    assert_eq!(
        result["snapshot"]["merge_request"]["api_response_digests"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let argv = result["checks"][0]["diagnostics"][0]["recheck"]["argv"]
        .as_array()
        .unwrap();
    assert!(argv.contains(&value!("--mr")));
    assert!(argv.contains(&value!("--mr-api-base")));
    assert_eq!(git_output(root, &["rev-parse", "HEAD"]), branches.target);
    assert_eq!(git_output(root, &["show-ref"]), refs);
    assert_eq!(std::fs::read(root.join(".git/index")).unwrap(), index);
    assert_eq!(
        std::fs::read_to_string(root.join("hello.txt")).unwrap(),
        "unstaged user bytes\n"
    );
    assert_eq!(server.requests.lock().unwrap().len(), 2);
    for format in ["table", "markdown"] {
        let server = Server::new(vec![github(&branches.source, &branches.target); 2]);
        let output = run(&branches, &server, format);
        assert_eq!(output.status.code(), Some(1));
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(&format!("Merge base: {}", branches.base)));
        assert!(text.contains(&format!("Source: {}", branches.source)));
    }
}

#[test]
fn target_branch_movement_invalidates_evidence_even_when_merge_base_is_unchanged() {
    let branches = branches();
    let root = branches.root.path();
    std::fs::write(root.join("target-only.txt"), "target advances again\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "advance"]);
    let later = git_output(root, &["rev-parse", "HEAD"]);
    let server = Server::new(vec![
        github(&branches.source, &branches.target),
        github(&branches.source, &later),
    ]);
    let result = report(&run(&branches, &server, "json"), 2);
    assert_eq!(result["gate"]["decision"], "incomplete");
    assert!(
        result["gate"]
            .to_string()
            .contains("Source snapshot changed")
    );
    assert_eq!(result["snapshot"]["base"], branches.base);
}

#[test]
fn gitlab_resolves_current_target_branch_and_revalidates_both_provider_responses() {
    let branches = branches();
    let mr = json(
        value!({"iid":2,"project_id":7,"target_project_id":7,"source_branch":"topic","target_branch":"target","sha":branches.source,"diff_refs":{"head_sha":branches.source,"start_sha":branches.base,"base_sha":branches.base}}),
    );
    let target = json(value!({"name":"target","commit":{"id":branches.target}}));
    let server = Server::new(vec![mr.clone(), target.clone(), mr, target]);
    let result = report(
        &cli(
            branches.root.path(),
            &[
                "check",
                "--mr",
                &format!("{}team/repo/-/merge_requests/2", server.base),
                "--format",
                "json",
            ],
        ),
        1,
    );
    assert_eq!(result["snapshot"]["merge_request"]["provider"], "gitlab");
    assert_eq!(
        result["snapshot"]["merge_request"]["target_head"],
        branches.target
    );
    assert_eq!(result["snapshot"]["base"], branches.base);
    assert_eq!(server.requests.lock().unwrap().len(), 4);
}

#[test]
fn provider_failures_origin_mismatches_and_shallow_history_are_incomplete() {
    let branches = branches();
    let server = Server::new(vec![
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".into(),
    ]);
    assert!(
        report(&run(&branches, &server, "json"), 2)
            .to_string()
            .contains("404")
    );
    git(
        branches.root.path(),
        &[
            "remote",
            "set-url",
            "origin",
            "git@127.0.0.1:wrong/repo.git",
        ],
    );
    let server = Server::new(vec![]);
    assert!(
        report(&run(&branches, &server, "json"), 2)
            .to_string()
            .contains("does not match")
    );
    assert!(server.requests.lock().unwrap().is_empty());
    git(
        branches.root.path(),
        &["remote", "set-url", "origin", "git@127.0.0.1:team/repo.git"],
    );
    std::fs::write(
        branches.root.path().join(".git/shallow"),
        format!("{}\n", branches.base),
    )
    .unwrap();
    let server = Server::new(vec![github(&branches.source, &branches.target)]);
    assert!(
        report(&run(&branches, &server, "json"), 2)
            .to_string()
            .contains("complete Git ancestry")
    );
}

#[test]
fn mr_arguments_reject_ambiguous_snapshot_selections() {
    let root = fixture();
    for extra in [
        vec!["--staged"],
        vec!["--worktree"],
        vec!["--path", "hello.txt"],
        vec!["--diff", "HEAD~1..HEAD"],
        vec!["--base", "HEAD"],
    ] {
        let mut args = vec!["check", "--mr", "https://github.com/team/repo/pull/2"];
        args.extend(extra);
        assert_eq!(cli(root.path(), &args).status.code(), Some(2));
    }
    assert_eq!(
        cli(
            root.path(),
            &["check", "--mr-api-base", "https://api.github.com/"]
        )
        .status
        .code(),
        Some(2)
    );
}

#[test]
fn final_provider_failure_preserves_initial_comparison_and_rule_diagnostics() {
    let branches = branches();
    let server = Server::new(vec![
        github(&branches.source, &branches.target),
        "HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\n\r\n".into(),
    ]);
    let result = report(&run(&branches, &server, "json"), 2);
    assert_eq!(result["snapshot"]["head"], branches.source);
    assert_eq!(result["checks"][0]["verdict"], "fail");
    assert!(
        !result["checks"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        result["gate"]
            .to_string()
            .contains("Cannot revalidate the source snapshot")
    );
}

#[cfg(unix)]
#[test]
fn missing_objects_are_fetched_without_moving_refs_index_or_fetch_head() {
    let remote = branches();
    let client = tempfile::tempdir().unwrap();
    git(client.path(), &["init", "-q"]);
    git(
        client.path(),
        &[
            "fetch",
            "-q",
            remote.root.path().to_str().unwrap(),
            &remote.target,
        ],
    );
    git(client.path(), &["checkout", "-q", "--detach", "FETCH_HEAD"]);
    git(
        client.path(),
        &["remote", "add", "origin", "git@127.0.0.1:team/repo.git"],
    );
    let script = client.path().join(".git/fixture-ssh.sh");
    let quote = |text: &str| format!("'{}'", text.replace('\'', "'\\''"));
    std::fs::write(
        &script,
        format!(
            "exec git-upload-pack {}\n",
            quote(remote.root.path().join(".git").to_str().unwrap())
        ),
    )
    .unwrap();
    let fetch_head = std::fs::read(client.path().join(".git/FETCH_HEAD")).unwrap();
    let index = std::fs::read(client.path().join(".git/index")).unwrap();
    let server = Server::new(vec![github(&remote.source, &remote.target); 2]);
    let output = Command::new(env!("CARGO_BIN_EXE_qualitygate"))
        .env("QUALITYGATE_HOME", client.path().join(".git/evidence"))
        .env("GIT_SSH_VARIANT", "simple")
        .env(
            "GIT_SSH_COMMAND",
            format!("sh {}", quote(script.to_str().unwrap())),
        )
        .arg("--root")
        .arg(client.path())
        .args([
            "check",
            "--mr",
            &format!("{}team/repo/pull/2", server.base),
            "--mr-api-base",
            &server.base,
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    let result: Value = report(&output, 1);
    assert_eq!(result["snapshot"]["head"], remote.source);
    assert_eq!(
        std::fs::read(client.path().join(".git/FETCH_HEAD")).unwrap(),
        fetch_head
    );
    assert_eq!(
        std::fs::read(client.path().join(".git/index")).unwrap(),
        index
    );
    assert_eq!(
        git_output(client.path(), &["rev-parse", "HEAD"]),
        remote.target
    );
    assert_eq!(
        git_output(client.path(), &["for-each-ref", "--format=%(refname)"]),
        ""
    );
    assert_eq!(
        git_output(client.path(), &["rev-parse", "--verify", &remote.source]),
        remote.source
    );
}
