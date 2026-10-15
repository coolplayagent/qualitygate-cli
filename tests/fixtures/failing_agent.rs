//! Deterministic failure producer: consume injected input before exiting.
use std::io::Read;

fn main() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();
    assert!(!input.is_empty(), "The harness must supply its prompt");
    eprintln!("controlled agent failure");
    std::process::exit(7);
}
