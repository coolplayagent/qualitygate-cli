//! Opt-in, billable model connectivity checks. Never part of the offline gate.
//! These probes establish CLI access and protocol comprehension, not repair skill.
use qualitygate::{runner, snapshot};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

const BASE: &str = "971455bd705c35e7ba7a7ea5fa905b90eaae2a54";
const MODELS: [&str; 2] = ["gpt-5.6-terra", "gpt-5.6-luna"];

async fn capture(argv: &[String], cwd: &Path, seconds: u64) -> runner::Output {
    runner::capture(argv, cwd, None, Duration::from_secs(seconds))
        .await
        .unwrap()
}

fn persist(directory: &Path, name: &str, bytes: &[u8]) -> Value {
    std::fs::write(directory.join(name), bytes).unwrap();
    json!({"path":name,"bytes":bytes.len(),"digest":snapshot::digest(bytes)})
}

fn version_matches(value: &Value) -> bool {
    value.as_str().is_some_and(|text| {
        text.strip_prefix("qualitygate ").unwrap_or(text) == env!("CARGO_PKG_VERSION")
    })
}

fn observed_version(events: &[Value], executable: &str) -> bool {
    events.iter().any(|event| {
        event["type"] == "item.completed"
            && event["item"]["type"] == "command_execution"
            && event["item"]["exit_code"] == 0
            && event["item"]["command"].as_str().is_some_and(|command| {
                command.contains(executable) && command.contains("--version")
            })
            && event["item"]["aggregated_output"]
                .as_str()
                .is_some_and(|text| {
                    text.lines().any(|line| {
                        line.trim() == format!("qualitygate {}", env!("CARGO_PKG_VERSION"))
                    })
                })
    })
}

fn understood(answer: &Value) -> bool {
    answer["pass_exit"] == 0
        && answer["violation_exit"] == 1
        && answer["incomplete_exit"] == 2
        && answer["zero_tests_accept"] == false
        && answer["policy_change_accept"] == false
        && answer["quick_is_final"] == false
        && version_matches(&answer["version"])
}

#[test]
fn version_answers_accept_the_version_or_banner_and_reject_guesses() {
    assert!(version_matches(&json!(env!("CARGO_PKG_VERSION"))));
    assert!(version_matches(&json!(format!(
        "qualitygate {}",
        env!("CARGO_PKG_VERSION")
    ))));
    for value in [
        json!("0.1.0"),
        json!("unobserved"),
        json!("qualitygate 0.4.0 possibly"),
        json!(null),
    ] {
        assert!(!version_matches(&value));
    }
}

#[test]
fn command_observations_accept_a_banner_line_but_reject_failed_or_unrelated_commands() {
    let mut event = json!({"type":"item.completed","item":{"type":"command_execution",
        "command":"/trusted/qualitygate --version; cat docs/tasks.md","exit_code":0,
        "aggregated_output":format!("setup\nqualitygate {}\nmore documentation\n", env!("CARGO_PKG_VERSION"))}});
    assert!(observed_version(&[event.clone()], "/trusted/qualitygate"));
    event["item"]["exit_code"] = json!(2);
    assert!(!observed_version(&[event.clone()], "/trusted/qualitygate"));
    event["item"]["exit_code"] = json!(0);
    event["item"]["command"] = json!("cat docs/tasks.md");
    assert!(!observed_version(&[event.clone()], "/trusted/qualitygate"));
    event["item"]["command"] = json!("/trusted/qualitygate --version");
    event["item"]["aggregated_output"] = json!("expected qualitygate 0.4.0, but unavailable");
    assert!(!observed_version(&[event], "/trusted/qualitygate"));
}

fn read_bounded(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(runner::MAX_OUTPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > runner::MAX_OUTPUT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Probe artifact exceeds its byte budget",
        ));
    }
    Ok(bytes)
}

#[test]
fn probe_artifact_reads_reject_missing_and_oversized_files() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("answer.json");
    assert!(read_bounded(&file).is_err());
    std::fs::write(&file, b"{}").unwrap();
    assert_eq!(read_bounded(&file).unwrap(), b"{}");
    std::fs::File::create(&file)
        .unwrap()
        .set_len(runner::MAX_OUTPUT_BYTES as u64 + 1)
        .unwrap();
    assert!(read_bounded(&file).is_err());
}

fn retained_artifact(root: &Path, artifact: &Value) -> Vec<u8> {
    // Early probe records serialized OsStr's platform-tagged representation.
    // Decode it explicitly while retaining the same single-filename contract.
    let path: std::path::PathBuf = match artifact["path"].as_str() {
        Some(path) => path.into(),
        None => serde_json::from_value::<std::ffi::OsString>(artifact["path"].clone())
            .unwrap()
            .into(),
    };
    assert!(
        path.components().count() == 1
            && matches!(
                path.components().next(),
                Some(std::path::Component::Normal(_))
            )
    );
    let bytes = read_bounded(&root.join(&path)).unwrap();
    assert_eq!(artifact["bytes"], bytes.len() as u64);
    assert_eq!(artifact["digest"], snapshot::digest(&bytes));
    bytes
}

