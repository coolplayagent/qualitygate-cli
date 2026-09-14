//! Bounded blocking workers with deterministic ordered results and cooperative deadlines.

use anyhow::{Result, bail};
use std::time::{Duration, Instant};

pub(super) fn jobs() -> usize {
    std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(8)
}

pub(super) fn map<T: Sync, R: Send>(
    items: &[T],
    jobs: usize,
    deadline: Instant,
    operation: impl Fn(&T) -> Result<R> + Sync,
) -> Result<Vec<R>> {
    if !(1..=8).contains(&jobs) {
        bail!("Rule loading jobs must be 1..8");
    }
    let evaluate = |item: &T| {
        if Instant::now() >= deadline {
            bail!("Rule loading exceeded its deadline");
        }
        operation(item)
    };
    let results = if jobs == 1 || items.len() < 16 {
        items.iter().map(evaluate).collect::<Result<Vec<_>>>()?
    } else {
        std::thread::scope(|scope| -> Result<Vec<R>> {
            let mut workers = Vec::new();
            for chunk in items.chunks(items.len().div_ceil(jobs)) {
                let evaluate = &evaluate;
                workers.push(
                    std::thread::Builder::new()
                        .name("qualitygate-rules".into())
                        .spawn_scoped(scope, move || {
                            chunk.iter().map(evaluate).collect::<Result<Vec<_>>>()
                        })?,
                );
            }
            // Join every worker even if an earlier chunk failed. Never publish partial results.
            let batches: Vec<_> = workers
                .into_iter()
                .map(|worker| {
                    worker
                        .join()
                        .map_err(|_| anyhow::anyhow!("Rule loading worker panicked"))
                        .and_then(|result| result)
                })
                .collect();
            let mut results = Vec::with_capacity(items.len());
            for batch in batches {
                results.extend(batch?);
            }
            Ok(results)
        })?
    };
    if Instant::now() >= deadline {
        bail!("Rule loading exceeded its deadline");
    }
    Ok(results)
}

pub(super) fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn workers_overlap_within_bound_and_keep_input_order() {
        let barrier = Barrier::new(4);
        let active = AtomicUsize::new(0);
        let maximum = AtomicUsize::new(0);
        let values: Vec<_> = (0..64).collect();
        let observed = map(&values, 4, deadline(), |value| {
            let readers = active.fetch_add(1, Ordering::SeqCst) + 1;
            maximum.fetch_max(readers, Ordering::SeqCst);
            // Every worker's first item rendezvous proves actual simultaneous execution.
            if value % 16 == 0 {
                barrier.wait();
            }
            active.fetch_sub(1, Ordering::SeqCst);
            Ok(value * 2)
        })
        .unwrap();
        assert_eq!(
            observed,
            values.iter().map(|value| value * 2).collect::<Vec<_>>()
        );
        assert_eq!(maximum.load(Ordering::SeqCst), 4);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn errors_expiration_and_panics_cannot_return_partial_evidence() {
        let values: Vec<_> = (0..64).collect();
        for jobs in [1, 4] {
            let error = map(&values, jobs, deadline(), |value| {
                if *value == 3 || *value == 17 {
                    bail!("bad {value}");
                }
                Ok(*value)
            })
            .unwrap_err();
            assert_eq!(error.to_string(), "bad 3");
            assert!(map(&values, jobs, Instant::now(), |value| Ok(*value)).is_err());
        }
        assert!(map(&values, 0, deadline(), |value| Ok(*value)).is_err());
        assert!(map(&values, 9, deadline(), |value| Ok(*value)).is_err());
        assert!(
            map(&values, 4, deadline(), |value| {
                if *value == 4 {
                    panic!("fixture panic");
                }
                Ok(*value)
            })
            .unwrap_err()
            .to_string()
            .contains("panicked")
        );
        assert!(
            map(&[1], 1, Instant::now() + Duration::from_millis(1), |_| {
                std::thread::sleep(Duration::from_millis(5));
                Ok(1)
            })
            .is_err()
        );
    }
}
