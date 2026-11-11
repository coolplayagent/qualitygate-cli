use crate::{application::judgment::PilotOptions as Options, snapshot};
use serde_json::{Value, json};
use std::{fs, path::Path};
fn fixture() -> (tempfile::TempDir, Value, Value) {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("runs")).unwrap();
    let digest = snapshot::digest(b"scope");
    let decision = snapshot::digest(b"decision");
    let labels = json!({"schema_version":1,"repository_digest":digest,"question_digest":digest,
        "provider_binding_digest":digest,"model_digest":digest,"calibrator_digest":digest,"label_source_digest":digest,
        "target_event":"finding_valid_after_independent_review","validation_from_unix":100,"validation_until_unix":200,
        "audit_fraction":1.0,"labels":[{"decision_id":decision,"finding_valid":true,"reviewer":"independent",
            "source_digest":digest,"reviewed_unix":160,"independent":true}]});
    let calibration = json!({"id":"calibration","declared_status":"valid","provider_digest":digest,
        "model_digest":digest,"calibrator_digest":digest,"label_source_digest":digest,"question_digest":digest,
        "target_event":"finding_valid_after_independent_review","repository_digest":digest,"language":"rust","rule_id":"lint",
        "labeled_from_unix":1,"labeled_until_unix":99,"expires_unix":201,"revoked":false,"sample_count":30,
        "independent_label_count":30,"brier_score":0.1,"log_loss":0.2,"near_threshold_error":0.1,
        "risk_coverage":[],"known_limits":[]});
    let run = json!({"schema_version":1,"decision_id":decision,"mode":"advisory",
        "original_gate":{"complete":true,"decision":"pass","blockers":[]},"report_digest":digest,"policy_digest":digest,
        "repository_digest":digest,"question_digest":digest,"rule_id":"lint","language":"rust","evidence_id":"warning",
        "assessment":{"kind":"probabilistic","question_id":"review", "primitive":"choice", "selected":"valid", "applicability":"in_domain", "calibration_ref":"calibration", "evidence_refs":[], "distribution":{"valid":0.8,"invalid":0.2},
            "target_event":"finding_valid_after_independent_review","model_digest":digest},
        "complete":true,"route":"prioritize_review","calibration_status":"valid","calibration":calibration,
        "review_priority_threshold":0.8,"provider":null,"provider_version_digest":digest,"provider_binding_digest":digest,
        "artifacts":[],"audit":{"started_unix_nanos":150_000_000_000u64}});
    (root, labels, run)
}

fn save(root: &Path, labels: &Value, run: Option<&Value>) -> Options {
    fs::write(
        root.join("labels.json"),
        serde_json::to_vec(labels).unwrap(),
    )
    .unwrap();
    if let Some(run) = run {
        let directory = root.join("runs").join(
            run["decision_id"]
                .as_str()
                .unwrap()
                .strip_prefix("sha256:")
                .unwrap(),
        );
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("decision.json"),
            serde_json::to_vec(run).unwrap(),
        )
        .unwrap();
    }
    Options {
        runs: root.join("runs"),
        labels: root.join("labels.json"),
        audit_seed: "independent-audit-seed".into(),
    }
}

#[tokio::test]
async fn pilot_dispatch_envelope_preserves_scoring_and_cannot_grant_delivery_authority() {
    use clap::Parser;
    let (root, labels, run) = fixture();
    let options = save(root.path(), &labels, Some(&run));
    for envelope in [false, true] {
        let mut args = vec![
            "qualitygate",
            "--root",
            root.path().to_str().unwrap(),
            "--format",
            "json",
        ];
        if envelope {
            args.push("--envelope");
        }
        args.extend([
            "judgment",
            "pilot",
            "--runs",
            options.runs.to_str().unwrap(),
            "--labels",
            options.labels.to_str().unwrap(),
            "--audit-seed",
            &options.audit_seed,
        ]);
        let (text, code) = crate::interfaces::cli::Cli::try_parse_from(args)
            .unwrap()
            .run()
            .await
            .unwrap();
        assert_eq!(code, 0);
        let result: Value = serde_json::from_str(&text).unwrap();
        let summary = if envelope {
            &result["payload"]
        } else {
            &result
        };
        assert_eq!(summary["gate_effect"], "none");
        assert_eq!(summary["counts"]["probabilistic_labeled"], 1);
    }
}
