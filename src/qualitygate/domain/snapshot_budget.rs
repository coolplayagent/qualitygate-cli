//! Shared acquisition limits; independent of rule severity and policy scope.

pub const DEFAULT_FILE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;

pub fn default_max_file_mib() -> u32 {
    (DEFAULT_FILE_BYTES / (1024 * 1024)) as u32
}

#[derive(Debug, serde::Serialize)]
pub struct OversizedFile {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, serde::Serialize)]
pub struct Preflight {
    pub excluded_file_count: usize,
    pub excluded_paths: Vec<String>,
    pub scope: &'static str,
    pub head: String,
    pub complete: bool,
    pub default_max_file_bytes: usize,
    pub maximum_max_file_bytes: usize,
    pub oversized_file_count: usize,
    pub oversized_files: Vec<OversizedFile>,
    pub unsupported_entry_count: usize,
    pub unsupported_entries: Vec<String>,
    pub details_truncated: bool,
    pub recommended_max_file_mib: Option<u64>,
    pub next_steps: Vec<String>,
}

pub fn file_limit_message(path: &str, size: u64, limit: usize) -> String {
    let next = if size <= MAX_FILE_BYTES as u64 {
        format!(
            "retry with --snapshot-max-file-mib {}",
            size.div_ceil(1024 * 1024)
        )
    } else {
        "the supported single-file maximum is 8 MiB; explicitly exclude this resource or acquisition remains incomplete".into()
    };
    format!(
        "File exceeds {limit} bytes: {path} ({size} bytes); snapshot acquisition limit, not a rule violation; {next}. --diff and --path retain unexcluded snapshot inputs; reviewed policy exclude entries can omit unrelated resources"
    )
}
