use clap::Parser;

#[derive(Debug, Parser)]
#[clap(name = "cargo")]
#[clap(bin_name = "cargo")]
#[clap(
    setting = clap::AppSettings::DeriveDisplayOrder,
    dont_collapse_args_in_usage = true,
)]
pub enum Command {
    #[clap(name = "release")]
    #[clap(about, author, version)]
    #[clap(
        setting = clap::AppSettings::DeriveDisplayOrder,
    )]
    Release(ReleaseOpt),
}

#[derive(Debug, Clone, clap::Args)]
pub struct ReleaseOpt {
    #[clap(flatten)]
    pub manifest: clap_cargo::Manifest,

    #[clap(flatten)]
    pub workspace: clap_cargo::Workspace,

    /// Release level or version: bumping specified version field or remove prerelease extensions by default. Possible level value: major, minor, patch, release, rc, beta, alpha or any valid semver version that is greater than current version
    #[clap(default_value_t)]
    pub level_or_version: crate::version::TargetVersion,

    /// Semver metadata
    #[clap(short, long)]
    pub metadata: Option<String>,

    /// Custom config file
    #[clap(short, long = "config")]
    pub custom_config: Option<String>,

    /// Ignore implicit configuration files.
    #[clap(long)]
    pub isolated: bool,

    #[clap(flatten)]
    pub config: ConfigArgs,

    /// Token to use when uploading
    #[clap(long)]
    pub token: Option<String>,

    /// Actually perform a release. Dry-run mode is the default
    #[clap(short = 'x', long)]
    pub execute: bool,

    /// Skip release confirmation and version preview
    #[clap(long)]
    pub no_confirm: bool,

    /// The name of tag for the previous release.
    #[clap(long)]
    pub prev_tag_name: Option<String>,

    /// Write the current configuration to file with `-` for stdout
    #[clap(long)]
    pub dump_config: Option<std::path::PathBuf>,

    #[clap(flatten)]
    pub logging: Verbosity,

    /// Allow dirty working directories to be released.
    #[clap(long)]
    pub allow_dirty: bool,
}

impl ReleaseOpt {
    pub fn dry_run(&self) -> bool {
        !self.execute
    }
}

#[derive(Debug, Clone, clap::Args)]
pub struct ConfigArgs {
    /// Sign both git commit and tag
    #[clap(long, overrides_with("no-sign"))]
    sign: bool,
    #[clap(long, overrides_with("sign"), hide(true))]
    no_sign: bool,

    /// Sign git commit
    #[clap(long, overrides_with("no-sign-commit"))]
    sign_commit: bool,
    #[clap(long, overrides_with("sign-commit"), hide(true))]
    no_sign_commit: bool,

    /// Sign git tag
    #[clap(long, overrides_with("no-sign-tag"))]
    sign_tag: bool,
    #[clap(long, overrides_with("sign-tag"), hide(true))]
    no_sign_tag: bool,

    /// Git remote to push
    #[clap(long)]
    push_remote: Option<String>,

    /// Cargo registry to upload to
    #[clap(long)]
    registry: Option<String>,

    #[clap(long, overrides_with("no-publish"), hide(true))]
    publish: bool,
    /// Do not run cargo publish on release
    #[clap(long, overrides_with("publish"))]
    no_publish: bool,

    #[clap(long, overrides_with("no-push"), hide(true))]
    push: bool,
    /// Do not run git push in the last step
    #[clap(long, overrides_with("push"))]
    no_push: bool,

    #[clap(long, overrides_with("no-tag"), hide(true))]
    tag: bool,
    /// Do not create git tag
    #[clap(long, overrides_with("tag"))]
    no_tag: bool,

    #[clap(long, overrides_with("no-verify"), hide(true))]
    verify: bool,
    /// Don't verify the contents by building them
    #[clap(long, overrides_with("verify"))]
    no_verify: bool,

    /// Specify how workspace dependencies on this crate should be handed.
    #[clap(long, arg_enum)]
    dependent_version: Option<crate::config::DependentVersion>,

    /// Prefix of git tag, note that this will override default prefix based on sub-directory
    #[clap(long)]
    tag_prefix: Option<String>,

    /// The name of the git tag.
    #[clap(long)]
    tag_name: Option<String>,

    /// Pre-release identifier(s) to append to the next development version after release
    #[clap(long)]
    dev_version_ext: Option<String>,

    /// Create dev version after release
    #[clap(long, overrides_with("no-dev-version"))]
    dev_version: bool,
    #[clap(long, overrides_with("dev-version"), hide(true))]
    no_dev_version: bool,

    /// Provide a set of features that need to be enabled
    #[clap(long)]
    features: Vec<String>,

    /// Enable all features via `all-features`. Overrides `features`
    #[clap(long)]
    all_features: bool,

    /// Build for the target triple
    #[clap(long)]
    target: Option<String>,

    /// Comma-separated globs of branch names a release can happen from
    #[clap(long, use_value_delimiter = true)]
    allow_branch: Option<Vec<String>>,
}

impl ConfigArgs {
    pub fn to_config(&self) -> crate::config::Config {
        crate::config::Config {
            allow_branch: self.allow_branch.clone(),
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
            target: self.target.clone(),
            ..Default::default()
        }
    }

    fn sign(&self) -> Option<bool> {
        resolve_bool_arg(self.sign, self.no_sign)
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct Verbosity {
    /// Pass many times for less log output
    #[clap(long, short, parse(from_occurrences))]
    quiet: i8,

    /// Pass many times for more log output
    ///
    /// By default, it'll report info. Passing `-v` one time adds debug
    /// logs, `-vv` adds trace logs.
    #[clap(long, short, parse(from_occurrences))]
    verbose: i8,
}

impl Verbosity {
    /// Get the log level.
    pub fn log_level(&self) -> log::Level {
        let verbosity = 2 - self.quiet + self.verbose;

        match verbosity {
            i8::MIN..=0 => log::Level::Error,
            1 => log::Level::Warn,
            2 => log::Level::Info,
            3 => log::Level::Debug,
            4..=i8::MAX => log::Level::Trace,
        }
    }
}

fn resolve_bool_arg(yes: bool, no: bool) -> Option<bool> {
    match (yes, no) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        (false, false) => None,
        (_, _) => unreachable!("clap should make this impossible"),
    }
}

#[test]
fn verify_app() {
    use clap::IntoApp;
    Command::command().debug_assert()
}
