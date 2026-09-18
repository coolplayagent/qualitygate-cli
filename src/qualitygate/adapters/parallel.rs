//! Bounded CPU workers for independent snapshot facts; results keep input order.

use anyhow::{Result, bail};
use std::time::{Duration, Instant};

pub(super) fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

pub(super) fn map<T: Sync, R: Send>(
    items: &[T],
    deadline: Instant,
    operation: impl Fn(&T) -> Result<R> + Sync,
) -> Result<Vec<R>> {
    let jobs = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(4);
    map_with_jobs(items, jobs, deadline, operation)
}

fn map_with_jobs<T: Sync, R: Send>(
    items: &[T],
    jobs: usize,
    deadline: Instant,
    operation: impl Fn(&T) -> Result<R> + Sync,
) -> Result<Vec<R>> {
    if !(1..=4).contains(&jobs) {
        bail!("Rule analysis jobs must be 1..4");
    }
    let evaluate = |item: &T| {
        if Instant::now() >= deadline {
            bail!("Rule analysis exceeded 30 seconds");
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
                        .name("qualitygate-adapter".into())
                        .spawn_scoped(scope, move || {
                            chunk.iter().map(evaluate).collect::<Result<Vec<_>>>()
                        })?,
                );
            }
            // Join every worker before using any evidence, including on error.
            let batches: Vec<_> = workers
                .into_iter()
                .map(|worker| {
                    worker
                        .join()
                        .map_err(|_| anyhow::anyhow!("Rule analysis worker panicked"))
                        .and_then(|result| result)
                })
                .collect();
            let mut ordered = Vec::with_capacity(items.len());
            for batch in batches {
                ordered.extend(batch?);
            }
            Ok(ordered)
        })?
    };
    if Instant::now() >= deadline {
        bail!("Rule analysis exceeded 30 seconds");
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn workers_overlap_stay_bounded_and_preserve_order() {
        let barrier = Barrier::new(4);
        let active = AtomicUsize::new(0);
        let maximum = AtomicUsize::new(0);
        let items: Vec<_> = (0..64).collect();
        let results = map_with_jobs(&items, 4, deadline(), |item| {
            let count = active.fetch_add(1, Ordering::SeqCst) + 1;
            maximum.fetch_max(count, Ordering::SeqCst);
            if item % 16 == 0 {
                barrier.wait();
            }
            active.fetch_sub(1, Ordering::SeqCst);
            Ok(item * 2)
        })
        .unwrap();
        assert_eq!(
            results,
            items.iter().map(|item| item * 2).collect::<Vec<_>>()
        );
        assert_eq!(maximum.load(Ordering::SeqCst), 4);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn failures_and_expiration_return_no_partial_evidence() {
        let items: Vec<_> = (0..64).collect();
        for jobs in [1, 4] {
            let error = map_with_jobs(&items, jobs, deadline(), |item| {
                if *item == 3 || *item == 17 {
                    bail!("bad {item}");
                }
                Ok(*item)
            })
            .unwrap_err();
            assert_eq!(error.to_string(), "bad 3");
            assert!(map_with_jobs(&items, jobs, Instant::now(), |item| Ok(*item)).is_err());
        }
        assert!(map_with_jobs(&items, 0, deadline(), |item| Ok(*item)).is_err());
        assert!(map_with_jobs(&items, 5, deadline(), |item| Ok(*item)).is_err());
        assert!(
            map_with_jobs(&items, 4, deadline(), |item| {
                if *item == 4 {
                    panic!("fixture panic");
                }
                Ok(*item)
            })
            .unwrap_err()
            .to_string()
            .contains("panicked")
        );
    }

    #[test]
    fn serial_and_parallel_java_parsing_agree_on_many_files() {
        let sources: Vec<_> = (0..256)
            .map(|index| {
                format!(
                    "class Case{index} {{ @Test void should_work_when_ready() {{ check({index}); }} }}"
                )
            })
            .collect();
        let parse = |source: &String| {
            Ok(super::super::syntax::parse("Case.java", source.as_bytes())?
                .unwrap()
                .tests
                .into_iter()
                .map(|test| (test.symbol, test.body_digest))
                .collect::<Vec<_>>())
        };
        let serial_started = Instant::now();
        let serial = map_with_jobs(&sources, 1, deadline(), parse).unwrap();
        let serial_time = serial_started.elapsed();
        let parallel_started = Instant::now();
        let parallel = map_with_jobs(&sources, 4, deadline(), parse).unwrap();
        let parallel_time = parallel_started.elapsed();
        assert_eq!(serial, parallel);
        assert!(serial_time < Duration::from_secs(30));
        assert!(parallel_time < Duration::from_secs(30));
        eprintln!(
            "256 changed Java syntax files: serial={serial_time:?} parallel={parallel_time:?}; shared host, no speedup threshold"
        );
    }
}
