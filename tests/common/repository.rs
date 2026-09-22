//! Repository-wide reusable-core contracts, separate from the delivery-only CLI.
use qualitygate::{application, snapshot};
use std::{path::Path, process::Output};

// Only fixture modifiers used by the core regression suite are supported here.
// No production CLI option or environment override enables repository scope.
pub fn check(root: &Path, args: &[&str]) -> Output {
    let mut options = application::CheckOptions {
        root: root.into(),
        config: "qualitygate.yaml".into(),
        selection: snapshot::Selection::Worktree {
            base: "HEAD".into(),
        },
        snapshot_options: snapshot::CaptureOptions::default(),
        profile: "full".into(),
        task: None,
        policy_ref: None,
        output_dir: Some(root.join(".git/qualitygate-core-evidence")),
        trust_store: None,
        evidence_dir: None,
    };
    let mut envelope = false;
    let mut feedback = false;
    let mut args = args.iter().copied();
    while let Some(arg) = args.next() {
        match arg {
            "--staged" => options.selection = snapshot::Selection::Staged,
            "--policy-ref" => options.policy_ref = Some(args.next().unwrap().into()),
            "--profile" => options.profile = args.next().unwrap().into(),
            "--path" => options.snapshot_options.path_filter = Some(args.next().unwrap().into()),
            "--envelope" => envelope = true,
            "--feedback" => feedback = true,
            other => panic!("unsupported core fixture modifier: {other}"),
        }
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let (value, code) = runtime.block_on(async {
        let checked = match application::check(options).await {
            Ok(checked) => checked,
            Err(error) => {
                return (
                    qualitygate::interfaces::incomplete_report(&format!("{error:#}")),
                    2,
                );
            }
        };
        let code = checked.gate.decision.exit_code();
        let value = if feedback {
            let payload = serde_json::from_str(
                &application::feedback::render(checked.clone(), 32768, None)
                    .await
                    .unwrap(),
            )
            .unwrap();
            if envelope {
                serde_json::to_value(
                    qualitygate::domain::decision_envelope::DecisionEnvelope::from_feedback(
                        &checked, payload,
                    )
                    .unwrap(),
                )
                .unwrap()
            } else {
                payload
            }
        } else if envelope {
            serde_json::to_value(
                qualitygate::domain::decision_envelope::DecisionEnvelope::from_check(&checked)
                    .unwrap(),
            )
            .unwrap()
        } else {
            serde_json::to_value(checked).unwrap()
        };
        (value, code)
    });
    #[cfg(windows)]
    let status = {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(u32::from(code))
    };
    #[cfg(unix)]
    let status = {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(i32::from(code) << 8)
    };
    Output {
        status,
        stdout: serde_json::to_vec(&value).unwrap(),
        stderr: Vec::new(),
    }
}
