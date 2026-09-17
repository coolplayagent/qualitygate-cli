use super::Manifest;
use serde_json::{Value, json};

pub(super) struct Audit {
    pub value: Value,
    pub evidence_complete: bool,
    pub compliant: bool,
}

pub(super) fn audit(manifest: &Manifest) -> Audit {
    let mut records = Vec::new();
    let mut starts = Vec::new();
    let mut missing = 0_usize;
    for observation in &manifest.observations {
        for attempt in &observation.attempts {
            match &attempt.execution_evidence {
                Some(evidence) => {
                    let capture = &evidence.capture;
                    if attempt.number == 1
                        && let Some(sequence) = observation.start_sequence
                    {
                        starts.push((
                            sequence,
                            capture.started_at_ms,
                            observation.assignment_id.as_str(),
                        ));
                    }
                    records.push(json!({
                        "assignment_id":observation.assignment_id,
                        "attempt_number":attempt.number,
                        "status":attempt.status,
                        "started_at_ms":capture.started_at_ms,
                        "ended_at_ms":capture.ended_at_ms,
                        "elapsed_ms":attempt.elapsed_ms,
                        "artifact_digest":evidence.artifact.digest
                    }));
                }
                None => {
                    missing += 1;
                    records.push(json!({
                        "assignment_id":observation.assignment_id,
                        "attempt_number":attempt.number,
                        "status":"missing",
                        "attempt_status":attempt.status
                    }));
                }
            }
        }
    }
    starts.sort_unstable_by_key(|(sequence, _, _)| *sequence);
    let deviations: Vec<_> = starts
        .windows(2)
        .filter(|pair| pair[1].1 <= pair[0].1)
        .map(|pair| {
            json!({
                "reason":"start_order_differs_from_sequence",
                "previous_assignment":pair[0].2,
                "assignment_id":pair[1].2,
                "previous_started_at_ms":pair[0].1,
                "started_at_ms":pair[1].1
            })
        })
        .collect();
    let unobserved = manifest.assignments.len() - manifest.observations.len();
    let evidence_complete = missing == 0
        && unobserved == 0
        && manifest.observations.iter().all(|observation| {
            !observation.attempts.is_empty() && observation.start_sequence.is_some()
        });
    let compliant = deviations.is_empty();
    let status = if !compliant {
        "deviated"
    } else if !evidence_complete {
        "incomplete"
    } else {
        "matched"
    };
    Audit {
        value: json!({
            "status":status,
            "missing_attempts":missing,
            "unobserved":unobserved,
            "records":records,
            "deviations":deviations,
            "note":"Archived harness times bind declarations and order, but do not authenticate clocks or process events"
        }),
        evidence_complete,
        compliant,
    }
}
