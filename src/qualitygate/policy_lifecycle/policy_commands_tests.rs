use super::*;
use serde_json::json;

fn actor() -> Attribution {
    Attribution {
        actor: "reviewer".into(),
        actor_kind: ActorKind::Human,
    }
}
fn page() -> Page {
    Page {
        offset: 0,
        limit: 10,
    }
}
async fn call(root: &std::path::Path, command: Policy) -> Value {
    let (value, code) = run(root.into(), "qualitygate.yaml".into(), command)
        .await
        .unwrap();
    assert_eq!(code, 0);
    value
}
async fn candidate(root: &std::path::Path, command: Candidate) -> Value {
    call(root, Policy::Candidate { command }).await
}

#[tokio::test]
async fn reviewer_retains_evidence_proposes_edits_and_rejects_without_activating_policy() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    std::fs::write(root.join("source.txt"), "Review found a gap").unwrap();
    let record = json!({"schema_version":1,"kind":"review","source_digest":crate::snapshot::digest(b"Review found a gap"),"scope":"policy","timestamp":1,"actor":{"id":"reviewer","kind":"human"},"claims":["Line ending check missing"],"sensitivity":"internal","rule_ids":["line-ending"],"known_limits":["unverified"],"unverified_assumptions":[]});
    std::fs::write(
        root.join("record.json"),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    let retained = call(
        root,
        Policy::Evidence {
            command: Evidence::Add {
                input: "record.json".into(),
                source: "source.txt".into(),
            },
        },
    )
    .await;
    let reference = retained["evidence_ref"].as_str().unwrap().to_owned();
    let shown = call(
        root,
        Policy::Evidence {
            command: Evidence::Show {
                reference: reference.clone(),
            },
        },
    )
    .await;
    assert_eq!(shown["trust"], "unverified_evidence");
    assert_eq!(
        call(
            root,
            Policy::Evidence {
                command: Evidence::List(page())
            }
        )
        .await["total"],
        1
    );
    let created = policy_candidates::create_from_snapshot(
        root,
        "qualitygate.yaml",
        "base",
        [(
            "qualitygate.yaml",
            b"schema_version: 1\nrules:\n  line-ending: {enabled: true}\n".as_slice(),
            false,
        )],
        policy_candidates::Proposal {
            actor: actor().into_actor(),
            reason: "exercise review".into(),
            evidence: vec![reference.clone()],
        },
    )
    .unwrap();
    let parent = created["revision"]["parent_policy_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let proposed = candidate(
        root,
        Candidate::Create {
            from_policy_ref: parent.clone(),
            evidence: vec![reference],
            reason: "evaluate correction".into(),
            attribution: actor(),
        },
    )
    .await;
    let id = proposed["id"].as_str().unwrap().to_owned();
    let disabled = candidate(
        root,
        Candidate::Rules {
            command: CandidateRules::Disable {
                candidate_id: id.clone(),
                rule_id: "line-ending".into(),
                attribution: actor(),
            },
        },
    )
    .await;
    assert_eq!(disabled["changed"], true);
    candidate(
        root,
        Candidate::Rules {
            command: CandidateRules::Enable {
                candidate_id: id.clone(),
                rule_id: "line-ending".into(),
                attribution: actor(),
            },
        },
    )
    .await;
    candidate(
        root,
        Candidate::Rules {
            command: CandidateRules::Configure {
                candidate_id: id.clone(),
                rule_id: "line-ending".into(),
                parameters: vec![],
                severity: Some(Severity::Warning),
                required: Some(false),
                attribution: actor(),
            },
        },
    )
    .await;
    let shown = candidate(root, Candidate::Show { id: id.clone() }).await;
    assert_eq!(shown["config"]["rules"]["line-ending"]["required"], false);
    assert!(
        candidate(root, Candidate::List(page())).await["total"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert_eq!(
        call(
            root,
            Policy::Show {
                digest: parent.clone()
            }
        )
        .await["active"],
        false
    );
    assert_eq!(
        call(
            root,
            Policy::Record {
                reference: parent,
                kind: "blob".into()
            }
        )
        .await["encoding"],
        "base64"
    );
    for action in ["revalidate", "demote", "deprecate", "retire", "revoke"] {
        use clap::Parser;
        let cli = crate::interfaces::cli::Cli::try_parse_from([
            "qualitygate",
            "--root",
            root.to_str().unwrap(),
            "--format",
            "json",
            "rules",
            action,
            "line-ending",
            "--candidate",
            &id,
            "--actor",
            "reviewer",
            "--actor-kind",
            "human",
            "--reason",
            "Review observed lifecycle change",
        ])
        .unwrap();
        let (output, code) = cli.run().await.unwrap();
        assert_eq!(code, 0);
        let result: Value = serde_json::from_str(&output).unwrap();
        assert!(result["revision"].is_object());
    }
    candidate(
        root,
        Candidate::Reject {
            id: id.clone(),
            reason: "independent evidence insufficient".into(),
            attribution: actor(),
        },
    )
    .await;
    assert!(
        run(
            root.into(),
            "qualitygate.yaml".into(),
            Policy::Candidate {
                command: Candidate::Promote { id }
            }
        )
        .await
        .is_err()
    );
    assert!(
        !call(
            root,
            Policy::History {
                cursor: None,
                limit: 10
            }
        )
        .await["events"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let effect = call(
        root,
        Policy::Effectiveness {
            policy_ref: None,
            page: page(),
        },
    )
    .await;
    assert!(effect.is_object());
}
