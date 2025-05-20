use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::error::CargoResult;
use crate::ops::cargo;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    #[serde(skip)]
    pub is_workspace: bool,
    pub unstable: Unstable,
    pub allow_branch: Option<Vec<String>>,
    pub sign_commit: Option<bool>,
    pub sign_tag: Option<bool>,
    pub push_remote: Option<String>,
    pub registry: Option<String>,
    pub release: Option<bool>,
    pub publish: Option<bool>,
    pub verify: Option<bool>,
    pub owners: Option<Vec<String>>,
    pub push: Option<bool>,
    pub push_options: Option<Vec<String>>,
    pub shared_version: Option<SharedVersion>,
    pub consolidate_commits: Option<bool>,
    pub pre_release_commit_message: Option<String>,
    pub pre_release_replacements: Option<Vec<Replace>>,
    pub pre_release_hook: Option<Command>,
    pub tag_message: Option<String>,
    pub tag_prefix: Option<String>,
    pub tag_name: Option<String>,
    pub tag: Option<bool>,
    pub enable_features: Option<Vec<String>>,
    pub enable_all_features: Option<bool>,
    pub dependent_version: Option<DependentVersion>,
    pub metadata: Option<MetadataPolicy>,
    pub target: Option<String>,
    pub rate_limit: RateLimit,
    pub certs_source: Option<CertsSource>,
}

impl Config {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn from_defaults() -> Self {
        let empty = Config::new();
        Config {
            is_workspace: true,
            unstable: Unstable::from_defaults(),
            allow_branch: Some(
                empty
                    .allow_branch()
                    .map(|s| s.to_owned())
                    .collect::<Vec<String>>(),
            ),
            sign_commit: Some(empty.sign_commit()),
            sign_tag: Some(empty.sign_tag()),
            push_remote: Some(empty.push_remote().to_owned()),
            registry: empty.registry().map(|s| s.to_owned()),
            release: Some(empty.release()),
            publish: Some(empty.publish()),
            verify: Some(empty.verify()),
            owners: Some(empty.owners().to_vec()),
            push: Some(empty.push()),
            push_options: Some(
                empty
                    .push_options()
                    .map(|s| s.to_owned())
                    .collect::<Vec<String>>(),
            ),
            shared_version: empty
                .shared_version()
                .map(|s| SharedVersion::Name(s.to_owned())),
            consolidate_commits: Some(empty.consolidate_commits()),
            pre_release_commit_message: Some(empty.pre_release_commit_message().to_owned()),
            pre_release_replacements: Some(empty.pre_release_replacements().to_vec()),
            pre_release_hook: empty.pre_release_hook().cloned(),
            tag_message: Some(empty.tag_message().to_owned()),
            tag_prefix: None, // Skipping, its location dependent
            tag_name: Some(empty.tag_name().to_owned()),
            tag: Some(empty.tag()),
            enable_features: Some(empty.enable_features().to_vec()),
            enable_all_features: Some(empty.enable_all_features()),
            dependent_version: Some(empty.dependent_version()),
            metadata: Some(empty.metadata()),
            target: None,
            rate_limit: RateLimit::from_defaults(),
            certs_source: Some(empty.certs_source()),
        }
    }

