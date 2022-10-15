use crate::config;
use crate::error::FatalError;
use crate::error::ProcessError;
use crate::ops::git;
use crate::steps::plan;

/// Bump crate versions
#[derive(Debug, Clone, clap::Args)]
pub struct VersionStep {
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

    /// Either bump by LEVEL or set the VERSION for all selected packages
    #[arg(value_name = "LEVEL|VERSION", help_heading = "Version")]
    level_or_version: crate::ops::version::TargetVersion,

    /// Semver metadata
    #[arg(short, long, help_heading = "Version")]
    metadata: Option<String>,

    /// The name of tag for the previous release.
    #[arg(long, help_heading = "Version")]
    prev_tag_name: Option<String>,
}

impl VersionStep {
    pub fn run(&self) -> Result<(), ProcessError> {
        git::git_version()?;

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

        for pkg in pkgs.values_mut() {
            if let Some(prev_tag) = self.prev_tag_name.as_ref() {
                // Trust the user that the tag passed in is the latest tag for the workspace and that
                // they don't care about any changes from before this tag.
                pkg.set_prior_tag(prev_tag.to_owned());
            }
            pkg.bump(&self.level_or_version, self.metadata.as_deref())?;
        }

        let (_selected_pkgs, excluded_pkgs) = self.workspace.partition_packages(&ws_meta);
        for excluded_pkg in excluded_pkgs {
            let pkg = if let Some(pkg) = pkgs.get_mut(&excluded_pkg.id) {
                pkg
            } else {
                // Either not in workspace or marked as `release = false`.
                continue;
            };
            pkg.config.release = Some(false);
            pkg.planned_version = None;

            let crate_name = pkg.meta.name.as_str();
            if let Some(prior_tag_name) = &pkg.prior_tag {
                if let Some((changed, lock_changed)) = changed_since(&ws_meta, pkg, prior_tag_name)
                {
                    if !changed.is_empty() {
                        log::warn!(
                            "Disabled by user, skipping {} which has files changed since {}: {:#?}",
                            crate_name,
                            prior_tag_name,
                            changed
                        );
                    } else if lock_changed {
                        log::warn!(
                        "Disabled by user, skipping {} despite lock file being changed since {}",
                        crate_name,
                        prior_tag_name
                    );
                    } else {
                        log::trace!(
                            "Disabled by user, skipping {} (no changes since {})",
                            crate_name,
                            prior_tag_name
                        );
                    }
                } else {
                    log::debug!(
                        "Disabled by user, skipping {} (no {} tag)",
                        crate_name,
                        prior_tag_name
                    );
                }
            } else {
                log::debug!("Disabled by user, skipping {} (no tag found)", crate_name,);
            }
        }

        let pkgs = plan::plan(pkgs)?;

        let pkgs: Vec<_> = pkgs
            .into_iter()
            .map(|(_, pkg)| pkg)
            .filter(|p| p.config.release())
            .collect();
        if pkgs.is_empty() {
            log::info!("No packages selected.");
            return Err(2.into());
        }

        let dry_run = !self.execute;
        let mut failed = false;

        // STEP 0: Help the user make the right decisions.
        failed |= !super::verify_git_is_clean(
            ws_meta.workspace_root.as_std_path(),
            dry_run,
            log::Level::Warn,
        )?;

        super::warn_changed(&ws_meta, &pkgs)?;

        failed |= !super::verify_git_branch(
            ws_meta.workspace_root.as_std_path(),
            &ws_config,
            dry_run,
            log::Level::Warn,
        )?;

        failed |= !super::verify_if_behind(
            ws_meta.workspace_root.as_std_path(),
            &ws_config,
            dry_run,
            log::Level::Warn,
        )?;

        // STEP 1: Release Confirmation
        super::confirm("Bump", &pkgs, self.no_confirm, dry_run)?;

        // STEP 2: update current version, save and commit
        for pkg in &pkgs {
            if let Some(version) = pkg.planned_version.as_ref() {
                let crate_name = pkg.meta.name.as_str();
                log::info!(
                    "Update {} to version {}",
                    crate_name,
                    version.full_version_string
                );
                crate::ops::cargo::set_package_version(
                    &pkg.manifest_path,
                    version.full_version_string.as_str(),
                    dry_run,
                )?;
                update_dependent_versions(pkg, version, dry_run)?;
                if dry_run {
                    log::debug!("Updating lock file");
                } else {
                    crate::ops::cargo::update_lock(&pkg.manifest_path)?;
                }
            }
        }

        super::finish(failed, dry_run)
    }

