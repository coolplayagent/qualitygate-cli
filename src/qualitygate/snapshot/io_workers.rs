//! Bounded parallel snapshot I/O with ordered results and a shared deadline.

use super::File;
use anyhow::{Result, bail};
use std::{collections::BTreeMap, time::Instant};

pub(super) fn map<R: Send>(
    files: &BTreeMap<String, File>,
    deadline: Instant,
    timeout: &str,
    operation: impl Fn(&str, &File) -> Result<R> + Sync,
) -> Result<Vec<R>> {
    if files.len() > super::MAX_FILES {
        bail!("Snapshot I/O exceeds the file-count limit");
    }
    let evaluate = |(name, file): &(&String, &File)| {
        if Instant::now() >= deadline {
            bail!("{timeout}");
        }
        operation(name, file)
    };
    let files: Vec<_> = files.iter().collect();
    let results = if files.len() < 64 {
        files.iter().map(evaluate).collect::<Result<Vec<_>>>()?
    } else {
        std::thread::scope(|scope| -> Result<Vec<R>> {
            let mut workers = Vec::new();
            for chunk in files.chunks(files.len().div_ceil(4)) {
                let evaluate = &evaluate;
                workers.push(
                    std::thread::Builder::new()
                        .name("qualitygate-snapshot-io".into())
                        .spawn_scoped(scope, move || {
                            chunk.iter().map(evaluate).collect::<Result<Vec<_>>>()
                        })?,
                );
            }
            // Finish every worker before propagating errors or dropping its workspace.
            let batches: Vec<_> = workers
                .into_iter()
                .map(|worker| {
                    worker
                        .join()
                        .map_err(|_| anyhow::anyhow!("Snapshot I/O worker panicked"))
                        .and_then(|result| result)
                })
                .collect();
            let mut results = Vec::with_capacity(files.len());
            for batch in batches {
                results.extend(batch?);
            }
            Ok(results)
        })?
    };
    if Instant::now() >= deadline {
        bail!("{timeout}");
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Barrier, time::Duration};

    #[test]
    fn workers_overlap_keep_order_and_never_return_partial_results() {
        let files = (0..64)
            .map(|index| {
                (
                    format!("{index:02}"),
                    File {
                        bytes: vec![],
                        executable: false,
                    },
                )
            })
            .collect();
        let deadline = || Instant::now() + Duration::from_secs(5);
        let barrier = Barrier::new(4);
        let result = map(&files, deadline(), "expired", |name, _| {
            let index = name.parse::<usize>()?;
            if index % 16 == 0 {
                barrier.wait();
            }
            Ok(index)
        })
        .unwrap();
        assert_eq!(result, (0..64).collect::<Vec<_>>());
        assert!(map(&files, Instant::now(), "expired", |_, _| Ok(())).is_err());
        assert!(
            map(&files, deadline(), "expired", |name, _| {
                if name == "17" {
                    bail!("unreadable input");
                }
                Ok(())
            })
            .is_err()
        );
        assert!(
            map(&files, deadline(), "expired", |name, _| {
                if name == "17" {
                    panic!("fixture worker failure");
                }
                Ok(())
            })
            .unwrap_err()
            .to_string()
            .contains("panicked")
        );
    }
}