    pub fn update(&mut self, source: &Config) {
        self.unstable.update(&source.unstable);
        if let Some(allow_branch) = source.allow_branch.as_deref() {
            self.allow_branch = Some(allow_branch.to_owned());
        }
        if let Some(sign_commit) = source.sign_commit {
            self.sign_commit = Some(sign_commit);
        }
        if let Some(sign_tag) = source.sign_tag {
            self.sign_tag = Some(sign_tag);
        }
        if let Some(push_remote) = source.push_remote.as_deref() {
            self.push_remote = Some(push_remote.to_owned());
        }
        if let Some(registry) = source.registry.as_deref() {
            self.registry = Some(registry.to_owned());
        }
        if let Some(release) = source.release {
            self.release = Some(release);
        }
        if let Some(publish) = source.publish {
            self.publish = Some(publish);
        }
        if let Some(verify) = source.verify {
            self.verify = Some(verify);
        }
        if let Some(owners) = source.owners.as_deref() {
            self.owners = Some(owners.to_owned());
        }
        if let Some(push) = source.push {
            self.push = Some(push);
        }
        if let Some(push_options) = source.push_options.as_deref() {
            self.push_options = Some(push_options.to_owned());
        }
        if let Some(shared_version) = source.shared_version.clone() {
            self.shared_version = Some(shared_version);
        }
        if let Some(consolidate_commits) = source.consolidate_commits {
            self.consolidate_commits = Some(consolidate_commits);
        }
        if let Some(pre_release_commit_message) = source.pre_release_commit_message.as_deref() {
            self.pre_release_commit_message = Some(pre_release_commit_message.to_owned());
        }
        if let Some(pre_release_replacements) = source.pre_release_replacements.as_deref() {
            self.pre_release_replacements = Some(pre_release_replacements.to_owned());
        }
        if let Some(pre_release_hook) = source.pre_release_hook.as_ref() {
            self.pre_release_hook = Some(pre_release_hook.to_owned());
        }
        if let Some(tag_message) = source.tag_message.as_deref() {
            self.tag_message = Some(tag_message.to_owned());
        }
        if let Some(tag_prefix) = source.tag_prefix.as_deref() {
            self.tag_prefix = Some(tag_prefix.to_owned());
        }
        if let Some(tag_name) = source.tag_name.as_deref() {
            self.tag_name = Some(tag_name.to_owned());
        }
        if let Some(tag) = source.tag {
            self.tag = Some(tag);
        }
        if let Some(enable_features) = source.enable_features.as_deref() {
            self.enable_features = Some(enable_features.to_owned());
        }
        if let Some(enable_all_features) = source.enable_all_features {
            self.enable_all_features = Some(enable_all_features);
        }
        if let Some(dependent_version) = source.dependent_version {
            self.dependent_version = Some(dependent_version);
        }
        if let Some(metadata) = source.metadata {
            self.metadata = Some(metadata);
        }
        if let Some(target) = source.target.as_deref() {
            self.target = Some(target.to_owned());
        }
        self.rate_limit.update(&source.rate_limit);
        if let Some(certs) = source.certs_source {
            self.certs_source = Some(certs);
        }
    }

    pub fn unstable(&self) -> &Unstable {
        &self.unstable
    }

    pub fn allow_branch(&self) -> impl Iterator<Item = &str> {
        self.allow_branch
            .as_deref()
            .map(|a| itertools::Either::Left(a.iter().map(|s| s.as_str())))
            .unwrap_or_else(|| itertools::Either::Right(IntoIterator::into_iter(["*", "!HEAD"])))
    }

    pub fn sign_commit(&self) -> bool {
        self.sign_commit.unwrap_or(false)
    }

    pub fn sign_tag(&self) -> bool {
        self.sign_tag.unwrap_or(false)
    }

    pub fn push_remote(&self) -> &str {
        self.push_remote.as_deref().unwrap_or("origin")
    }

    pub fn registry(&self) -> Option<&str> {
        self.registry.as_deref()
    }

    pub fn release(&self) -> bool {
        self.release.unwrap_or(true)
    }

    pub fn publish(&self) -> bool {
        self.publish.unwrap_or(true)
    }

    pub fn verify(&self) -> bool {
        self.verify.unwrap_or(true)
    }

    pub fn owners(&self) -> &[String] {
        self.owners.as_ref().map(|v| v.as_ref()).unwrap_or(&[])
    }

    pub fn push(&self) -> bool {
        self.push.unwrap_or(true)
    }

    pub fn push_options(&self) -> impl Iterator<Item = &str> {
        self.push_options
            .as_ref()
            .into_iter()
            .flat_map(|v| v.iter().map(|s| s.as_str()))
    }

    pub fn shared_version(&self) -> Option<&str> {
        self.shared_version.as_ref().and_then(|s| s.as_name())
    }

    pub fn consolidate_commits(&self) -> bool {
        self.consolidate_commits.unwrap_or(self.is_workspace)
    }

    pub fn pre_release_commit_message(&self) -> &str {
        self.pre_release_commit_message
            .as_deref()
            .unwrap_or_else(|| {
                if self.consolidate_commits() {
                    "chore: Release"
                } else {
                    "chore: Release {{crate_name}} version {{version}}"
                }
            })
    }

    pub fn pre_release_replacements(&self) -> &[Replace] {
        self.pre_release_replacements
            .as_ref()
            .map(|v| v.as_ref())
            .unwrap_or(&[])
    }

    pub fn pre_release_hook(&self) -> Option<&Command> {
        self.pre_release_hook.as_ref()
    }

