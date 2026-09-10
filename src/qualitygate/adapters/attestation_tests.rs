use super::*;
use crate::config::attestation;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};

fn fixture() -> (SigningKey, TrustStore, ManualSubject, Value) {
    let key = SigningKey::from_bytes(&[7; 32]);
    let store = attestation::parse(&serde_json::to_vec(&json!({
        "schema_version":1,"repository":"example/project","max_age_seconds":3600,
        "keys":[{"id":"review","public_key":general_purpose::STANDARD.encode(key.verifying_key().as_bytes()),
            "checks":["manual"],"tasks":["task"],"allow_repository_checks":true}]
    })).unwrap()).unwrap();
    let expected: ManualSubject = serde_json::from_value(json!({
        "repository":"example/project","snapshot":{"mode":"worktree","base":"base","head":"head",
            "content_digest":"sha256:code","path_filter":null,"merge_request":null},
        "policy":{"config_digest":"sha256:config","rules_digest":"sha256:rules","task_contract_digest":null},
        "check_id":"manual","task":null
    })).unwrap();
    let record = json!({"schema_version":1,"record_id":"record-1","subject":expected,
        "reviewer":"reviewer@example.test","decision":"approved","reason":"Observed expected behavior",
        "issued_at":100,"expires_at":200});
    (key, store, expected, record)
}

fn signed(key: &SigningKey, record: &Value) -> Vec<u8> {
    let bytes = serde_json::to_vec(record).unwrap();
    serde_json::to_vec(&json!({"payloadType":PAYLOAD_TYPE,"payload":general_purpose::STANDARD.encode(&bytes),
        "signatures":[{"keyid":"review","sig":general_purpose::STANDARD.encode(key.sign(&pae(PAYLOAD_TYPE, &bytes)).to_bytes())}]})).unwrap()
}

#[test]
fn authenticated_payload_and_authorized_scope_are_both_required() {
    let (key, store, expected, record) = fixture();
    let verified = verify(&signed(&key, &record), &store, &expected, 150).unwrap();
    assert_eq!(
        verified.record.decision,
        crate::domain::ManualDecision::Approved
    );
    assert_eq!(verified.signer_key_id, "review");
    assert!(verified.public_key_digest.starts_with("sha256:"));
    for altered in [
        json!({"repository":"elsewhere"}),
        json!({"check_id":"other"}),
        json!({"task":{"task_id":"task","acceptance_id":"one"}}),
        json!({"snapshot":{"mode":"staged","base":"base","head":"head","content_digest":"sha256:code","path_filter":null,"merge_request":null}}),
        json!({"policy":{"config_digest":"sha256:other","rules_digest":"sha256:rules","task_contract_digest":null}}),
    ] {
        let mut invalid = record.clone();
        for (field, value) in altered.as_object().unwrap() {
            invalid["subject"][field] = value.clone();
        }
        assert!(verify(&signed(&key, &invalid), &store, &expected, 150).is_err());
    }
    for changed in ["checks", "tasks", "allow_repository_checks"] {
        let mut invalid = store.clone();
        let mut task_expected = expected.clone();
        let mut task_record = record.clone();
        match changed {
            "checks" => invalid.keys[0].checks.clear(),
            "tasks" => {
                invalid.keys[0].tasks.clear();
                task_expected.task = Some(crate::domain::TaskBinding {
                    task_id: "task".into(),
                    acceptance_id: "one".into(),
                });
                task_record["subject"] = serde_json::to_value(&task_expected).unwrap();
            }
            _ => invalid.keys[0].allow_repository_checks = false,
        }
        assert!(verify(&signed(&key, &task_record), &invalid, &task_expected, 150).is_err());
    }
    let mut rejected = record;
    rejected["decision"] = json!("rejected");
    assert_eq!(
        verify(&signed(&key, &rejected), &store, &expected, 150)
            .unwrap()
            .record
            .decision,
        crate::domain::ManualDecision::Rejected
    );
}

