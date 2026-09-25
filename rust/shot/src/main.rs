use std::process::ExitCode;

const USAGE: &str = "usage: shot (--scenario <name> | --scenario-path <file>) --tick <n> \
[--tick <n> ...] (--out <path> | --hashes) [--scale <n>] [--tc-root <path>] [--hud]
       shot --weapsel --scenario-path <settings scenario> --out <png> [--scale <n>] [--tc-root <path>]";

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