    fn to_config(&self) -> crate::config::ConfigArgs {
        crate::config::ConfigArgs {
            custom_config: self.custom_config.clone(),
            isolated: self.isolated,
            allow_branch: self.allow_branch.clone(),
            ..Default::default()
        }
    }
}

pub fn changed_since<'m>(
    ws_meta: &cargo_metadata::Metadata,
    pkg: &'m plan::PackageRelease,
    since_ref: &str,
) -> Option<(Vec<std::path::PathBuf>, bool)> {
    let lock_path = ws_meta.workspace_root.join("Cargo.lock");
    let changed_root = if pkg.bin {
        ws_meta.workspace_root.as_std_path()
    } else {
        // Limit our lookup since we don't need to check for `Cargo.lock`
        &pkg.package_root
    };
    let changed = git::changed_files(changed_root, since_ref).ok().flatten()?;
    let mut changed: Vec<_> = changed
        .into_iter()
        .filter(|p| pkg.package_content.contains(p))
        .collect();

    let mut lock_changed = false;
    if let Some(lock_index) =
        changed.iter().enumerate().find_map(
            |(idx, path)| {
                if path == &lock_path {
                    Some(idx)
                } else {
                    None
                }
            },
        )
    {
        let _ = changed.swap_remove(lock_index);
        if !pkg.bin {
            let crate_name = pkg.meta.name.as_str();
            log::trace!(
                "Ignoring lock file change since {}; {} has no [[bin]]",
                since_ref,
                crate_name
            );
        } else if pkg.config.dev_version() {
            log::debug!(
                "Ignoring lock file change since {}; could be a pre-release version bump.",
                since_ref
            );
        } else {
            lock_changed = true;
        }
    }

    Some((changed, lock_changed))
}

pub fn update_dependent_versions(
    pkg: &plan::PackageRelease,
    version: &crate::ops::version::Version,
    dry_run: bool,
) -> Result<(), FatalError> {
    let new_version_string = version.bare_version_string.as_str();
    let mut dependents_failed = false;
    for dep in pkg.dependents.iter() {
        match pkg.config.dependent_version() {
            config::DependentVersion::Ignore => (),
            config::DependentVersion::Warn => {
                if !dep.req.matches(&version.bare_version) {
                    log::warn!(
                        "{}'s dependency on {} `{}` is incompatible with {}",
                        dep.pkg.name,
                        pkg.meta.name,
                        dep.req,
                        new_version_string
                    );
                }
            }
            config::DependentVersion::Error => {
                if !dep.req.matches(&version.bare_version) {
                    log::warn!(
                        "{}'s dependency on {} `{}` is incompatible with {}",
                        dep.pkg.name,
                        pkg.meta.name,
                        dep.req,
                        new_version_string
                    );
                    dependents_failed = true;
                }
            }
            config::DependentVersion::Fix => {
                if pkg.config.always_update_version() || !dep.req.matches(&version.bare_version) {
                    let new_req =
                        crate::ops::version::set_requirement(&dep.req, &version.bare_version)?;
                    if let Some(new_req) = new_req {
                        log::info!(
                            "Fixing {}'s dependency on {} to `{}` (from `{}`)",
                            dep.pkg.name,
                            pkg.meta.name,
                            new_req,
                            dep.req
                        );
                        crate::ops::cargo::set_dependency_version(
                            dep.pkg.manifest_path.as_std_path(),
                            &pkg.meta.name,
                            &new_req,
                            dry_run,
                        )?;
                    }
                } else {
                    log::debug!(
                        "Not updating {}'s dependency on {} from `{}` \
                        because it already matches new version `{}`",
                        dep.pkg.name,
                        pkg.meta.name,
                        dep.req,
                        version.bare_version,
                    );
                }
            }
            config::DependentVersion::Upgrade => {
                let new_req =
                    crate::ops::version::set_requirement(&dep.req, &version.bare_version)?;
                if let Some(new_req) = new_req {
                    log::info!(
                        "Upgrading {}'s dependency on {} to `{}` (from `{}`)",
                        dep.pkg.name,
                        pkg.meta.name,
                        new_req,
                        dep.req
                    );
                    crate::ops::cargo::set_dependency_version(
                        dep.pkg.manifest_path.as_std_path(),
                        &pkg.meta.name,
                        &new_req,
                        dry_run,
                    )?;
                }
            }
        }
    }
    if dependents_failed {
        Err(FatalError::DependencyVersionConflict)
    } else {
        Ok(())
    }
}
