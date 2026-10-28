use globset::Glob;
use std::{path::Path, time::Duration};

#[tokio::test]
async fn documentation_repository_integration_gate_covers_cargo_targets() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(root.join("qualitygate.yaml")).unwrap();
    let policy: serde_norway::Value = serde_norway::from_str(&text).unwrap();
    let integration = policy["checks"]
        .as_sequence()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "integration")
        .unwrap();
    let argv: Vec<_> = integration["argv"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|arg| arg.as_str().unwrap())
        .collect();
    assert_eq!(&argv[..2], ["cargo", "test"]);
    assert!(argv.contains(&"--locked") && argv.contains(&"--all-features"));
    let mut patterns = Vec::new();
    let mut args = argv[2..].iter();
    while let Some(arg) = args.next() {
        match *arg {
            "--locked" | "--all-features" | "--no-fail-fast" => {}
            "--test" => patterns.push(Glob::new(args.next().unwrap()).unwrap().compile_matcher()),
            other => {
                panic!("Integration gate must not filter tests or select unit targets: {other}")
            }
        }
    }
    assert!(!patterns.is_empty());

    // Ask Cargo for its actual target inventory; do not duplicate target discovery
    // or recursively compile tests while the parent Cargo process is running.
    let output = qualitygate::runner::capture(
        &[
            env!("CARGO").into(),
            "metadata".into(),
            "--no-deps".into(),
            "--locked".into(),
            "--offline".into(),
            "--format-version".into(),
            "1".into(),
        ],
        root,
        None,
        Duration::from_secs(30),
    )
    .await
    .unwrap();
    assert!(!output.timed_out, "Cargo metadata timed out");
    assert_eq!(output.capture_error, None);
    assert_eq!(
        output.exit_code,
        Some(0),
        "Cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let package = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == env!("CARGO_PKG_NAME"))
        .unwrap();
    let targets: Vec<_> = package["targets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|target| target["kind"] == serde_json::json!(["test"]))
        .map(|target| target["name"].as_str().unwrap())
        .collect();
    assert!(!targets.is_empty());
    let missing: Vec<_> = targets
        .iter()
        .filter(|target| !patterns.iter().any(|pattern| pattern.is_match(target)))
        .collect();
    assert!(
        missing.is_empty(),
        "Unselected integration targets: {missing:?}"
    );
}
