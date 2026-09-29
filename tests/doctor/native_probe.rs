//! Small native producer compiled by the integration harness on each host.
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("failure") => std::process::exit(7),
        Some("record-failure") => {
            std::fs::write(std::env::args_os().nth(2).unwrap(), "executed").unwrap();
            std::process::exit(7);
        }
        Some("timeout-long") => std::thread::sleep(std::time::Duration::from_secs(60)),
        Some("overflow") => {
            use std::io::Write;
            std::io::stdout().lock().write_all(&vec![b'x'; 17 * 1024 * 1024]).unwrap();
        }
        _ => println!("fixture-version-1"),
    }
}
