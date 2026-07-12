use std::process::ExitCode;

const USAGE: &str = "usage: shot --scenario <name> --tick <n> [--tick <n> ...] \
(--out <path> | --hashes) [--scale <n>] [--tc-root <path>] [--hud]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = match shot::parse_args(&args) {
        Ok(config) => config,
        Err(msg) => {
            eprintln!("shot: {msg}");
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    match shot::run(&config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("shot: {msg}");
            ExitCode::FAILURE
        }
    }
}