#[test]
#[ignore = "requires an explicit retained probe directory; offline, no model calls"]
fn recheck_recorded_codex_probe_artifacts() {
    let root = std::path::PathBuf::from(std::env::var_os("QUALITYGATE_PILOT_REPLAY").unwrap());
    let index_bytes = read_bounded(&root.join("index.json")).unwrap();
    let index: Value = serde_json::from_slice(&index_bytes).unwrap();
    assert_eq!(index["source_commit"], BASE);
    let executable = index["qualitygate_executable"]["path"].as_str().unwrap();
    let records = index["records"].as_array().unwrap();
    assert_eq!(records.len(), MODELS.len());
    let mut reviewed = Vec::new();
    for (record, model) in records.iter().zip(MODELS) {
        assert_eq!(record["requested_model"], model);
        let raw = retained_artifact(&root, &record["stdout"]);
        let events: Vec<Value> = std::str::from_utf8(&raw)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let answer: Value =
            serde_json::from_slice(&retained_artifact(&root, &record["answer"])).unwrap();
        let command = observed_version(&events, executable);
        let contract = understood(&answer);
        let complete = record["exit_code"] == 0
            && record["timed_out"] == false
            && record["capture_error"].is_null()
            && record["events_error"].is_null()
            && record["checkout_unchanged"] == true
            && command
            && contract;
        reviewed.push(json!({"requested_model":model,"original_complete":record["complete"],
            "observed_cli_version_execution":command,"contract_understood":contract,"complete":complete}));
    }
    persist(&root, "reassessment.json", &serde_json::to_vec_pretty(&json!({
        "schema_version":1,"kind":"offline_artifact_reassessment","new_model_calls":0,
        "original_index_digest":snapshot::digest(&index_bytes),
        "validator_digest":snapshot::digest(include_bytes!("pilot_codex_live.rs")),
        "records":reviewed,"known_limits":["Local artifact hashes do not authenticate the harness or serving model"]
    })).unwrap());
    assert!(reviewed.iter().all(|record| record["complete"] == true));
}

