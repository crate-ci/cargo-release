use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::FatalError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub allow_branch: Option<Vec<String>>,
    pub sign_commit: Option<bool>,
    pub sign_tag: Option<bool>,
    pub push_remote: Option<String>,
    pub registry: Option<String>,
    pub release: Option<bool>,
    pub disable_release: Option<bool>,
    pub publish: Option<bool>,
    pub disable_publish: Option<bool>,
    pub rust_version: Option<String>,
    pub verify: Option<bool>,
    pub no_verify: Option<bool>,
    pub push: Option<bool>,
    pub disable_push: Option<bool>,
    pub push_options: Option<Vec<String>>,
    pub dev_version_ext: Option<String>,
    pub dev_version: Option<bool>,
    pub no_dev_version: Option<bool>,
    pub shared_version: Option<bool>,
    pub consolidate_commits: Option<bool>,
    pub consolidate_pushes: Option<bool>,
    pub pre_release_commit_message: Option<String>,
    pub post_release_commit_message: Option<String>,
    pub pre_release_replacements: Option<Vec<Replace>>,
    pub post_release_replacements: Option<Vec<Replace>>,
    pub pre_release_hook: Option<Command>,
    pub tag_message: Option<String>,
    pub tag_prefix: Option<String>,
    pub tag_name: Option<String>,
    pub tag: Option<bool>,
    pub disable_tag: Option<bool>,
    pub enable_features: Option<Vec<String>>,
    pub enable_all_features: Option<bool>,
    pub dependent_version: Option<DependentVersion>,
}

