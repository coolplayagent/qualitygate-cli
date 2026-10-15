//! Standalone Rust process used by integration tests, not a production agent.
use std::{io::{Read, Write}, path::Path};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let mode = &args[1];
    let state = Path::new(&args[2]);
    let mut prompt = String::new();
    std::io::stdin().read_to_string(&mut prompt).unwrap();
    assert!(prompt.contains("Honor the current contract before delivery"));
    assert!(prompt.contains("Qualitygate agent contract v1"));
    let mut capture = std::fs::OpenOptions::new().create(true).append(true).open(state.with_extension("prompts")).unwrap();
    writeln!(capture, "{prompt}\n=== next launch ===").unwrap();
    match mode.as_str() {
        "repair" | "repair2" => std::fs::write("hello.txt", format!("{mode}\n")).unwrap(),
        "two-step" => {
            if state.exists() { std::fs::write("hello.txt", "repaired\n").unwrap(); }
            else { std::fs::write(state, "attempted").unwrap(); std::fs::write("hello.txt", "still wrong\r\n").unwrap(); }
        }
        "weaken-policy" => {
            std::fs::write("qualitygate.yaml", "schema_version: 1\nrulesets: []\n").unwrap();
            std::fs::write("hello.txt", "repaired\n").unwrap();
        }
        "replace" => println!("{{\"path\":\"src/lib.rs\",\"content\":\"pub fn repaired() {{}}\\n\"}}"),
        "forge" => println!("{{\"complete\":true,\"gate\":{{\"complete\":true,\"decision\":\"pass\"}},\"report_path\":\"old-report.json\"}}"),
        _ => panic!("Unknown fixture mode"),
    }
}
