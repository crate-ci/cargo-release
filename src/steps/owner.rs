use crate::error::CliError;
use crate::ops::git;
use crate::steps::plan;

/// Ensure owners are set on specified packages
#[derive(Debug, Clone, clap::Args)]
pub struct OwnerStep {
    #[command(flatten)]
    manifest: clap_cargo::Manifest,

    #[command(flatten)]
    workspace: clap_cargo::Workspace,

    /// Custom config file
    #[arg(short, long = "config", value_name = "PATH")]
    custom_config: Option<std::path::PathBuf>,

    /// Ignore implicit configuration files.
    #[arg(long)]
    isolated: bool,

    /// Unstable options
    #[arg(short = 'Z', value_name = "FEATURE")]
    z: Vec<crate::config::UnstableValues>,

    /// Comma-separated globs of branch names a release can happen from
    #[arg(long, value_delimiter = ',')]
    allow_branch: Option<Vec<String>>,

    /// Actually perform a release. Dry-run mode is the default
    #[arg(short = 'x', long)]
    execute: bool,

    #[arg(short = 'n', long, conflicts_with = "execute", hide = true)]
    dry_run: bool,

    /// Skip release confirmation and version preview
    #[arg(long)]
    no_confirm: bool,
}

impl OwnerStep {
    pub fn run(&self) -> Result<(), CliError> {
        git::git_version()?;

        if self.dry_run {
            let _ =
                crate::ops::shell::warn("`--dry-run` is superfluous, dry-run is done by default");
        }

        let ws_meta = self
            .manifest
            .metadata()
            // When evaluating dependency ordering, we need to consider optional dependencies
            .features(cargo_metadata::CargoOpt::AllFeatures)
            .exec()?;
        let config = self.to_config();
        let ws_config = crate::config::load_workspace_config(&config, &ws_meta)?;
        let mut pkgs = plan::load(&config, &ws_meta)?;

        let (_selected_pkgs, excluded_pkgs) = self.workspace.partition_packages(&ws_meta);
        for excluded_pkg in excluded_pkgs {
            let Some(pkg) = pkgs.get_mut(&excluded_pkg.id) else {
                // Either not in workspace or marked as `release = false`.
                continue;
            };
            if !pkg.config.release() {
                continue;
            }

            pkg.config.publish = Some(false);
            pkg.config.owners = Some(vec![]);
            pkg.config.release = Some(false);

            let crate_name = pkg.meta.name.as_str();
            log::debug!("disabled by user, skipping {crate_name}",);
        }

        let mut pkgs = plan::plan(pkgs)?;

        for pkg in pkgs.values_mut() {
            if pkg.config.owners().is_empty() {
                log::debug!("disabled due to no owners, skipping {}", pkg.meta.name);
                pkg.config.publish = Some(false);
                pkg.config.owners = Some(vec![]);
                pkg.config.release = Some(false);
            } else if !pkg.config.publish() {
                log::debug!("disabled due to publish=false, skipping {}", pkg.meta.name);
                pkg.config.publish = Some(false);
                pkg.config.owners = Some(vec![]);
                pkg.config.release = Some(false);
            }
        }

        let (selected_pkgs, _excluded_pkgs): (Vec<_>, Vec<_>) = pkgs
            .into_iter()
            .map(|(_, pkg)| pkg)
            .partition(|p| p.config.release());
        if selected_pkgs.is_empty() {
            let _ = crate::ops::shell::error("no packages selected");
            return Err(2.into());
        }

        let dry_run = !self.execute;
        let mut failed = false;

        // STEP 0: Help the user make the right decisions.
        failed |= !super::verify_git_is_clean(
            ws_meta.workspace_root.as_std_path(),
            dry_run,
            log::Level::Error,
        )?;

        failed |= !super::verify_git_branch(
            ws_meta.workspace_root.as_std_path(),
            &ws_config,
            dry_run,
            log::Level::Error,
        )?;

        failed |= !super::verify_if_behind(
            ws_meta.workspace_root.as_std_path(),
            &ws_config,
            dry_run,
            log::Level::Warn,
        )?;

        // STEP 1: Release Confirmation
        super::confirm("Owner", &selected_pkgs, self.no_confirm, dry_run)?;

        ensure_owners(&selected_pkgs, dry_run)?;

        super::finish(failed, dry_run)
    }

    fn to_config(&self) -> crate::config::ConfigArgs {
        crate::config::ConfigArgs {
            custom_config: self.custom_config.clone(),
            isolated: self.isolated,
            z: self.z.clone(),
            allow_branch: self.allow_branch.clone(),
            ..Default::default()
        }
    }
}

pub fn ensure_owners(pkgs: &[plan::PackageRelease], dry_run: bool) -> Result<(), CliError> {
    for pkg in pkgs {
        if !pkg.config.publish() || !pkg.ensure_owners {
            continue;
        }

        let crate_name = pkg.meta.name.as_str();
        crate::ops::cargo::ensure_owners(
            crate_name,
            pkg.config.owners(),
            pkg.config.registry(),
            dry_run,
        )?;
    }

    Ok(())
}
