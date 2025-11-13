use itertools::Itertools;

use crate::error::CliError;
use crate::ops::git;
use crate::steps::plan;

/// Publish the specified packages
///
/// Will automatically skip published versions
#[derive(Debug, Clone, clap::Args)]
pub struct PublishStep {
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

    #[command(flatten)]
    publish: crate::config::PublishArgs,
}

impl PublishStep {
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
            pkg.config.release = Some(false);

            let crate_name = pkg.meta.name.as_str();
            log::debug!("disabled by user, skipping {crate_name}",);
        }

        let mut pkgs = plan::plan(pkgs)?;

        let mut index = crate::ops::index::CratesIoIndex::new();
        for pkg in pkgs.values_mut() {
            if pkg.config.release() {
                let crate_name = pkg.meta.name.as_str();
                let version = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
                if crate::ops::cargo::is_published(
                    &mut index,
                    pkg.config.registry(),
                    crate_name,
                    &version.full_version_string,
                    pkg.config.certs_source(),
                ) {
                    let _ = crate::ops::shell::warn(format!(
                        "disabled due to previous publish ({}), skipping {}",
                        version.full_version_string, crate_name
                    ));
                    pkg.config.publish = Some(false);
                    pkg.config.release = Some(false);
                }
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

        failed |= !super::verify_metadata(&selected_pkgs, dry_run, log::Level::Error)?;
        failed |= !super::verify_rate_limit(
            &selected_pkgs,
            &mut index,
            &ws_config.rate_limit,
            dry_run,
            log::Level::Error,
        )?;

        // STEP 1: Release Confirmation
        super::confirm("Publish", &selected_pkgs, self.no_confirm, dry_run)?;

        // STEP 3: cargo publish
        publish(&selected_pkgs, dry_run)?;

        super::finish(failed, dry_run)
    }

    fn to_config(&self) -> crate::config::ConfigArgs {
        crate::config::ConfigArgs {
            custom_config: self.custom_config.clone(),
            isolated: self.isolated,
            z: self.z.clone(),
            allow_branch: self.allow_branch.clone(),
            publish: self.publish.clone(),
            ..Default::default()
        }
    }
}

pub fn publish(pkgs: &[plan::PackageRelease], dry_run: bool) -> Result<(), CliError> {
    if pkgs.is_empty() {
        Ok(())
    } else {
        let first_pkg = pkgs.first().unwrap();
        let registry = first_pkg.config.registry();
        let target = first_pkg.config.target.as_deref();
        let publish_grace_sleep = publish_grace_sleep();
        if publish_grace_sleep.is_none()
            && pkgs
                .iter()
                .all(|p| p.config.registry() == registry && p.config.target.as_deref() == target)
        {
            let manifest_path = &first_pkg.manifest_path;
            workspace_publish(manifest_path, pkgs, registry, target, dry_run)
        } else {
            serial_publish(pkgs, publish_grace_sleep, dry_run)
        }
    }
}

fn workspace_publish(
    manifest_path: &std::path::Path,
    pkgs: &[plan::PackageRelease],
    registry: Option<&str>,
    target: Option<&str>,
    dry_run: bool,
) -> Result<(), CliError> {
    let crate_names = pkgs.iter().map(|p| p.meta.name.as_str()).join(", ");
    let _ = crate::ops::shell::status("Publishing", crate_names);

    let verify = pkgs.iter().all(|p| p.config.verify());
    let features = pkgs.iter().map(|p| &p.features).collect::<Vec<_>>();
    // HACK: Ignoring the more precise `pkg.meta.id`.  While it has been stabilized,
    // the version won't match after we do a version bump and it seems too messy to bother
    // trying to specify it.
    // atm at least Cargo doesn't seem to mind if `crate_name` is also a transitive dep, unlike
    // other cargo commands
    let pkgids = pkgs
        .iter()
        .filter(|p| p.config.publish())
        .map(|p| p.meta.name.as_str())
        .collect::<Vec<_>>();
    if !crate::ops::cargo::publish(
        dry_run,
        verify,
        manifest_path,
        &pkgids,
        &features,
        registry,
        target,
    )? {
        return Err(101.into());
    }

    Ok(())
}

fn serial_publish(
    pkgs: &[plan::PackageRelease],
    publish_grace_sleep: Option<u64>,
    dry_run: bool,
) -> Result<(), CliError> {
    for pkg in pkgs {
        if !pkg.config.publish() {
            continue;
        }

        let crate_name = pkg.meta.name.as_str();
        let _ = crate::ops::shell::status("Publishing", crate_name);

        let verify = if !pkg.config.verify() {
            false
        } else if dry_run && pkgs.len() != 1 {
            log::debug!("skipping verification to avoid unpublished dependencies from dry-run");
            false
        } else {
            true
        };
        // feature list to release
        let features = &[&pkg.features];
        // HACK: Ignoring the more precise `pkg.meta.id`.  While it has been stabilized,
        // the version won't match after we do a version bump and it seems too messy to bother
        // trying to specify it.
        // atm at least Cargo doesn't seem to mind if `crate_name` is also a transitive dep, unlike
        // other cargo commands
        let pkgid = &[crate_name];
        if !crate::ops::cargo::publish(
            dry_run,
            verify,
            &pkg.manifest_path,
            pkgid,
            features,
            pkg.config.registry(),
            pkg.config.target.as_ref().map(AsRef::as_ref),
        )? {
            return Err(101.into());
        }

        // HACK: This is a fallback in case users can't or don't want to rely on cargo waiting for
        // them
        if !dry_run && let Some(publish_grace_sleep) = publish_grace_sleep {
            log::debug!(
                "waiting an additional {} seconds for {} to update its indices...",
                publish_grace_sleep,
                pkg.config.registry().unwrap_or("crates.io")
            );
            std::thread::sleep(std::time::Duration::from_secs(publish_grace_sleep));
        }
    }

    Ok(())
}

fn publish_grace_sleep() -> Option<u64> {
    let publish_grace_sleep = std::env::var("PUBLISH_GRACE_SLEEP")
        .unwrap_or_else(|_| Default::default())
        .parse()
        .unwrap_or(0);
    if publish_grace_sleep == 0 {
        None
    } else {
        Some(publish_grace_sleep)
    }
}