    pub fn tag_message(&self) -> &str {
        self.tag_message
            .as_deref()
            .unwrap_or("chore: Release {{crate_name}} version {{version}}")
    }

    pub fn tag_prefix(&self, is_root: bool) -> &str {
        // crate_name as default tag prefix for multi-crate project
        self.tag_prefix
            .as_deref()
            .unwrap_or(if !is_root { "{{crate_name}}-" } else { "" })
    }

    pub fn tag_name(&self) -> &str {
        self.tag_name.as_deref().unwrap_or("{{prefix}}v{{version}}")
    }

    pub fn tag(&self) -> bool {
        self.tag.unwrap_or(true)
    }

    pub fn enable_features(&self) -> &[String] {
        self.enable_features
            .as_ref()
            .map(|v| v.as_ref())
            .unwrap_or(&[])
    }

    pub fn enable_all_features(&self) -> bool {
        self.enable_all_features.unwrap_or(false)
    }

    pub fn features(&self) -> cargo::Features {
        if self.enable_all_features() {
            cargo::Features::All
        } else {
            let features = self.enable_features();
            cargo::Features::Selective(features.to_owned())
        }
    }

    pub fn dependent_version(&self) -> DependentVersion {
        self.dependent_version.unwrap_or_default()
    }

    pub fn metadata(&self) -> MetadataPolicy {
        self.metadata.unwrap_or_default()
    }

