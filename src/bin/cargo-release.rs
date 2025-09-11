#![expect(clippy::exit, reason = "is binary")]
#![allow(clippy::comparison_chain)]

use clap::Parser;

use cargo_release::{error, steps};

fn main() {
    let res = run();
    exit(res)
}

fn run() -> Result<(), error::CliError> {
    let Command::Release(ref release_matches) = Command::parse();

    let mut builder = get_logging(release_matches.logging.log_level());
    builder.init();

    match &release_matches.step {
        Some(Step::Changes(config)) => config.run(),
        Some(Step::Version(config)) => config.run(),
        Some(Step::Replace(config)) => config.run(),
        Some(Step::Hook(config)) => config.run(),
        Some(Step::Commit(config)) => config.run(),
        Some(Step::Publish(config)) => config.run(),
        Some(Step::Owner(config)) => config.run(),
        Some(Step::Tag(config)) => config.run(),
        Some(Step::Push(config)) => config.run(),
        Some(Step::Config(config)) => config.run(),
        None => release_matches.release.run(),
    }
}

pub fn get_logging(level: log::Level) -> env_logger::Builder {
    let mut builder = env_logger::Builder::new();

    builder.filter(None, level.to_level_filter());
    builder.format_module_path(false);

    if level == log::LevelFilter::Trace || level == log::LevelFilter::Debug {
        builder.format_timestamp_secs();
    } else {
        builder.format_target(false);
        builder.format_timestamp(None);
    }

    builder
}

#[derive(Debug, Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
#[command(styles = clap_cargo::style::CLAP_STYLING)]
pub enum Command {
    #[command(name = "release")]
    #[command(about, author, version)]
    Release(ReleaseOpt),
}

/// Automated release for Rust crates
#[derive(Debug, Clone, clap::Args)]
#[command(subcommand_value_name = "STEP")]
#[command(subcommand_help_heading = "Steps")]
#[command(args_conflicts_with_subcommands = true)]
pub struct ReleaseOpt {
    #[command(flatten)]
    pub release: steps::release::ReleaseStep,

    #[command(flatten)]
    pub logging: Verbosity,

    #[command(subcommand)]
    pub step: Option<Step>,
}

#[derive(Clone, Debug, clap::Subcommand)]
pub enum Step {
    Changes(steps::changes::ChangesStep),
    Version(steps::version::VersionStep),
    Replace(steps::replace::ReplaceStep),
    Hook(steps::hook::HookStep),
    Commit(steps::commit::CommitStep),
    Publish(steps::publish::PublishStep),
    Owner(steps::owner::OwnerStep),
    Tag(steps::tag::TagStep),
    Push(steps::push::PushStep),
    Config(steps::config::ConfigStep),
}

#[derive(clap::Args, Debug, Clone)]
#[command(next_help_heading = None)]
pub struct Verbosity {
    /// Pass many times for less log output
    #[arg(long, short, action = clap::ArgAction::Count, global = true)]
    quiet: u8,

    /// Pass many times for more log output
    ///
    /// By default, it'll report info. Passing `-v` one time adds debug
    /// logs, `-vv` adds trace logs.
    #[arg(long, short, action = clap::ArgAction::Count, global = true)]
    verbose: u8,
}

impl Verbosity {
    /// Get the log level.
    pub fn log_level(&self) -> log::Level {
        let verbosity = 2 - (self.quiet as i8) + (self.verbose as i8);

        match verbosity {
            i8::MIN..=0 => log::Level::Error,
            1 => log::Level::Warn,
            2 => log::Level::Info,
            3 => log::Level::Debug,
            4..=i8::MAX => log::Level::Trace,
        }
    }
}

/// Report any error message and exit.
fn exit(result: Result<(), error::CliError>) -> ! {
    let code = error::report(result);
    std::process::exit(code)
}

#[test]
fn verify_app() {
    use clap::CommandFactory;
    Command::command().debug_assert();
}
