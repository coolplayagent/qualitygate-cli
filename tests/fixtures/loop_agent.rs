//! Deterministic loop fixture: consume injected input before repair or failure.
use std::io::Read;

fn main() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();
    assert!(!input.is_empty(), "The harness must supply its prompt");
    match std::env::args().nth(1).as_deref() {
        Some("restore") => {
            assert!(
                std::process::Command::new("git")
                    .args(["restore", "hello.txt"])
                    .stdin(std::process::Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
            println!("consumed {} prompt bytes", input.len());
        }
        Some("fail") => {
            eprintln!("controlled agent failure");
            std::process::exit(7);
        }
        mode => panic!("Unknown fixture mode: {mode:?}"),
    }
}