#[tokio::test]
#[ignore = "requires Codex credentials and consumes model quota; explicit phase-A probe only"]
async fn codex_medium_and_lower_models_share_the_same_cli_contract() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"));
    let destination = source.join("target/pilot-phase-a");
    std::fs::create_dir_all(&destination).unwrap();
    let evidence = tempfile::Builder::new()
        .prefix("codex-")
        .tempdir_in(destination)
        .unwrap()
        .keep();
    println!("Codex phase-A probe evidence: {}", evidence.display());
    let executable = std::env::var_os("QUALITYGATE_PILOT_CLI")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_qualitygate").into())
        .canonicalize()
        .unwrap();
    let prompt = format!(
        "This is a read-only phase-A connectivity and protocol probe, not a repair task. \
         Do not modify files, contact other services, spawn agents or inspect credentials. \
         Run exactly this local executable with --version using the shell tool: {}. \
         Read docs/tasks.md and docs/reports.md in this checkout to determine the existing \
         qualitygate contract. Return the observed version, the exit codes for a complete \
         pass, a blocking violation, and incomplete verification, and whether zero tests, \
         an unauthorized selected-policy change, or quick-only checks can establish final \
         task acceptance. Do not run builds or tests. Do not claim repairs or measured benefits.",
        executable.display()
    );
    let schema = json!({"type":"object","additionalProperties":false,
        "properties":{
            "version":{"type":"string"},"pass_exit":{"type":"integer"},
            "violation_exit":{"type":"integer"},"incomplete_exit":{"type":"integer"},
            "zero_tests_accept":{"type":"boolean"},
            "policy_change_accept":{"type":"boolean"},"quick_is_final":{"type":"boolean"}},
        "required":["version","pass_exit","violation_exit","incomplete_exit",
            "zero_tests_accept","policy_change_accept","quick_is_final"]});
    let schema_bytes = serde_json::to_vec_pretty(&schema).unwrap();
    let schema_ref = persist(&evidence, "response-schema.json", &schema_bytes);
    let prompt_ref = persist(&evidence, "prompt.txt", prompt.as_bytes());
    let version = capture(&["codex".into(), "--version".into()], source, 15).await;
    assert_eq!(version.exit_code, Some(0));
    let version_ref = persist(&evidence, "codex-version.txt", &version.stdout);
    let binary = runner::identity::executable(executable.to_str().unwrap(), source)
        .await
        .unwrap();
    let backend =
        std::env::var("QUALITYGATE_PILOT_SANDBOX_BACKEND").unwrap_or_else(|_| "default".into());
    assert!(["default", "landlock"].contains(&backend.as_str()));
    let mut records = Vec::new();
    // Sequential: no shared checkout, chat history or concurrent model requests.
    for model in MODELS {
        let temporary = tempfile::tempdir().unwrap();
        let checkout = temporary.path().join("repository");
        let cloned = capture(
            &[
                "git".into(),
                "clone".into(),
                "--quiet".into(),
                "--no-hardlinks".into(),
                "--no-checkout".into(),
                source.display().to_string(),
                checkout.display().to_string(),
            ],
            temporary.path(),
            60,
        )
        .await;
        assert_eq!(cloned.exit_code, Some(0));
        let selected = capture(
            &[
                "git".into(),
                "checkout".into(),
                "--quiet".into(),
                "--detach".into(),
                BASE.into(),
            ],
            &checkout,
            30,
        )
        .await;
        assert_eq!(selected.exit_code, Some(0));
        let output_path = evidence.join(format!("{model}-answer.json"));
        let mut argv = vec![
            "codex".into(),
            "exec".into(),
            "--ignore-user-config".into(),
            "--ignore-rules".into(),
            "--ephemeral".into(),
            "--sandbox".into(),
            "read-only".into(),
            "-c".into(),
            "approval_policy=\"never\"".into(),
            "-c".into(),
            "model_reasoning_effort=\"medium\"".into(),
            "--model".into(),
            model.into(),
            "--color".into(),
            "never".into(),
            "--json".into(),
            "--output-schema".into(),
            evidence.join("response-schema.json").display().to_string(),
            "--output-last-message".into(),
            output_path.display().to_string(),
            prompt.clone(),
        ];
        if backend == "landlock" {
            argv.splice(2..2, ["--enable".into(), "use_legacy_landlock".into()]);
        }
        let output = capture(&argv, &checkout, 180).await;
        let stdout_ref = persist(&evidence, &format!("{model}-events.jsonl"), &output.stdout);
        let stderr_ref = persist(&evidence, &format!("{model}-stderr.log"), &output.stderr);
        let answer_result = read_bounded(&output_path);
        let answer_error = answer_result.as_ref().err().map(ToString::to_string);
        let answer_bytes = answer_result.ok();
        let answer = answer_bytes
            .as_deref()
            .and_then(|bytes| serde_json::from_slice::<Value>(bytes).ok());
        let parsed: Result<Vec<Value>, _> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(serde_json::from_str)
            .collect();
        let events_error = parsed.as_ref().err().map(ToString::to_string);
        let events = parsed.unwrap_or_default();
        let usage = events
            .iter()
            .rev()
            .find(|event| event["type"] == "turn.completed")
            .map(|event| event["usage"].clone());
        let observed_version = observed_version(&events, executable.to_str().unwrap());
        let status = capture(
            &["git".into(), "status".into(), "--porcelain".into()],
            &checkout,
            15,
        )
        .await;
        let clean = status.exit_code == Some(0) && status.stdout.is_empty();
        let understood = answer.as_ref().is_some_and(understood);
        let complete = output.exit_code == Some(0)
            && !output.timed_out
            && output.capture_error.is_none()
            && events_error.is_none()
            && observed_version
            && understood
            && clean;
        let answer_ref = answer_bytes.as_ref().map(|bytes| {
            json!({"path":output_path.file_name().unwrap().to_str().unwrap(),
            "bytes":bytes.len(),"digest":snapshot::digest(bytes)})
        });
        records.push(json!({"requested_model":model,"actual_model":null,
            "actual_model_reason":"Codex JSONL does not attest the serving model identity",
            "reasoning_effort":"medium","sandbox_backend":backend,"argv":argv,"cwd":checkout,
            "events_error":events_error,"answer_error":answer_error,
            "exit_code":output.exit_code,"timed_out":output.timed_out,"capture_error":output.capture_error,
            "started_at_ms":output.started_at_ms,"ended_at_ms":output.ended_at_ms,"duration_ms":output.duration_ms,
            "stdout":stdout_ref,"stderr":stderr_ref,"answer":answer_ref,"usage":usage,
            "cost":null,"cost_reason":"No billed amount is available in this probe",
            "observed_cli_version_execution":observed_version,"contract_understood":understood,
            "checkout_unchanged":clean,"complete":complete}));
        persist(&evidence, "index.json", &serde_json::to_vec_pretty(&json!({
            "schema_version":1,"origin":"live_connectivity_probe","source_commit":BASE,
            "codex_version":version_ref,"qualitygate_executable":binary,
            "prompt":prompt_ref,"response_schema":schema_ref,"records":records,
            "budget":{"max_runs_per_model":1,"seconds_per_model":180,"stream_bytes":runner::MAX_OUTPUT_BYTES},
            "known_limits":["No repair, capability ranking, cross-product or business-benefit evidence",
                "Model aliases and server-side settings may change; actual serving model is not attested"]
        })).unwrap());
    }
    assert!(
        records.iter().all(|record| record["complete"] == true),
        "One or more probes are incomplete; inspect {}",
        evidence.display()
    );
}
