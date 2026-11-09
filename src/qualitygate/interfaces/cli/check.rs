//! Check command execution and rendering have a separate poll frame from dispatch.
use super::super::{doctor, render};
use super::{CheckArgs, Format, decision_json};
use crate::{
    application::{self, CheckOptions},
    config,
    snapshot::Selection,
};
use anyhow::{Context, Result, bail};
use std::path::PathBuf;

pub(super) async fn run(
    root: PathBuf,
    config: String,
    args: CheckArgs,
    format: Format,
    envelope: bool,
) -> Result<(String, u8)> {
    if envelope && format != Format::Json {
        bail!("--envelope requires --format json");
    }
    let root = root
        .canonicalize()
        .context("Repository root does not exist")?;
    let selection = if let Some(url) = args.mr {
        Selection::MergeRequest {
            url,
            api_base: args.mr_api_base,
        }
    } else if let Some(diff) = args.diff {
        doctor::diff_selection(&diff)?
    } else if args.staged {
        Selection::Staged
    } else if args.worktree {
        Selection::Worktree {
            base: args.base.unwrap_or_else(|| "HEAD".into()),
        }
    } else if let Some(path) = &args.path {
        Selection::Path {
            path: path.clone(),
            base: args.base.unwrap_or_else(|| "HEAD".into()),
        }
    } else {
        Selection::Worktree {
            base: args.base.unwrap_or_else(|| "HEAD".into()),
        }
    };
    let path_filter = if matches!(selection, Selection::Path { .. }) {
        None
    } else {
        args.path
    };
    let options = CheckOptions {
        root,
        config,
        selection,
        snapshot_options: crate::snapshot::CaptureOptions {
            path_filter,
            ..args.snapshot.options()
        },
        profile: args.profile,
        task: args.task,
        policy_ref: args.policy_ref,
        output_dir: args.output_dir,
        trust_store: args.trust_store,
        evidence_dir: args.evidence_dir,
    };
    if let Some(destination) = args.export_delivery {
        let export =
            application::delivery::check(options, args.expect_base.as_deref(), destination).await?;
        let code = export.report.gate.decision.exit_code();
        let output = tokio::task::spawn_blocking(move || {
            render::metadata(&serde_json::to_value(export)?, format)
        })
        .await??;
        return Ok((output, code));
    }
    let report =
        application::check_with_expected_base(options, args.expect_base.as_deref()).await?;
    let code = report.gate.decision.exit_code();
    if args.feedback {
        let max_bytes = args.feedback_max_bytes.unwrap_or(32768) as usize;
        if !envelope {
            let feedback = application::feedback::render(report, max_bytes, args.severity).await?;
            return Ok((feedback, code));
        }
        let (report, feedback) =
            application::feedback::render_retaining(report, max_bytes, args.severity).await?;
        let output = tokio::task::spawn_blocking(move || -> Result<String> {
            let payload: serde_json::Value = serde_json::from_str(&feedback)?;
            config::decision_schema::validate_feedback(&payload)?;
            let mut envelope = crate::domain::decision_envelope::DecisionEnvelope::from_feedback(
                &report, payload,
            )?;
            let mut output = serde_json::to_string(&envelope)?;
            if output.len() + 1 > max_bytes {
                envelope.compact_feedback()?;
                output = serde_json::to_string(&envelope)?;
            }
            config::decision_schema::validate_feedback(&envelope.payload)?;
            config::decision_schema::validate_decision(&serde_json::to_value(&envelope)?)?;
            if output.len() + 1 > max_bytes {
                bail!("Feedback envelope exceeds byte limit; inspect the retained full report");
            }
            Ok(output)
        })
        .await??;
        return Ok((output, code));
    }
    if envelope {
        let output = tokio::task::spawn_blocking(move || {
            let envelope = crate::domain::decision_envelope::DecisionEnvelope::from_check(&report)?;
            decision_json(&envelope)
        })
        .await??;
        return Ok((output, code));
    }
    let severity = args.severity;
    let output =
        tokio::task::spawn_blocking(move || render::report(report, format, severity)).await??;
    Ok((output, code))
}
