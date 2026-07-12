use std::process::ExitCode;

const USAGE: &str = "usage: shot --scenario <name> --tick <n> [--tick <n> ...] \
(--out <path> | --hashes) [--scale <n>] [--tc-root <path>]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match shot::parse_args(&args) {
        Ok(config) => {
            // Real render/PNG behaviour lands in T1. For now, echo the parsed
            // config to stderr so the skeleton is observable end-to-end.
            eprintln!(
                "shot: scenario={} ticks={:?} out={:?} scale={} hashes={} tc_root={:?}",
                config.scenario,
                config.ticks,
                config.out,
                config.scale,
                config.hashes,
                config.tc_root,
            );
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("shot: {msg}");
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}
