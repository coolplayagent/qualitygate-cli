use super::*;
use base64::Engine;
use serde_json::json;

fn fixture() -> (tempfile::TempDir, tempfile::TempDir, Vec<CommandCheck>) {
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let public = ed25519_dalek::SigningKey::from_bytes(&[7; 32]).verifying_key();
    std::fs::write(external.path().join("trust.json"), serde_json::to_vec(&json!({
        "schema_version":1,"repository":"example/project","max_age_seconds":3600,
        "keys":[{"id":"review","public_key":base64::engine::general_purpose::STANDARD.encode(public.as_bytes()),"checks":["manual"],"allow_repository_checks":true}]
    })).unwrap()).unwrap();
    let checks = vec![
        serde_json::from_value(
            json!({"id":"manual","kind":"manual","evidence_file":"review.json"}),
        )
        .unwrap(),
    ];
    (root, external, checks)
}

#[test]
fn external_inputs_are_separate_bounded_and_revalidated() {
    let (root, external, checks) = fixture();
    let store = external.path().join("trust.json");
    let load_inputs = || load(root.path(), &store, external.path(), &checks);
    let missing = load_inputs().unwrap();
    assert!(record(&missing, &checks[0]).is_err());
    std::fs::write(external.path().join("review.json"), b"signed record").unwrap();
    let inputs = load_inputs().unwrap();
    unchanged(&inputs).unwrap();
    assert_eq!(record(&inputs, &checks[0]).unwrap(), b"signed record");
    std::fs::write(external.path().join("review.json"), b"changed").unwrap();
    assert!(unchanged(&inputs).is_err());
    std::fs::write(external.path().join("review.json"), b"signed record").unwrap();
    std::fs::write(&store, b"changed trust").unwrap();
    assert!(unchanged(&inputs).is_err());
    assert!(load_inputs().is_err());
    assert!(load(root.path(), &store, root.path(), &checks).is_err());
    std::fs::write(root.path().join("trust.json"), &inputs.store_bytes).unwrap();
    assert!(
        load(
            root.path(),
            &root.path().join("trust.json"),
            external.path(),
            &checks
        )
        .is_err()
    );
    std::fs::write(&store, &inputs.store_bytes).unwrap();
    std::fs::write(
        external.path().join("review.json"),
        vec![0; attestation::MAX_ENVELOPE_BYTES + 1],
    )
    .unwrap();
    assert!(record(&load_inputs().unwrap(), &checks[0]).is_err());
    let many: Vec<_> = (0..129)
        .map(|number| {
            let mut check = checks[0].clone();
            check.evidence_file = Some(format!("review-{number}.json"));
            check
        })
        .collect();
    assert!(load(root.path(), &store, external.path(), &many).is_err());
    let mut no_filename = checks[0].clone();
    no_filename.evidence_file = None;
    assert!(record(&inputs, &no_filename).is_err());
    let mut bulk = Vec::new();
    for number in 0..9 {
        let name = format!("bulk-{number}.json");
        std::fs::write(
            external.path().join(&name),
            vec![b' '; attestation::MAX_ENVELOPE_BYTES],
        )
        .unwrap();
        let mut check = checks[0].clone();
        check.id = format!("bulk-{number}");
        check.evidence_file = Some(name);
        bulk.push(check);
    }
    assert!(
        load(root.path(), &store, external.path(), &bulk)
            .err()
            .unwrap()
            .to_string()
            .contains("8 MiB")
    );
}

#[cfg(unix)]
#[test]
fn external_inputs_reject_symlinks_and_special_files() {
    let (root, external, checks) = fixture();
    let store = external.path().join("trust.json");
    std::os::unix::fs::symlink(&store, external.path().join("review.json")).unwrap();
    let inputs = load(root.path(), &store, external.path(), &checks).unwrap();
    assert!(record(&inputs, &checks[0]).is_err());
    std::os::unix::fs::symlink(&store, external.path().join("linked-trust.json")).unwrap();
    assert!(
        load(
            root.path(),
            &external.path().join("linked-trust.json"),
            external.path(),
            &checks
        )
        .is_err()
    );
    std::fs::remove_file(external.path().join("review.json")).unwrap();
    std::fs::create_dir(external.path().join("review.json")).unwrap();
    assert!(
        record(
            &load(root.path(), &store, external.path(), &checks).unwrap(),
            &checks[0]
        )
        .is_err()
    );
}