impl Config {
    pub fn update(&mut self, source: &Config) {
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
        if let Some(release) = resolve_bool_arg(source.release, source.disable_release) {
            self.release = Some(release);
            self.disable_release = None;
        }
        if let Some(publish) = resolve_bool_arg(source.publish, source.disable_publish) {
            self.publish = Some(publish);
            self.disable_publish = None;
        }
        if let Some(rust_version) = source.rust_version.as_deref() {
            self.rust_version = Some(rust_version.to_owned());
        }
        if let Some(verify) = resolve_bool_arg(source.verify, source.no_verify) {
            self.verify = Some(verify);
            self.no_verify = None;
        }
        if let Some(push) = resolve_bool_arg(source.push, source.disable_push) {
            self.push = Some(push);
            self.disable_push = None;
        }
        if let Some(push_options) = source.push_options.as_deref() {
            self.push_options = Some(push_options.to_owned());
        }
        if let Some(dev_version_ext) = source.dev_version_ext.as_deref() {
            self.dev_version_ext = Some(dev_version_ext.to_owned());
        }
        if let Some(dev_version) = resolve_bool_arg(source.dev_version, source.no_dev_version) {
            self.dev_version = Some(dev_version);
            self.no_dev_version = None;
        }
        if let Some(shared_version) = source.shared_version {
            self.shared_version = Some(shared_version);
        }
        if let Some(consolidate_commits) = source.consolidate_commits {
            self.consolidate_commits = Some(consolidate_commits);
        }
        if let Some(consolidate_pushes) = source.consolidate_pushes {
            self.consolidate_pushes = Some(consolidate_pushes);
        }
        if let Some(pre_release_commit_message) = source.pre_release_commit_message.as_deref() {
            self.pre_release_commit_message = Some(pre_release_commit_message.to_owned());
        }
        if let Some(post_release_commit_message) = source.post_release_commit_message.as_deref() {
            self.post_release_commit_message = Some(post_release_commit_message.to_owned());
        }
        if let Some(pre_release_replacements) = source.pre_release_replacements.as_deref() {
            self.pre_release_replacements = Some(pre_release_replacements.to_owned());
        }
        if let Some(post_release_replacements) = source.post_release_replacements.as_deref() {
            self.post_release_replacements = Some(post_release_replacements.to_owned());
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
        if let Some(tag) = resolve_bool_arg(source.tag, source.disable_tag) {
            self.tag = Some(tag);
            self.disable_tag = None;
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
        resolve_bool_arg(self.release, self.disable_release).unwrap_or(true)
    }

    pub fn publish(&self) -> bool {
        resolve_bool_arg(self.publish, self.disable_publish).unwrap_or(true)
    }

    pub fn rust_version(&self) -> Option<&str> {
        self.rust_version.as_deref()
    }

    pub fn verify(&self) -> bool {
        resolve_bool_arg(self.verify, self.no_verify).unwrap_or(true)
    }

    pub fn push(&self) -> bool {
        resolve_bool_arg(self.push, self.disable_push).unwrap_or(true)
    }

    pub fn push_options(&self) -> &[String] {
        self.push_options
            .as_ref()
            .map(|v| v.as_ref())
            .unwrap_or(&[])
    }

    pub fn dev_version_ext(&self) -> &str {
        self.dev_version_ext.as_deref().unwrap_or("alpha.0")
    }

    pub fn dev_version(&self) -> bool {
        resolve_bool_arg(self.dev_version, self.no_dev_version).unwrap_or(false)
    }

    pub fn shared_version(&self) -> bool {
        self.shared_version.unwrap_or(false)
    }

    pub fn consolidate_commits(&self) -> bool {
        self.consolidate_commits.unwrap_or(false)
    }

    pub fn consolidate_pushes(&self) -> bool {
        self.consolidate_pushes.unwrap_or(false)
    }

    pub fn pre_release_commit_message(&self) -> &str {
        self.pre_release_commit_message
            .as_deref()
            .unwrap_or("(cargo-release) version {{version}}")
    }

    pub fn post_release_commit_message(&self) -> &str {
        self.post_release_commit_message
            .as_deref()
            .unwrap_or("(cargo-release) start next development iteration {{next_version}}")
    }

    pub fn pre_release_replacements(&self) -> &[Replace] {
        self.pre_release_replacements
            .as_ref()
            .map(|v| v.as_ref())
            .unwrap_or(&[])
    }

    pub fn post_release_replacements(&self) -> &[Replace] {
        self.post_release_replacements
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
            .unwrap_or("(cargo-release) {{crate_name}} version {{version}}")
    }

    pub fn tag_prefix(&self, is_root: bool) -> &str {
        // crate_name as default tag prefix for multi-crate project
        self.tag_prefix
            .as_deref()
            .unwrap_or_else(|| if !is_root { "{{crate_name}}-" } else { "" })
    }

    pub fn tag_name(&self) -> &str {
        self.tag_name.as_deref().unwrap_or("{{prefix}}v{{version}}")
    }

    pub fn tag(&self) -> bool {
        resolve_bool_arg(self.tag, self.disable_tag).unwrap_or(true)
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

    pub fn features(&self) -> crate::cargo::Features {
        if self.enable_all_features() {
            crate::cargo::Features::All
        } else {
            let features = self.enable_features();
            if features.is_empty() {
                crate::cargo::Features::None
            } else {
                crate::cargo::Features::Selective(features.to_owned())
            }
        }
    }

    pub fn dependent_version(&self) -> DependentVersion {
        self.dependent_version.unwrap_or_default()
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
            Command::Line(ref s) => vec![s.as_str()],
            Command::Args(ref a) => a.iter().map(|s| s.as_str()).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ArgEnum)]
#[serde(rename_all = "kebab-case")]
#[clap(rename_all = "kebab-case")]
pub enum DependentVersion {
    Upgrade,
    Fix,
    Error,
    Warn,
    Ignore,
}

impl Default for DependentVersion {
    fn default() -> Self {
        DependentVersion::Fix
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
    metadata: Option<CargoMetadata>,
}

impl CargoWorkspace {
    fn into_config(self) -> Option<Config> {
        self.metadata?.release
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CargoPackage {
    metadata: Option<CargoMetadata>,
}

impl CargoPackage {
    fn into_config(self) -> Option<Config> {
        self.metadata?.release
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CargoMetadata {
    release: Option<Config>,
}

pub fn dump_config(
    args: &crate::args::ReleaseOpt,
    output_path: &std::path::Path,
) -> Result<i32, FatalError> {
    log::trace!("Initializing");
    let ws_meta = args
        .manifest
        .metadata()
        // When evaluating dependency ordering, we need to consider optional depednencies
        .features(cargo_metadata::CargoOpt::AllFeatures)
        .exec()
        .map_err(FatalError::from)?;

    let release_config =
        if let Some(root_id) = ws_meta.resolve.as_ref().and_then(|r| r.root.as_ref()) {
            let pkg = ws_meta
                .packages
                .iter()
                .find(|p| p.id == *root_id)
                .expect("root should always be present");
            let manifest_path = pkg.manifest_path.as_std_path();

            let mut release_config = Config::default();

            if !args.isolated {
                let cfg = resolve_config(ws_meta.workspace_root.as_std_path(), manifest_path)?;
                release_config.update(&cfg);
            }

            if let Some(custom_config_path) = args.custom_config.as_ref() {
                // when calling with -c option
                let cfg = resolve_custom_config(Path::new(custom_config_path))?.unwrap_or_default();
                release_config.update(&cfg);
            }

            release_config.update(&args.config.to_config());

            // the publish flag in cargo file
            let cargo_file = crate::cargo::parse_cargo_config(manifest_path)?;
            if !cargo_file
                .get("package")
                .and_then(|f| f.as_table())
                .and_then(|f| f.get("publish"))
                .and_then(|f| f.as_bool())
                .unwrap_or(true)
            {
                release_config.publish = Some(false);
                release_config.disable_publish = None;
            }
            if release_config.rust_version() == Some("manifest") {
                if let Some(rust_version) = cargo_file
                    .get("package")
                    .and_then(|p| p.get("rust-version"))
                    .and_then(|r| r.as_str())
                {
                    release_config.rust_version = Some(rust_version.to_owned());
                } else {
                    release_config.rust_version = None;
                }
            }

            release_config
        } else {
            let mut release_config = Config::default();

            if !args.isolated {
                let cfg = resolve_workspace_config(ws_meta.workspace_root.as_std_path())?;
                release_config.update(&cfg);
            }

            if let Some(custom_config_path) = args.custom_config.as_ref() {
                // when calling with -c option
                let cfg = resolve_custom_config(Path::new(custom_config_path))?.unwrap_or_default();
                release_config.update(&cfg);
            }

            release_config.update(&args.config.to_config());
            release_config
        };

    let output = toml_edit::easy::to_string_pretty(&release_config)?;

    if output_path == std::path::Path::new("-") {
        std::io::stdout().write_all(output.as_bytes())?;
    } else {
        std::fs::write(output_path, &output)?;
    }

    Ok(0)
}

fn get_pkg_config_from_manifest(manifest_path: &Path) -> Result<Option<Config>, FatalError> {
    if manifest_path.exists() {
        let m = std::fs::read_to_string(manifest_path).map_err(FatalError::from)?;
        let c: CargoManifest = toml_edit::easy::from_str(&m).map_err(FatalError::from)?;

        Ok(c.package.and_then(|p| p.into_config()))
    } else {
        Ok(None)
    }
}

fn get_ws_config_from_manifest(manifest_path: &Path) -> Result<Option<Config>, FatalError> {
    if manifest_path.exists() {
        let m = std::fs::read_to_string(manifest_path).map_err(FatalError::from)?;
        let c: CargoManifest = toml_edit::easy::from_str(&m).map_err(FatalError::from)?;

        Ok(c.workspace.and_then(|p| p.into_config()))
    } else {
        Ok(None)
    }
}

fn get_config_from_file(file_path: &Path) -> Result<Option<Config>, FatalError> {
    if file_path.exists() {
        let c = std::fs::read_to_string(file_path).map_err(FatalError::from)?;
        let config = toml_edit::easy::from_str(&c).map_err(FatalError::from)?;
        Ok(Some(config))
    } else {
        Ok(None)
    }
}

pub fn resolve_custom_config(file_path: &Path) -> Result<Option<Config>, FatalError> {
    get_config_from_file(file_path)
}

/// Try to resolve workspace configuration source.
///
/// This tries the following sources in order, merging the results:
/// 1. $HOME/.release.toml
/// 2. $HOME/.config/cargo-release/release.toml
/// 3. $(workspace)/release.toml
/// 3. $(workspace)/Cargo.toml
pub fn resolve_workspace_config(workspace_root: &Path) -> Result<Config, FatalError> {
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
/// 2. $HOME/.config/release.toml
/// 3. $(workspace)/release.toml
/// 3. $(workspace)/Cargo.toml `workspace.metadata.release`
/// 4. $(crate)/release.toml
/// 5. $(crate)/Cargo.toml `package.metadata.release`
///
/// `$(crate)/Cargo.toml` is a way to differentiate configuration for the root crate and the
/// workspace.
pub fn resolve_config(workspace_root: &Path, manifest_path: &Path) -> Result<Config, FatalError> {
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

    let crate_root = manifest_path.parent().unwrap_or_else(|| Path::new("."));

    // Workspace config
    if crate_root != workspace_root {
        let default_config = workspace_root.join("release.toml");
        let current_dir_config = get_config_from_file(&default_config)?;
        if let Some(cfg) = current_dir_config {
            config.update(&cfg);
        };
    }

    let current_dir_config = get_ws_config_from_manifest(manifest_path)?;
    if let Some(cfg) = current_dir_config {
        config.update(&cfg);
    };

    // Crate config
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

fn resolve_bool_arg(yes: Option<bool>, no: Option<bool>) -> Option<bool> {
    match (yes, no) {
        (Some(_), Some(_)) => panic!("`disable_` flags are deprecated and can't be set at the same time with their positive versions"),
        (Some(yes), None) => Some(yes),
        (None, Some(no)) => {
            log::warn!("Negative config values are deprecated (`no_`, `disable_`)");
            Some(!no)
        },
        (None, None) => None,
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
