use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "cargo")]
#[structopt(
    setting = structopt::clap::AppSettings::UnifiedHelpMessage,
    setting = structopt::clap::AppSettings::DeriveDisplayOrder,
    setting = structopt::clap::AppSettings::DontCollapseArgsInUsage
)]
pub enum Command {
    #[structopt(name = "release")]
    #[structopt(
        setting = structopt::clap::AppSettings::UnifiedHelpMessage,
        setting = structopt::clap::AppSettings::DeriveDisplayOrder,
        setting = structopt::clap::AppSettings::DontCollapseArgsInUsage
    )]
    Release(ReleaseOpt),
    Completions(CompletionsOpt),
}

#[derive(Debug, StructOpt)]
pub struct ReleaseOpt {
    #[structopt(flatten)]
    pub manifest: clap_cargo::Manifest,

    #[structopt(flatten)]
    pub workspace: clap_cargo::Workspace,

    /// Release level or version: bumping specified version field or remove prerelease extensions by default. Possible level value: major, minor, patch, release, rc, beta, alpha or any valid semver version that is greater than current version
    #[structopt(default_value)]
    pub level_or_version: crate::version::TargetVersion,

    /// Semver metadata
    #[structopt(short = "m")]
    pub metadata: Option<String>,

    /// Custom config file
    #[structopt(short = "c", long = "config")]
    pub custom_config: Option<String>,

    /// Ignore implicit configuration files.
    #[structopt(long)]
    pub isolated: bool,

    #[structopt(flatten)]
    pub config: ConfigArgs,

    /// Token to use when uploading
    #[structopt(long)]
    pub token: Option<String>,

    /// Actually perform a release. Dry-run mode is the default
    #[structopt(short = "x", long)]
    pub execute: bool,

    /// Skip release confirmation and version preview
    #[structopt(long)]
    pub no_confirm: bool,

    /// The name of tag for the previous release.
    #[structopt(long)]
    pub prev_tag_name: Option<String>,

    #[structopt(flatten)]
    pub logging: Verbosity,
}

impl ReleaseOpt {
    pub fn dry_run(&self) -> bool {
        !self.execute
    }
}

#[derive(Debug, StructOpt)]
pub struct ConfigArgs {
    /// Sign both git commit and tag,
    #[structopt(long, overrides_with("no-sign"))]
    sign: bool,
    #[structopt(long, overrides_with("sign"), hidden(true))]
    no_sign: bool,

    /// Sign git commit
    #[structopt(long, overrides_with("no-sign-commit"))]
    sign_commit: bool,
    #[structopt(long, overrides_with("sign-commit"), hidden(true))]
    no_sign_commit: bool,

    /// Sign git tag
    #[structopt(long, overrides_with("no-sign-tag"))]
    sign_tag: bool,
    #[structopt(long, overrides_with("sign-tag"), hidden(true))]
    no_sign_tag: bool,

    /// Git remote to push
    #[structopt(long)]
    push_remote: Option<String>,

    /// Cargo registry to upload to
    #[structopt(long)]
    registry: Option<String>,

    #[structopt(long, overrides_with("no-publish"), hidden(true))]
    publish: bool,
    /// Do not run cargo publish on release
    #[structopt(long, overrides_with("publish"), visible_alias = "skip-publish")]
    no_publish: bool,

    #[structopt(long, overrides_with("no-push"), hidden(true))]
    push: bool,
    /// Do not run git push in the last step
    #[structopt(long, overrides_with("push"), visible_alias = "skip-push")]
    no_push: bool,

    #[structopt(long, overrides_with("no-tag"), hidden(true))]
    tag: bool,
    /// Do not create git tag
    #[structopt(long, overrides_with("tag"), visible_alias = "skip-tag")]
    no_tag: bool,

    #[structopt(long, overrides_with("no-verify"), hidden(true))]
    verify: bool,
    /// Don't verify the contents by building them
    #[structopt(long, overrides_with("verify"))]
    no_verify: bool,

    /// Specify how workspace dependencies on this crate should be handed.
    #[structopt(
        long,
        possible_values(&crate::config::DependentVersion::variants()),
        case_insensitive(true),
    )]
    dependent_version: Option<crate::config::DependentVersion>,

    /// Prefix of git tag, note that this will override default prefix based on sub-directory
    #[structopt(long)]
    tag_prefix: Option<String>,

    /// The name of the git tag.
    #[structopt(long)]
    tag_name: Option<String>,

    /// Pre-release identifier(s) to append to the next development version after release
    #[structopt(long)]
    dev_version_ext: Option<String>,

    /// Create dev version after release
    #[structopt(long, overrides_with("no-dev-version"))]
    dev_version: bool,
    #[structopt(long, overrides_with("dev-version"), hidden(true))]
    no_dev_version: bool,

    /// Provide a set of features that need to be enabled
    #[structopt(long)]
    features: Vec<String>,

    /// Enable all features via `all-features`. Overrides `features`
    #[structopt(long)]
    all_features: bool,
}

impl ConfigArgs {
    pub fn to_config(&self) -> crate::config::Config {
        crate::config::Config {
            sign_commit: resolve_bool_arg(self.sign_commit, self.no_sign_commit)
                .or_else(|| self.sign()),
            sign_tag: resolve_bool_arg(self.sign_tag, self.no_sign_tag).or_else(|| self.sign()),
            push_remote: self.push_remote.clone(),
            registry: self.registry.clone(),
            publish: resolve_bool_arg(self.publish, self.no_publish),
            verify: resolve_bool_arg(self.verify, self.no_verify),
            push: resolve_bool_arg(self.push, self.no_push),
            dev_version_ext: self.dev_version_ext.clone(),
            dev_version: resolve_bool_arg(self.dev_version, self.no_dev_version),
            tag_prefix: self.tag_prefix.clone(),
            tag_name: self.tag_name.clone(),
            tag: resolve_bool_arg(self.tag, self.no_tag),
            enable_features: (!self.features.is_empty()).then(|| self.features.clone()),
            enable_all_features: self.all_features.then(|| true),
            dependent_version: self.dependent_version,
            ..Default::default()
        }
    }

    fn sign(&self) -> Option<bool> {
        resolve_bool_arg(self.sign, self.no_sign)
    }
}

#[derive(StructOpt, Debug, Clone)]
pub struct Verbosity {
    /// Pass many times for less log output
    #[structopt(long, short = "q", parse(from_occurrences))]
    quiet: i8,

    /// Pass many times for more log output
    ///
    /// By default, it'll report info. Passing `-v` one time also prints
    /// warnings, `-vv` enables info logging, `-vvv` debug, and `-vvvv` trace.
    #[structopt(long, short = "v", parse(from_occurrences))]
    verbose: i8,
}

impl Verbosity {
    /// Get the log level.
    pub fn log_level(&self) -> log::Level {
        let verbosity = 2 - self.quiet + self.verbose;

        match verbosity {
            std::i8::MIN..=0 => log::Level::Error,
            1 => log::Level::Warn,
            2 => log::Level::Info,
            3 => log::Level::Debug,
            4..=std::i8::MAX => log::Level::Trace,
        }
    }
}

fn resolve_bool_arg(yes: bool, no: bool) -> Option<bool> {
    match (yes, no) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        (false, false) => None,
        (_, _) => unreachable!("StructOpt should make this impossible"),
    }
}

#[derive(StructOpt, Debug, Clone)]
pub struct CompletionsOpt {
    #[structopt(long, short = "s", possible_values = &clap::Shell::variants(), case_insensitive = true)]
    pub shell: clap::Shell,
}
