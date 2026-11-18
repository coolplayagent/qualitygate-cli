// Linux-only synthetic workloads. Build with the flags in the experiment record.
use std::error::Error;
use std::fs::{self, File};
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Output};

const ALLOCATION: usize = 4096;
const ITERATIONS: usize = 16;
const LIMIT: usize = 8;

#[inline(never)]
fn allocate(faulty: bool) {
    let buffer = std::hint::black_box(vec![1_u8; ALLOCATION]);
    if faulty {
        std::mem::forget(buffer);
    }
}

fn finish_request(faulty: bool) -> io::Result<()> {
    let file = File::open("/dev/null")?;
    if faulty {
        std::mem::forget(file);
    }
    Err(io::Error::from(io::ErrorKind::InvalidData))
}

fn open_fds() -> io::Result<usize> {
    fs::read_dir("/proc/self/fd")?.try_fold(0, |count, entry| entry.map(|_| count + 1))
}

fn child(scenario: &str, faulty: bool) -> Result<(), Box<dyn Error>> {
    match scenario {
        "crash" => {
            let field = std::hint::black_box(None::<usize>);
            if faulty {
                field.expect("required field missing");
            }
            if field.is_none() {
                println!("rejected");
            }
        }
        "heap" => allocate(faulty),
        "fd" => {
            let before = open_fds()?;
            for _ in 0..ITERATIONS {
                if finish_request(faulty).unwrap_err().kind() != io::ErrorKind::InvalidData {
                    return Err("request did not reach the rejection path".into());
                }
            }
            println!("{before} {}", open_fds()?);
        }
        "bounds" => {
            for size in [0, LIMIT - 1, LIMIT, LIMIT + 1, usize::MAX] {
                // The contract accepts nonempty batches up to LIMIT inclusive.
                let accepted = size > 0 && if faulty { size < LIMIT } else { size <= LIMIT };
                println!("{size} {accepted}");
            }
        }
        _ => return Err("unknown workload".into()),
    }
    Ok(())
}

fn observe(scenario: &str, variant: &str) -> io::Result<Output> {
    Command::new(std::env::current_exe()?)
        .args(["worker", scenario, variant])
        .env("LSAN_OPTIONS", "exitcode=23:detect_leaks=1")
        .output()
}

fn leak_found(output: &Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr);
    output.status.code() == Some(23)
        && stderr.contains("LeakSanitizer: detected memory leaks")
        && stderr.contains("4096 byte(s) leaked in 1 allocation(s)")
}

fn evaluate(scenario: &str, variant: &str) -> Result<bool, Box<dyn Error>> {
    if scenario == "heap" && !leak_found(&observe("heap", "faulty")?) {
        return Err("LeakSanitizer positive control missing".into());
    }
    let output = observe(scenario, variant)?;
    let stdout = std::str::from_utf8(&output.stdout)?.trim();
    eprintln!(
        "workload={scenario} status={} stdout={stdout}",
        output.status
    );
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    if !output.status.success() {
        return match scenario {
            "crash"
                if output.status.signal() == Some(6)
                    && String::from_utf8_lossy(&output.stderr)
                        .contains("required field missing") =>
            {
                Ok(false)
            }
            "heap" if leak_found(&output) => Ok(false),
            _ => Err("unexpected workload termination".into()),
        };
    }
    match scenario {
        "crash" => Ok(stdout == "rejected"),
        "heap" => Ok(true),
        "fd" => {
            let (before, after) = stdout.split_once(' ').ok_or("missing descriptor counts")?;
            Ok(before.parse::<usize>()? == after.parse::<usize>()?)
        }
        "bounds" => Ok(stdout == format!("0 false\n7 true\n8 true\n9 false\n{} false", usize::MAX)),
        _ => Err("unknown workload".into()),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--version"] => println!("native-resources fixture 1"),
        ["worker", scenario, variant @ ("fixed" | "faulty")] => {
            child(scenario, *variant == "faulty")?
        }
        [scenario @ ("crash" | "heap" | "fd" | "bounds")] => {
            let variant = fs::read_to_string("variant.txt")?;
            if !matches!(variant.trim(), "fixed" | "faulty") {
                return Err("invalid experiment variant".into());
            }
            let failures = usize::from(!evaluate(scenario, variant.trim())?);
            println!(
                r#"{{"issues":[],"tests":{{"executed":1,"failures":{failures},"skipped":0}}}}"#
            );
        }
        _ => return Err("expected a workload name".into()),
    }
    Ok(())
}
