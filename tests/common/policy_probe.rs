//! Standalone Rust child used by the bounded policy-evaluation integration harness.
use std::{path::PathBuf, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let mode = &args[1];
    let value = &args[2];
    let millis = || SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    println!("start {} {}", std::process::id(), millis());
    match mode.as_str() {
        "sleep" => std::thread::sleep(Duration::from_millis(value.parse().unwrap())),
        "barrier" => {
            let directory = PathBuf::from(value);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join(std::process::id().to_string()), b"arrived").unwrap();
            let start = Instant::now();
            while std::fs::read_dir(&directory).unwrap().count() < 2 {
                if start.elapsed() > Duration::from_secs(4) { std::process::exit(3); }
                std::thread::sleep(Duration::from_millis(10));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        "restore" => {
            let bytes = std::fs::read(value).unwrap();
            std::fs::write(value, b"temporarily modified").unwrap();
            std::fs::write(value, bytes).unwrap();
        }
        _ => std::process::exit(4),
    }
    println!("end {} {}", std::process::id(), millis());
}
