use crate::error::FatalError;
use crate::error::ProcessError;
use crate::ops::git;
use crate::steps::plan;

/// Publish the specified packages
#[derive(Debug, Clone, clap::Args)]
pub struct PublishStep {
    #[command(flatten)]
    manifest: clap_cargo::Manifest,

    #[command(flatten)]
    workspace: clap_cargo::Workspace,

    /// Custom config file
    #[arg(short, long = "config")]
    custom_config: Option<String>,

    /// Ignore implicit configuration files.
    #[arg(long)]
    isolated: bool,

    /// Comma-separated globs of branch names a release can happen from
    #[arg(long, value_delimiter = ',')]
    allow_branch: Option<Vec<String>>,

    /// Actually perform a release. Dry-run mode is the default
    #[arg(short = 'x', long)]
    execute: bool,

    /// Skip release confirmation and version preview
    #[arg(long)]
    no_confirm: bool,

    #[command(flatten)]
    publish: crate::config::PublishArgs,
}

impl PublishStep {
    pub fn run(&self) -> Result<(), ProcessError> {
        let ws_meta = self
            .manifest
            .metadata()
            // When evaluating dependency ordering, we need to consider optional dependencies
            .features(cargo_metadata::CargoOpt::AllFeatures)
            .exec()
            .map_err(FatalError::from)?;
        let config = self.to_config();
        let ws_config = crate::config::load_workspace_config(&config, &ws_meta)?;
        let mut pkgs = plan::load(&config, &ws_meta)?;

        let (_selected_pkgs, excluded_pkgs) = self.workspace.partition_packages(&ws_meta);
        for excluded_pkg in excluded_pkgs {
            let pkg = if let Some(pkg) = pkgs.get_mut(&excluded_pkg.id) {
                pkg
            } else {
                // Either not in workspace or marked as `release = false`.
                continue;
            };
            pkg.config.release = Some(false);

            let crate_name = pkg.meta.name.as_str();
            log::debug!("Disabled by user, skipping {}", crate_name,);
        }

        let mut pkgs = plan::plan(pkgs)?;

        let index = crates_index::Index::new_cargo_default()?;
        for pkg in pkgs.values_mut() {
            if pkg.config.registry().is_none() {
                let crate_name = pkg.meta.name.as_str();
                let version = &pkg.prev_version;
                if crate::ops::cargo::is_published(&index, crate_name, &version.full_version_string)
                {
                    log::warn!(
                        "Disabled due to previous publish ({}), skipping {}",
                        version.full_version_string,
                        crate_name
                    );
                    pkg.config.publish = Some(false);
                    pkg.config.release = Some(false);
                }
            }
        }

        let pkgs: Vec<_> = pkgs
            .into_iter()
            .map(|(_, pkg)| pkg)
            .filter(|p| p.config.release())
            .collect();
        if pkgs.is_empty() {
            log::info!("No packages selected.");
            return Err(0.into());
        }

        let dry_run = !self.execute;
        let mut failed = false;

        // STEP 0: Help the user make the right decisions.
        git::git_version()?;

        failed |= !super::verify_git_is_clean(ws_meta.workspace_root.as_std_path(), dry_run)?;

        failed |=
            !super::verify_git_branch(ws_meta.workspace_root.as_std_path(), &ws_config, dry_run)?;

        super::warn_if_behind(ws_meta.workspace_root.as_std_path(), &ws_config)?;

        // STEP 1: Release Confirmation
        super::confirm("Publish", &pkgs, self.no_confirm, dry_run)?;

        // STEP 3: cargo publish
        publish(&ws_meta, &pkgs, dry_run)?;

        super::finish(failed, dry_run)
    }

    fn to_config(&self) -> crate::config::ConfigArgs {
        crate::config::ConfigArgs {
            custom_config: self.custom_config.clone(),
            isolated: self.isolated,
            allow_branch: self.allow_branch.clone(),
            publish: self.publish.clone(),
            ..Default::default()
        }
    }
}

pub fn publish(
    ws_meta: &cargo_metadata::Metadata,
    pkgs: &[plan::PackageRelease],
    dry_run: bool,
) -> Result<(), ProcessError> {
    let index = crates_index::Index::new_cargo_default()?;
    for pkg in pkgs {
        if !pkg.config.publish() {
            continue;
        }

        let crate_name = pkg.meta.name.as_str();
        if pkg.config.registry().is_none() && pkg.version.is_none() {
            let version = &pkg.prev_version;
            if crate::ops::cargo::is_published(&index, crate_name, &version.full_version_string) {
                log::warn!("Skipping publish of {} {}, assuming we are recovering from a prior failed release", crate_name, version.full_version_string);
                continue;
            }
        }

        log::info!("Publishing {}", crate_name);

        let verify = if !pkg.config.verify() {
            false
        } else if dry_run && pkgs.len() != 1 {
            log::debug!("Skipping verification to avoid unpublished dependencies from dry-run");
            false
        } else {
            true
        };
        // feature list to release
        let features = &pkg.features;
        let pkgid = if 1 < ws_meta.workspace_members.len() {
            // Override `workspace.default-members`
            Some(crate_name)
        } else {
            // `-p` is not recommended outside of a workspace
            None
        };
        if !crate::ops::cargo::publish(
            dry_run,
            verify,
            &pkg.manifest_path,
            pkgid,
            features,
            pkg.config.registry(),
            pkg.config.target.as_ref().map(AsRef::as_ref),
        )? {
            return Err(103.into());
        }

        if pkg.config.registry().is_none() {
            let mut index = crates_index::Index::new_cargo_default()?;

            let timeout = std::time::Duration::from_secs(300);
            let version = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
            crate::ops::cargo::wait_for_publish(
                &mut index,
                crate_name,
                &version.full_version_string,
                timeout,
                dry_run,
            )?;
            // HACK: Even once the index is updated, there seems to be another step before the publish is fully ready.
            // We don't have a way yet to check for that, so waiting for now in hopes everything is ready
            if !dry_run {
                let publish_grace_sleep = std::env::var("PUBLISH_GRACE_SLEEP")
                    .unwrap_or_else(|_| Default::default())
                    .parse()
                    .unwrap_or(0);
                if 0 < publish_grace_sleep {
                    log::info!(
                        "Waiting an additional {} seconds for crates.io to update its indices...",
                        publish_grace_sleep
                    );
                    std::thread::sleep(std::time::Duration::from_secs(publish_grace_sleep));
                }
            }
        } else {
            log::debug!("Not waiting for publish because the registry is not crates.io and doesn't get updated automatically");
        }
    }

    Ok(())
}