#[test]
fn dsse_byte_identity_encoding_and_key_hints_do_not_bypass_signatures() {
    assert_eq!(
        pae("http://example.com/HelloWorld", b"hello world"),
        b"DSSEv1 29 http://example.com/HelloWorld 11 hello world"
    );
    let (key, store, expected, record) = fixture();
    let original: Value = serde_json::from_slice(&signed(&key, &record)).unwrap();
    for engine in [
        general_purpose::URL_SAFE,
        general_purpose::STANDARD_NO_PAD,
        general_purpose::URL_SAFE_NO_PAD,
    ] {
        let mut value = original.clone();
        for path in [vec!["payload"], vec!["signatures", "0", "sig"]] {
            let field = if path.len() == 1 {
                &mut value["payload"]
            } else {
                &mut value["signatures"][0]["sig"]
            };
            *field = json!(engine.encode(decode(field.as_str().unwrap()).unwrap()));
        }
        value["signatures"][0]["keyid"] = json!("");
        verify(&serde_json::to_vec(&value).unwrap(), &store, &expected, 150).unwrap();
    }
    for field in [
        "type",
        "body",
        "key",
        "sig",
        "encoding",
        "length",
        "count",
        "signature_count",
        "unknown",
    ] {
        let mut value = original.clone();
        match field {
            "type" => value["payloadType"] = json!("application/json"),
            "body" => value["payload"] = json!(general_purpose::STANDARD.encode(b"{}")),
            "key" => value["signatures"][0]["keyid"] = json!("unknown"),
            "sig" => {
                value["signatures"][0]["sig"] = json!(general_purpose::STANDARD.encode([0; 64]))
            }
            "encoding" => value["signatures"][0]["sig"] = json!("!bad"),
            "length" => value["signatures"][0]["sig"] = json!("YQ=="),
            "count" => value["signatures"] = json!([]),
            "signature_count" => {
                value["signatures"] = json!(vec![value["signatures"][0].clone(); 17])
            }
            _ => value["private_key"] = json!("forbidden"),
        }
        assert!(
            verify(&serde_json::to_vec(&value).unwrap(), &store, &expected, 150).is_err(),
            "{field}"
        );
    }
    assert!(verify(&vec![b' '; MAX_ENVELOPE_BYTES + 1], &store, &expected, 150).is_err());
    assert!(verify(b"not JSON", &store, &expected, 150).is_err());
    assert!(
        verify(
            &signed(&SigningKey::from_bytes(&[8; 32]), &record),
            &store,
            &expected,
            150
        )
        .is_err()
    );
}

#[test]
fn signed_decisions_require_validity_revocation_and_unambiguous_fields() {
    let (key, store, expected, record) = fixture();
    for (field, value) in [
        ("schema_version", json!(2)),
        ("record_id", json!("")),
        ("reviewer", json!(" ")),
        ("reason", json!("x".repeat(16_385))),
        ("issued_at", json!(151)),
        ("expires_at", json!(150)),
        ("expires_at", json!(99)),
        ("expires_at", json!(3701)),
        ("decision", json!("pending")),
        ("unsigned_hint", json!(true)),
    ] {
        let mut changed = record.clone();
        changed[field] = value;
        assert!(
            verify(&signed(&key, &changed), &store, &expected, 150).is_err(),
            "{field}"
        );
    }
    let mut revoked = store.clone();
    revoked.revoked_records.push("record-1".into());
    assert!(verify(&signed(&key, &record), &revoked, &expected, 150).is_err());
    let mut short = store;
    short.max_age_seconds = 20;
    assert!(verify(&signed(&key, &record), &short, &expected, 150).is_err());
    let payload = br#"{"schema_version":1,"schema_version":1}"#;
    let envelope = json!({"payloadType":PAYLOAD_TYPE,"payload":general_purpose::STANDARD.encode(payload),
        "signatures":[{"keyid":"review","sig":general_purpose::STANDARD.encode(key.sign(&pae(PAYLOAD_TYPE,payload)).to_bytes())}]});
    assert!(
        verify(
            &serde_json::to_vec(&envelope).unwrap(),
            &short,
            &expected,
            150
        )
        .is_err()
    );
}

#[test]
fn trust_store_rejects_weak_duplicate_unbounded_and_unknown_keys() {
    let (_, store, _, _) = fixture();
    for (field, value) in [
        ("schema_version", json!(2)),
        ("repository", json!("")),
        ("keys", json!([])),
        ("max_age_seconds", json!(0)),
        ("revoked_records", json!(["same", "same"])),
        ("private_key", json!("secret")),
    ] {
        let mut changed = serde_json::to_value(&store).unwrap();
        changed[field] = value;
        assert!(attestation::parse(&serde_json::to_vec(&changed).unwrap()).is_err());
    }
    for value in [
        "!bad".to_string(),
        "YQ==".into(),
        general_purpose::STANDARD.encode([0; 32]),
    ] {
        let mut changed = store.clone();
        changed.keys[0].public_key = value;
        assert!(validate_keys(&changed).is_err());
    }
    let mut duplicate = store.clone();
    duplicate.keys.push(duplicate.keys[0].clone());
    assert!(attestation::parse(&serde_json::to_vec(&duplicate).unwrap()).is_err());
    duplicate.keys[1].id = "alias".into();
    assert!(validate_keys(&duplicate).is_err());
    let mut invalid = store;
    invalid.keys[0].checks.clear();
    assert!(attestation::parse(&serde_json::to_vec(&invalid).unwrap()).is_err());
    assert!(attestation::parse(&vec![b' '; 256 * 1024 + 1]).is_err());
}
