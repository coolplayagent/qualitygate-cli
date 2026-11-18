// Bounded, deterministic workloads for reachable retention and retry amplification.
use std::collections::VecDeque;
use std::error::Error;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum End {
    Completed,
    Cancelled,
    Expired,
}

struct Worker {
    pending: VecDeque<Arc<Vec<u8>>>,
    history: Vec<End>,
    retained: Vec<Arc<Vec<u8>>>,
    faulty: bool,
}

impl Worker {
    fn finish(&mut self, outcome: End) {
        let payload = self.pending.pop_front().expect("queued payload");
        self.history.push(outcome);
        if self.faulty && outcome != End::Completed {
            self.retained.push(payload);
        }
    }
}

fn retention(faulty: bool) -> (usize, usize) {
    const BATCHES: usize = 8;
    let mut worker = Worker {
        pending: VecDeque::new(),
        history: Vec::new(),
        retained: Vec::new(),
        faulty,
    };
    let outcomes = [End::Completed, End::Cancelled, End::Expired];
    let mut failed_checks = 0;
    for _ in 0..BATCHES {
        for outcome in outcomes {
            let payload = Arc::new(vec![42; 256]);
            let observed = Arc::downgrade(&payload);
            worker.pending.push_back(payload);
            let queued = observed.strong_count() == 1;
            worker.finish(outcome);
            let released = observed.strong_count() == 0;
            failed_checks += usize::from(!queued || !released || !worker.pending.is_empty());
        }
    }
    let bytes = worker
        .retained
        .iter()
        .map(|payload| payload.len())
        .sum::<usize>();
    eprintln!(
        "terminal={} pending={} retained={} retained_payload_bytes={bytes}",
        worker.history.len(),
        worker.pending.len(),
        worker.retained.len(),
    );
    (BATCHES * outcomes.len(), failed_checks)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Reply {
    Success,
    Transient,
    Capacity,
    Permanent,
}

struct Backend<'a> {
    script: &'a [Reply],
    exhausted_reply: Reply,
    calls: usize,
}

impl Backend<'_> {
    fn call(&mut self) -> Reply {
        let result = self
            .script
            .get(self.calls)
            .copied()
            .unwrap_or(self.exhausted_reply);
        self.calls += 1;
        result
    }
}

const LIMIT: usize = 3;

fn forward(backend: &mut Backend<'_>, remaining: &mut usize, faulty: bool) -> Reply {
    while *remaining > 0 {
        *remaining -= 1;
        match backend.call() {
            Reply::Transient => {}
            Reply::Capacity if faulty => {}
            result => return result,
        }
    }
    Reply::Transient
}

fn send(backend: &mut Backend<'_>, faulty: bool) -> Reply {
    if !faulty {
        // This entry point owns the budget for all downstream calls.
        let mut remaining = LIMIT;
        return forward(backend, &mut remaining, false);
    }
    for _ in 0..LIMIT {
        let mut fresh_budget = LIMIT;
        let result = forward(backend, &mut fresh_budget, true);
        if result != Reply::Transient {
            return result;
        }
    }
    Reply::Transient
}

fn retry(faulty: bool) -> (usize, usize) {
    use Reply::*;
    let cases: &[(&str, &[Reply], Reply, Reply, usize)] = &[
        ("success", &[Success], Permanent, Success, 1),
        ("recovers", &[Transient, Success], Permanent, Success, 2),
        ("persistent-transient", &[], Transient, Transient, 3),
        ("capacity", &[], Capacity, Capacity, 1),
        ("permanent", &[], Permanent, Permanent, 1),
    ];
    let mut failures = 0;
    for &(label, script, exhausted_reply, expected, expected_calls) in cases {
        let mut backend = Backend {
            script,
            exhausted_reply,
            calls: 0,
        };
        let result = send(&mut backend, faulty);
        eprintln!("{label}: reply={result:?} calls={}", backend.calls);
        failures += usize::from(result != expected || backend.calls != expected_calls);
    }
    (cases.len(), failures)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let workload = args.next().ok_or("expected a workload")?;
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    if workload == "--version" {
        println!("retention-retry fixture 1");
        return Ok(());
    }
    let variant = std::fs::read_to_string("variant.txt")?;
    let faulty = match variant.trim() {
        "faulty" => true,
        "fixed" => false,
        _ => return Err("invalid experiment variant".into()),
    };
    let (executed, failures) = match workload.as_str() {
        "retention" => retention(faulty),
        "retry" => retry(faulty),
        _ => return Err("unknown workload".into()),
    };
    println!(
        r#"{{"issues":[],"tests":{{"executed":{executed},"failures":{failures},"skipped":0}}}}"#
    );
    Ok(())
}