    pub fn certs_source(&self) -> CertsSource {
        self.certs_source.unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
#[serde(rename_all = "kebab-case")]
pub struct Unstable {
    workspace_publish: Option<bool>,
}

impl Unstable {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn from_defaults() -> Self {
        let empty = Self::new();
        Self {
            workspace_publish: Some(empty.workspace_publish()),
        }
    }
    pub fn update(&mut self, source: &Self) {
        if let Some(workspace_publish) = source.workspace_publish {
            self.workspace_publish = Some(workspace_publish);
        }
    }

    pub fn workspace_publish(&self) -> bool {
        self.workspace_publish.unwrap_or(false)
    }
}

impl From<Vec<UnstableValues>> for Unstable {
    fn from(values: Vec<UnstableValues>) -> Self {
        let mut unstable = Unstable::new();
        for value in values {
            match value {
                UnstableValues::WorkspacePublish(value) => unstable.workspace_publish = Some(value),
            }
        }
        unstable
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replace {
    pub file: PathBuf,
    pub search: String,
    pub replace: String,
    pub min: Option<usize>,
    pub max: Option<usize>,
    pub exactly: Option<usize>,
    #[serde(default)]
    pub prerelease: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Command {
    Line(String),
    Args(Vec<String>),
}

impl Command {
    pub fn args(&self) -> Vec<&str> {
        match self {
            Command::Line(s) => vec![s.as_str()],
            Command::Args(a) => a.iter().map(|s| s.as_str()).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[value(rename_all = "kebab-case")]
#[derive(Default)]
pub enum DependentVersion {
    /// Always upgrade dependents
    #[default] // This is the safest option as its hard to test `Fix`
    Upgrade,
    /// Upgrade when the old version requirement no longer applies
    Fix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[value(rename_all = "kebab-case")]
#[derive(Default)]
pub enum CertsSource {
    /// Use certs from Mozilla's root certificate store.
    #[default]
    Webpki,
    /// Use certs from the system root certificate store.
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[value(rename_all = "kebab-case")]
#[derive(Default)]
pub enum MetadataPolicy {
    /// Apply when set, clear when not
    #[default]
    Optional,
    /// Error if not set
    Required,
    /// Never apply the set metadata
    Ignore,
    /// Keep the prior metadata if not set
    Persistent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
#[serde(rename_all = "kebab-case")]
pub enum SharedVersion {
    Enabled(bool),
    Name(String),
}

impl SharedVersion {
    pub const WORKSPACE: &'static str = "workspace";

    pub fn as_name(&self) -> Option<&str> {
        match self {
            SharedVersion::Enabled(true) => Some("default"),
            SharedVersion::Enabled(false) => None,
            SharedVersion::Name(name) => Some(name.as_str()),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CargoManifest {
    workspace: Option<CargoWorkspace>,
    package: Option<CargoPackage>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CargoWorkspace {
    package: Option<CargoWorkspacePackage>,
    metadata: Option<CargoMetadata>,
}

impl CargoWorkspace {
    fn into_config(self) -> Option<Config> {
        self.metadata?.release
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CargoWorkspacePackage {
    publish: Option<CargoPublishField>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CargoPackage {
    publish: Option<MaybeWorkspace<CargoPublishField>>,
    version: Option<MaybeWorkspace<String>>,
    metadata: Option<CargoMetadata>,
}

impl CargoPackage {
    fn into_config(self) -> Option<Config> {
        self.metadata?.release
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum CargoPublishField {
    Bool(bool),
    Registries(Vec<String>),
}

impl CargoPublishField {
    fn publishable(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Registries(r) => !r.is_empty(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MaybeWorkspace<T> {
    Workspace(TomlWorkspaceField),
    Defined(T),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TomlWorkspaceField {
    workspace: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CargoMetadata {
    release: Option<Config>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RateLimit {
    #[serde(default)]
    pub new_packages: Option<usize>,
    #[serde(default)]
    pub existing_packages: Option<usize>,
}

impl RateLimit {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn from_defaults() -> Self {
        Self {
            new_packages: Some(5),
            existing_packages: Some(30),
        }
    }

    pub fn update(&mut self, source: &RateLimit) {
        if source.new_packages.is_some() {
            self.new_packages = source.new_packages;
        }
        if source.existing_packages.is_some() {
            self.existing_packages = source.existing_packages;
        }
    }

    pub fn new_packages(&self) -> usize {
        self.new_packages.unwrap_or(5)
    }

    pub fn existing_packages(&self) -> usize {
        self.existing_packages.unwrap_or(30)
    }
}

pub fn load_workspace_config(
    args: &ConfigArgs,
    ws_meta: &cargo_metadata::Metadata,
) -> CargoResult<Config> {
    let mut release_config = Config {
        is_workspace: 1 < ws_meta.workspace_members.len(),
        ..Default::default()
    };

    if !args.isolated {
        let is_workspace = 1 < ws_meta.workspace_members.len();
        let cfg = if is_workspace {
            resolve_workspace_config(ws_meta.workspace_root.as_std_path())?
        } else {
            // Outside of workspaces, go ahead and treat package config as workspace config so
            // users don't have to specially configure workspace-specific fields
            let pkg = ws_meta
                .packages
                .iter()
                .find(|p| ws_meta.workspace_members.contains(&p.id))
                .unwrap();
            resolve_config(
                ws_meta.workspace_root.as_std_path(),
                pkg.manifest_path.as_std_path(),
            )?
        };
        release_config.update(&cfg);
    }

    if let Some(custom_config_path) = args.custom_config.as_ref() {
        // when calling with -c option
        let cfg = resolve_custom_config(custom_config_path.as_ref())?.unwrap_or_default();
        release_config.update(&cfg);
    }

    release_config.update(&args.to_config());
    Ok(release_config)
}

pub fn load_package_config(
    args: &ConfigArgs,
    ws_meta: &cargo_metadata::Metadata,
    pkg: &cargo_metadata::Package,
) -> CargoResult<Config> {
    let manifest_path = pkg.manifest_path.as_std_path();

    let is_workspace = 1 < ws_meta.workspace_members.len();
    let mut release_config = Config {
        is_workspace,
        ..Default::default()
    };

    if !args.isolated {
        let cfg = resolve_config(ws_meta.workspace_root.as_std_path(), manifest_path)?;
        release_config.update(&cfg);
    }

    if let Some(custom_config_path) = args.custom_config.as_ref() {
        // when calling with -c option
        let cfg = resolve_custom_config(Path::new(custom_config_path))?.unwrap_or_default();
        release_config.update(&cfg);
    }

    release_config.update(&args.to_config());

    let overrides = resolve_overrides(ws_meta.workspace_root.as_std_path(), manifest_path)?;
    release_config.update(&overrides);

    Ok(release_config)
}

#[derive(Clone, Default, Debug, clap::Args)]
pub struct ConfigArgs {
    /// Custom config file
    #[arg(short, long = "config", value_name = "PATH")]
    pub custom_config: Option<PathBuf>,

    /// Ignore implicit configuration files.
    #[arg(long)]
    pub isolated: bool,

    /// Unstable options
    #[arg(short = 'Z', value_name = "FEATURE")]
    pub z: Vec<UnstableValues>,

    /// Sign both git commit and tag
    #[arg(long, overrides_with("no_sign"))]
    pub sign: bool,
    #[arg(long, overrides_with("sign"), hide(true))]
    pub no_sign: bool,

    /// Specify how workspace dependencies on this crate should be handed.
    #[arg(long, value_name = "ACTION", value_enum)]
    pub dependent_version: Option<DependentVersion>,

    /// Comma-separated globs of branch names a release can happen from
    #[arg(long, value_delimiter = ',', value_name = "GLOB[,...]")]
    pub allow_branch: Option<Vec<String>>,

    /// Indicate what certificate store to use for web requests.
    #[arg(long)]
    pub certs_source: Option<CertsSource>,

    #[command(flatten)]
    pub commit: CommitArgs,

    #[command(flatten)]
    pub publish: PublishArgs,

    #[command(flatten)]
    pub tag: TagArgs,

    #[command(flatten)]
    pub push: PushArgs,
}

impl ConfigArgs {
    pub fn to_config(&self) -> Config {
        let mut config = Config {
            unstable: Unstable::from(self.z.clone()),
            allow_branch: self.allow_branch.clone(),
            sign_commit: self.sign(),
            sign_tag: self.sign(),
            dependent_version: self.dependent_version,
            certs_source: self.certs_source,
            ..Default::default()
        };
        config.update(&self.commit.to_config());
        config.update(&self.publish.to_config());
        config.update(&self.tag.to_config());
        config.update(&self.push.to_config());
        config
    }

    fn sign(&self) -> Option<bool> {
        resolve_bool_arg(self.sign, self.no_sign)
    }
}

#[derive(Clone, Debug)]
pub enum UnstableValues {
    WorkspacePublish(bool),
}

impl std::str::FromStr for UnstableValues {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (name, mut value) = value.split_once('=').unwrap_or((value, ""));
        match name {
            "workspace-publish" => {
                if value.is_empty() {
                    value = "true";
                }
                let value = match value {
                    "true" => true,
                    "false" => false,
                    _ => anyhow::bail!(
                        "unsupported value `{name}={value}`, expected one of `true`, `false`"
                    ),
                };
                Ok(UnstableValues::WorkspacePublish(value))
            }
            _ => {
                anyhow::bail!("unsupported unstable feature name `{name}` (value `{value}`)");
            }
        }
    }
}

impl std::fmt::Display for UnstableValues {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::WorkspacePublish(true) => "workspace-publish".fmt(fmt),
            Self::WorkspacePublish(false) => "".fmt(fmt),
        }
    }
}

#[derive(Clone, Default, Debug, clap::Args)]
#[command(next_help_heading = "Commit")]
pub struct CommitArgs {
    /// Sign git commit
    #[arg(long, overrides_with("no_sign_commit"))]
    pub sign_commit: bool,
    #[arg(long, overrides_with("sign_commit"), hide(true))]
    pub no_sign_commit: bool,
}

impl CommitArgs {
    pub fn to_config(&self) -> Config {
        Config {
            sign_commit: resolve_bool_arg(self.sign_commit, self.no_sign_commit),
            ..Default::default()
        }
    }
}

#[derive(Clone, Default, Debug, clap::Args)]
#[command(next_help_heading = "Publish")]
pub struct PublishArgs {
    #[arg(long, overrides_with("no_publish"), hide(true))]
    publish: bool,
    /// Do not run cargo publish on release
    #[arg(long, overrides_with("publish"))]
    no_publish: bool,

    /// Cargo registry to upload to
    #[arg(long, value_name = "NAME")]
    registry: Option<String>,

    #[arg(long, overrides_with("no_verify"), hide(true))]
    verify: bool,
    /// Don't verify the contents by building them
    #[arg(long, overrides_with("verify"))]
    no_verify: bool,

    /// Provide a set of features that need to be enabled
    #[arg(long)]
    features: Vec<String>,

    /// Enable all features via `all-features`. Overrides `features`
    #[arg(long)]
    all_features: bool,

    /// Build for the target triple
    #[arg(long, value_name = "TRIPLE")]
    target: Option<String>,
}

impl PublishArgs {
    pub fn to_config(&self) -> Config {
        Config {
            publish: resolve_bool_arg(self.publish, self.no_publish),
            registry: self.registry.clone(),
            verify: resolve_bool_arg(self.verify, self.no_verify),
            enable_features: (!self.features.is_empty()).then(|| self.features.clone()),
            enable_all_features: self.all_features.then_some(true),
            target: self.target.clone(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Default, Debug, clap::Args)]
#[command(next_help_heading = "Tag")]
pub struct TagArgs {
    #[arg(long, overrides_with("no_tag"), hide(true))]
    tag: bool,
    /// Do not create git tag
    #[arg(long, overrides_with("tag"))]
    no_tag: bool,

    /// Sign git tag
    #[arg(long, overrides_with("no_sign_tag"))]
    sign_tag: bool,
    #[arg(long, overrides_with("sign_tag"), hide(true))]
    no_sign_tag: bool,

    /// Prefix of git tag, note that this will override default prefix based on sub-directory
    #[arg(long, value_name = "PREFIX")]
    tag_prefix: Option<String>,

    /// The name of the git tag.
    #[arg(long, value_name = "NAME")]
    tag_name: Option<String>,
}

impl TagArgs {
    pub fn to_config(&self) -> Config {
        Config {
            tag: resolve_bool_arg(self.tag, self.no_tag),
            sign_tag: resolve_bool_arg(self.sign_tag, self.no_sign_tag),
            tag_prefix: self.tag_prefix.clone(),
            tag_name: self.tag_name.clone(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Default, Debug, clap::Args)]
#[command(next_help_heading = "Push")]
pub struct PushArgs {
    #[arg(long, overrides_with("no_push"), hide(true))]
    push: bool,
    /// Do not run git push in the last step
    #[arg(long, overrides_with("push"))]
    no_push: bool,

    /// Git remote to push
    #[arg(long, value_name = "NAME")]
    push_remote: Option<String>,
}

impl PushArgs {
    pub fn to_config(&self) -> Config {
        Config {
            push: resolve_bool_arg(self.push, self.no_push),
            push_remote: self.push_remote.clone(),
            ..Default::default()
        }
    }
}

fn get_pkg_config_from_manifest(manifest_path: &Path) -> CargoResult<Option<Config>> {
    if manifest_path.exists() {
        let m = std::fs::read_to_string(manifest_path)?;
        let c: CargoManifest = toml::from_str(&m)
            .with_context(|| format!("Failed to parse `{}`", manifest_path.display()))?;

        Ok(c.package.and_then(|p| p.into_config()))
    } else {
        Ok(None)
    }
}

fn get_ws_config_from_manifest(manifest_path: &Path) -> CargoResult<Option<Config>> {
    if manifest_path.exists() {
        let m = std::fs::read_to_string(manifest_path)?;
        let c: CargoManifest = toml::from_str(&m)
            .with_context(|| format!("Failed to parse `{}`", manifest_path.display()))?;

        Ok(c.workspace.and_then(|p| p.into_config()))
    } else {
        Ok(None)
    }
}

fn get_config_from_file(file_path: &Path) -> CargoResult<Option<Config>> {
    if file_path.exists() {
        let c = std::fs::read_to_string(file_path)?;
        let config = toml::from_str(&c)
            .with_context(|| format!("Failed to parse `{}`", file_path.display()))?;
        Ok(Some(config))
    } else {
        Ok(None)
    }
}

pub fn resolve_custom_config(file_path: &Path) -> CargoResult<Option<Config>> {
    get_config_from_file(file_path)
}

/// Try to resolve workspace configuration source.
///
/// This tries the following sources in order, merging the results:
/// 1. $HOME/.release.toml
/// 2. $HOME/.config/cargo-release/release.toml
/// 3. $(workspace)/release.toml
/// 3. $(workspace)/Cargo.toml
pub fn resolve_workspace_config(workspace_root: &Path) -> CargoResult<Config> {
    let mut config = Config::default();

    // User-local configuration from home directory.
    let home_dir = dirs_next::home_dir();
    if let Some(mut home) = home_dir {
        home.push(".release.toml");
        if let Some(cfg) = get_config_from_file(&home)? {
            config.update(&cfg);
        }
    };

    let config_dir = dirs_next::config_dir();
    if let Some(mut config_path) = config_dir {
        config_path.push("cargo-release/release.toml");
        if let Some(cfg) = get_config_from_file(&config_path)? {
            config.update(&cfg);
        }
    };

    // Workspace config
    let default_config = workspace_root.join("release.toml");
    let current_dir_config = get_config_from_file(&default_config)?;
    if let Some(cfg) = current_dir_config {
        config.update(&cfg);
    };

    let manifest_path = workspace_root.join("Cargo.toml");
    let current_dir_config = get_ws_config_from_manifest(&manifest_path)?;
    if let Some(cfg) = current_dir_config {
        config.update(&cfg);
    };

    Ok(config)
}

/// Try to resolve configuration source.
///
/// This tries the following sources in order, merging the results:
/// 1. $HOME/.release.toml
/// 2. $HOME/.config/cargo-release/release.toml
/// 3. $(workspace)/release.toml
/// 3. $(workspace)/Cargo.toml `workspace.metadata.release`
/// 4. $(crate)/release.toml
/// 5. $(crate)/Cargo.toml `package.metadata.release`
///
/// `$(crate)/Cargo.toml` is a way to differentiate configuration for the root crate and the
/// workspace.
pub fn resolve_config(workspace_root: &Path, manifest_path: &Path) -> CargoResult<Config> {
    let mut config = resolve_workspace_config(workspace_root)?;

    // Crate config
    let crate_root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let default_config = crate_root.join("release.toml");
    let current_dir_config = get_config_from_file(&default_config)?;
    if let Some(cfg) = current_dir_config {
        config.update(&cfg);
    };

    let current_dir_config = get_pkg_config_from_manifest(manifest_path)?;
    if let Some(cfg) = current_dir_config {
        config.update(&cfg);
    };

    Ok(config)
}

pub fn resolve_overrides(workspace_root: &Path, manifest_path: &Path) -> CargoResult<Config> {
    fn load_workspace<'m, 'c: 'm>(
        workspace_root: &Path,
        workspace_cache: &'c mut Option<CargoManifest>,
    ) -> CargoResult<&'m CargoManifest> {
        if workspace_cache.is_none() {
            let workspace_path = workspace_root.join("Cargo.toml");
            let toml = std::fs::read_to_string(&workspace_path)?;
            let manifest: CargoManifest = toml::from_str(&toml)
                .with_context(|| format!("Failed to parse `{}`", workspace_path.display()))?;

            *workspace_cache = Some(manifest);
        }
        Ok(workspace_cache.as_ref().unwrap())
    }

    let mut release_config = Config::default();

    let mut workspace_cache = None;
    // the publish flag in cargo file
    let manifest = std::fs::read_to_string(manifest_path)?;
    let manifest: CargoManifest = toml::from_str(&manifest)
        .with_context(|| format!("Failed to parse `{}`", manifest_path.display()))?;
    if let Some(package) = manifest.package.as_ref() {
        let publish = match package.publish.as_ref() {
            Some(MaybeWorkspace::Defined(publish)) => publish.publishable(),
            Some(MaybeWorkspace::Workspace(workspace)) => {
                if workspace.workspace {
                    let workspace = load_workspace(workspace_root, &mut workspace_cache)?;
                    workspace
                        .workspace
                        .as_ref()
                        .and_then(|w| w.package.as_ref())
                        .and_then(|p| p.publish.as_ref())
                        .map(|p| p.publishable())
                        .unwrap_or(true)
                } else {
                    true
                }
            }
            None => true,
        };
        if !publish {
            release_config.publish = Some(false);
        }

        if package.version.is_none() {
            // No point releasing if it can't be published and doesn't have a version to update
            release_config.release = Some(false);
        }
        if package
            .version
            .as_ref()
            .and_then(|v| match v {
                MaybeWorkspace::Defined(_) => None,
                MaybeWorkspace::Workspace(workspace) => Some(workspace.workspace),
            })
            .unwrap_or(false)
        {
            release_config.shared_version =
                Some(SharedVersion::Name(SharedVersion::WORKSPACE.to_owned()));
            // We can't isolate commits because by changing the version in one crate, we change it in all
            release_config.consolidate_commits = Some(true);
        }
    }

    Ok(release_config)
}

fn resolve_bool_arg(yes: bool, no: bool) -> Option<bool> {
    match (yes, no) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        (false, false) => None,
        (_, _) => unreachable!("clap should make this impossible"),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    mod resolve_config {
        use super::*;

        #[test]
        fn doesnt_panic() {
            let release_config = resolve_config(Path::new("."), Path::new("Cargo.toml")).unwrap();
            assert!(!release_config.sign_commit());
        }
    }
}
