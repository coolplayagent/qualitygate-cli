use super::*;

const MIB: usize = 1024 * 1024;

#[test]
fn base_and_target_budgets_are_distinct_and_expose_supported_file_boundaries() {
    let root = fixture();
    policy(root.path(), simple());
    std::fs::write(root.path().join("large.data"), vec![b'x'; 2 * MIB + 1]).unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "large base"]);
    std::fs::write(root.path().join("large.data"), "small").unwrap();
    let value = doctor(root.path(), &[], 2);
    assert_eq!(value["budgets"][0]["oversized_file_count"], 1);
    assert_eq!(value["budgets"][1]["oversized_file_count"], 0);
    assert_eq!(value["budgets"][0]["recommended_max_file_mib"], 3);
    let step = value["next_steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["action"] == "increase_file_budget")
        .unwrap();
    let argv: Vec<_> = step["command"]
        .as_array()
        .unwrap()
        .iter()
        .skip(1)
        .map(|v| v.as_str().unwrap())
        .collect();
    let replay = Command::new(env!("CARGO_BIN_EXE_qualitygate"))
        .args(&argv)
        .output()
        .unwrap();
    report(&replay, 0);
    doctor(root.path(), &["--snapshot-max-mib", "512"], 2);
    for (bytes, limit, expected) in [(2 * MIB, "2", 0), (8 * MIB, "8", 0), (8 * MIB + 1, "8", 2)] {
        std::fs::write(root.path().join("large.data"), vec![b'x'; bytes]).unwrap();
        // Commit each boundary to avoid retaining a different over-budget base.
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-qm", "boundary"]);
        let value = doctor(root.path(), &["--snapshot-max-file-mib", limit], expected);
        if expected == 2 {
            assert!(value["budgets"][0]["recommended_max_file_mib"].is_null());
            assert!(
                !value["next_steps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|step| step["action"] == "increase_file_budget")
            );
        }
    }
}

#[test]
fn index_metadata_uses_index_blobs_and_diff_uses_commit_endpoints() {
    let root = fixture();
    policy(root.path(), simple());
    std::fs::write(root.path().join("large.data"), "small").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "policy"]);
    std::fs::write(root.path().join("large.data"), vec![b'x'; 3 * MIB]).unwrap();
    doctor(root.path(), &["--staged"], 0);
    doctor(root.path(), &["--diff", "HEAD~1..HEAD"], 0);
    git(root.path(), &["add", "."]);
    std::fs::write(root.path().join("large.data"), "small").unwrap();
    let value = doctor(root.path(), &["--staged"], 2);
    assert_eq!(value["budgets"][1]["oversized_file_count"], 1);
    doctor(root.path(), &[], 0);
}

#[test]
fn total_budget_and_protected_exclusions_are_blocking_without_mutation() {
    let root = fixture();
    let mut value = simple();
    value["exclude"] = json!(["resource.data"]);
    policy(root.path(), value.clone());
    std::fs::write(root.path().join("resource.data"), vec![b'x'; 3 * MIB]).unwrap();
    doctor(root.path(), &[], 0);
    value["verification_assets"] = json!(["resource.data"]);
    policy(root.path(), value.clone());
    let result = doctor(root.path(), &[], 2);
    assert!(has_code(&result, "snapshot.protected_excluded"));
    assert_eq!(
        result["budgets"][1]["protected_exclusions"],
        json!(["resource.data"])
    );
    value.as_object_mut().unwrap().remove("exclude");
    policy(root.path(), value);
    let result = doctor(
        root.path(),
        &["--snapshot-max-file-mib", "4", "--snapshot-max-mib", "1"],
        2,
    );
    assert!(has_code(&result, "snapshot.total_limit"));
    assert_eq!(result["budgets"][1]["oversized_file_count"], 0);
}

#[test]
fn exclusion_details_are_truncated_but_counts_and_protection_remain_complete() {
    let root = fixture();
    let mut value = simple();
    value["exclude"] = json!(["resources/**"]);
    value["verification_assets"] = json!(["resources/104.data"]);
    policy(root.path(), value);
    std::fs::create_dir(root.path().join("resources")).unwrap();
    for index in 0..105 {
        std::fs::write(root.path().join(format!("resources/{index:03}.data")), "x").unwrap();
    }
    let result = doctor(root.path(), &[], 2);
    assert_eq!(result["budgets"][1]["excluded_file_count"], 105);
    assert_eq!(
        result["budgets"][1]["excluded_paths"]
            .as_array()
            .unwrap()
            .len(),
        100
    );
    assert_eq!(result["budgets"][1]["details_truncated"], true);
    assert!(has_code(&result, "snapshot.protected_excluded"));
}

#[cfg(unix)]
#[test]
fn unsupported_symlinks_are_reported_for_each_selected_side() {
    let root = fixture();
    policy(root.path(), simple());
    std::os::unix::fs::symlink("hello.txt", root.path().join("link")).unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "link"]);
    let value = doctor(root.path(), &[], 2);
    assert_eq!(value["budgets"][0]["unsupported_entry_count"], 1);
    assert_eq!(value["budgets"][1]["unsupported_entry_count"], 1);
    let staged = doctor(root.path(), &["--staged"], 2);
    assert_eq!(staged["budgets"][1]["unsupported_entry_count"], 1);
}

#[test]
fn oversized_index_inventory_is_reported_before_content_acquisition() {
    use std::io::Write;
    use std::process::Stdio;
    let root = fixture();
    policy(root.path(), simple());
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "policy"]);
    let oid = Command::new("git")
        .args(["rev-parse", "HEAD:hello.txt"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(oid.status.success());
    let oid = std::str::from_utf8(&oid.stdout).unwrap().trim();
    let mut child = Command::new("git")
        .args(["update-index", "--index-info"])
        .current_dir(root.path())
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    let input: String = (0..100_001)
        .map(|index| format!("100644 {oid}\tresource-{index:06}\n"))
        .collect();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());
    let result = doctor(root.path(), &["--staged"], 2);
    assert!(has_code(&result, "snapshot.file_count"), "{result}");
    assert_eq!(result["budgets"][1]["file_count"], 100_003);
    assert!(result["snapshot"].is_null());
}
