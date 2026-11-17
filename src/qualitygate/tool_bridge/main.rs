fn main() -> std::process::ExitCode {
    emit(qualitygate::tool_bridge::cli::entry())
}

fn emit(output: qualitygate::tool_bridge::cli::Output) -> std::process::ExitCode {
    if output.stderr {
        eprint!("{}", output.text);
    } else {
        println!("{}", output.text);
    }
    std::process::ExitCode::from(output.code)
}

#[cfg(test)]
mod tests {
    #[test]
    fn process_output_preserves_success_and_incomplete_exit_codes() {
        for (stderr, code) in [(false, 0), (true, 2)] {
            assert_eq!(
                super::emit(qualitygate::tool_bridge::cli::Output {
                    text: String::new(),
                    stderr,
                    code,
                }),
                std::process::ExitCode::from(code)
            );
        }
    }
}
