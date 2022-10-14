#![allow(clippy::collapsible_if)]
#![allow(clippy::comparison_chain)]

use clap::Parser;

use cargo_release::*;

fn main() {
    let res = run();
    error::exit(res)
}

fn run() -> Result<(), error::ProcessError> {
    let args::Command::Release(ref release_matches) = args::Command::parse();

    let mut builder = get_logging(release_matches.logging.log_level());
    builder.init();

    match &release_matches.step {
        Some(args::Step::Version(config)) => config.run(),
        Some(args::Step::Replace(config)) => config.run(),
        Some(args::Step::Publish(config)) => config.run(),
        Some(args::Step::Tag(config)) => config.run(),
        Some(args::Step::Push(config)) => config.run(),
        Some(args::Step::Config(config)) => config.run(),
        None => steps::release::release_workspace(release_matches),
    }
}

pub fn get_logging(level: log::Level) -> env_logger::Builder {
    let mut builder = env_logger::Builder::new();

    builder.filter(None, level.to_level_filter());

    builder.format_timestamp_secs().format_module_path(false);

    builder
}
