use clap::Parser;

use crate::ops::version;

#[derive(Debug, Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
pub enum Command {
    #[command(name = "release")]
    #[command(about, author, version)]
    Release(ReleaseOpt),
}

#[derive(Debug, Clone, clap::Args)]
#[command(help_template = "\
{before-help}{about-with-newline}
{usage-heading} {usage}

Arguments:
{positionals}

Options:
{options}

Steps:
{subcommands}{after-help}

(release steps broken out for custom behavior and/or recovering from failures)
")]
#[command(subcommand_value_name = "STEP")]
#[command(subcommand_help_heading = "Steps")]
#[command(args_conflicts_with_subcommands = true)]
pub struct ReleaseOpt {
    #[command(flatten)]
    pub manifest: clap_cargo::Manifest,

    #[command(flatten)]
    pub workspace: clap_cargo::Workspace,

    /// Release level or version: bumping specified version field or remove prerelease extensions by default. Possible level value: major, minor, patch, release, rc, beta, alpha or any valid semver version that is greater than current version
    #[arg(default_value_t)]
    pub level_or_version: version::TargetVersion,

    /// Semver metadata
    #[arg(short, long)]
    pub metadata: Option<String>,

    #[command(flatten)]
    pub config: crate::config::ConfigArgs,

    /// Actually perform a release. Dry-run mode is the default
    #[arg(short = 'x', long)]
    pub execute: bool,

    /// Skip release confirmation and version preview
    #[arg(long)]
    pub no_confirm: bool,

    /// The name of tag for the previous release.
    #[arg(long)]
    pub prev_tag_name: Option<String>,

    #[command(flatten)]
    pub logging: Verbosity,

    #[command(subcommand)]
    pub step: Option<Step>,
}

impl ReleaseOpt {
    pub fn dry_run(&self) -> bool {
        !self.execute
    }
}

#[derive(Clone, Debug, clap::Subcommand)]
pub enum Step {
    Version(crate::steps::version::VersionStep),
    Replace(crate::steps::replace::ReplaceStep),
    Publish(crate::steps::publish::PublishStep),
    Tag(crate::steps::tag::TagStep),
    Push(crate::steps::push::PushStep),
    Config(crate::steps::config::ConfigStep),
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

#[test]
fn verify_app() {
    use clap::CommandFactory;
    Command::command().debug_assert()
}
