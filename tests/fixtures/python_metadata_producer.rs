//! Synthetic output fault injection; real pip acceptance remains in tests/python.rs.
use std::{fs, path::Path, process::Command};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version") {
        if args.iter().any(|arg| arg == "pip") {
            println!("pip 26.0.1 from fixture (python 3.12)");
        } else {
            println!("Python 3.12.3");
        }
        return;
    }
    let directory = Path::new("target/python/fixture.dist-info");
    fs::create_dir_all(directory).unwrap();
    let metadata = directory.join("METADATA");
    match fs::read_to_string("producer-mode").unwrap().as_str() {
        "fifo" => assert!(
            Command::new("mkfifo")
                .arg(&metadata)
                .status()
                .unwrap()
                .success()
        ),
        "socket" => {
            let _socket = std::os::unix::net::UnixListener::bind(&metadata).unwrap();
        }
        "directory" => fs::create_dir(&metadata).unwrap(),
        "oversized" => fs::File::create(&metadata)
            .unwrap()
            .set_len(2 * 1024 * 1024 + 1)
            .unwrap(),
        "regular" => fs::write(
            &metadata,
            "Metadata-Version: 2.4\nName: fixture\nVersion: 1.0\n\n",
        )
        .unwrap(),
        mode => panic!("Unknown fixture mode: {mode}"),
    }
    let root = std::env::current_dir().unwrap();
    let mut url = String::from("file://");
    for byte in root.to_str().unwrap().bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            url.push(char::from(byte));
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    let report = r#"{"version":"1","pip_version":"26.0.1","environment":{"implementation_name":"cpython","implementation_version":"3.12.3","os_name":"posix","platform_machine":"x86_64","platform_python_implementation":"CPython","platform_release":"6.0","platform_system":"Linux","platform_version":"fixture","python_full_version":"3.12.3","python_version":"3.12","sys_platform":"linux"},"install":[{"download_info":{"url":"ROOT_URL","dir_info":{}},"is_direct":true,"requested":true,"metadata":{"name":"fixture","version":"1.0"}}]}"#;
    fs::write("target/pip.json", report.replace("ROOT_URL", &url)).unwrap();
}
