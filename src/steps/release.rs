use crate::config;
use crate::error::CliError;
use crate::ops::cargo;
use crate::ops::git;
use crate::steps::plan;

#[derive(Debug, Clone, clap::Args)]
pub struct ReleaseStep {
    #[command(flatten)]
    manifest: clap_cargo::Manifest,

    #[command(flatten)]
    workspace: clap_cargo::Workspace,

    /// Process all packages whose current version is unpublished
    #[arg(long, conflicts_with = "level_or_version")]
    unpublished: bool,

    /// Either bump by LEVEL or set the VERSION for all selected packages
    #[arg(value_name = "LEVEL|VERSION")]
    level_or_version: Option<super::TargetVersion>,

    /// Semver metadata
    #[arg(short, long, requires = "level_or_version")]
    metadata: Option<String>,

    /// Actually perform a release. Dry-run mode is the default
    #[arg(short = 'x', long)]
    execute: bool,

    #[arg(short = 'n', long, conflicts_with = "execute", hide = true)]
    dry_run: bool,

    /// Skip release confirmation and version preview
    #[arg(long)]
    no_confirm: bool,

    /// The name of tag for the previous release.
    #[arg(long, value_name = "NAME")]
    prev_tag_name: Option<String>,

    #[command(flatten)]
    config: config::ConfigArgs,
}

impl ReleaseStep {
    pub fn run(&self) -> Result<(), CliError> {
        git::git_version()?;
        let mut index = crate::ops::index::CratesIoIndex::new();

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
        let ws_config = config::load_workspace_config(&self.config, &ws_meta)?;
        let mut pkgs = plan::load(&self.config, &ws_meta)?;

        for pkg in pkgs.values_mut() {
            if let Some(prev_tag) = self.prev_tag_name.as_ref() {
                // Trust the user that the tag passed in is the latest tag for the workspace and that
                // they don't care about any changes from before this tag.
                pkg.set_prior_tag(prev_tag.to_owned());
            }
            if pkg.config.release() {
                if let Some(level_or_version) = &self.level_or_version {
                    pkg.bump(level_or_version, self.metadata.as_deref())?;
                }
            }
            if index.has_krate(pkg.config.registry(), &pkg.meta.name)? {
                // Already published, skip it.  Use `cargo release owner` for one-time updates
                pkg.ensure_owners = false;
            }
        }

        let (_selected_pkgs, excluded_pkgs) = self.workspace.partition_packages(&ws_meta);
        for excluded_pkg in &excluded_pkgs {
            let pkg = if let Some(pkg) = pkgs.get_mut(&excluded_pkg.id) {
                pkg
            } else {
                // Either not in workspace or marked as `release = false`.
                continue;
            };
            if !pkg.config.release() {
                continue;
            }

            let crate_name = pkg.meta.name.as_str();
            let explicitly_excluded = self.workspace.exclude.contains(&excluded_pkg.name);
            // 1. Don't show this message if already not releasing in config
            // 2. Still respect `--exclude`
            if pkg.config.release()
                && pkg.config.publish()
                && self.unpublished
                && !explicitly_excluded
            {
                let version = &pkg.initial_version;
                if !cargo::is_published(
                    &mut index,
                    pkg.config.registry(),
                    crate_name,
                    &version.full_version_string,
                ) {
                    log::debug!(
                        "enabled {}, v{} is unpublished",
                        crate_name,
                        version.full_version_string
                    );
                    continue;
                }
            }

            pkg.planned_version = None;
            pkg.config.release = Some(false);

            if let Some(prior_tag_name) = &pkg.prior_tag {
                if let Some(changed) =
                    crate::steps::version::changed_since(&ws_meta, pkg, prior_tag_name)
                {
                    if !changed.is_empty() {
                        let _ = crate::ops::shell::warn(format!(
                            "disabled by user, skipping {} which has files changed since {}: {:#?}",
                            crate_name, prior_tag_name, changed
                        ));
                    } else {
                        log::trace!(
                            "disabled by user, skipping {} (no changes since {})",
                            crate_name,
                            prior_tag_name
                        );
                    }
                } else {
                    log::debug!(
                        "disabled by user, skipping {} (no {} tag)",
                        crate_name,
                        prior_tag_name
                    );
                }
            } else {
                log::debug!("disabled by user, skipping {} (no tag found)", crate_name,);
            }
        }

        let pkgs = plan::plan(pkgs)?;

        for excluded_pkg in &excluded_pkgs {
            let pkg = if let Some(pkg) = pkgs.get(&excluded_pkg.id) {
                pkg
            } else {
                // Either not in workspace or marked as `release = false`.
                continue;
            };

            // HACK: `index` only supports default registry
            if pkg.config.publish() && pkg.config.registry().is_none() {
                let version = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
                let crate_name = pkg.meta.name.as_str();
                if !cargo::is_published(
                    &mut index,
                    pkg.config.registry(),
                    crate_name,
                    &version.full_version_string,
                ) {
                    let _ = crate::ops::shell::warn(format!(
                        "disabled by user, skipping {} v{} despite being unpublished",
                        crate_name, version.full_version_string,
                    ));
                }
            }
        }

        let (selected_pkgs, excluded_pkgs): (Vec<_>, Vec<_>) = pkgs
            .into_iter()
            .map(|(_, pkg)| pkg)
            .partition(|p| p.config.release());
        if selected_pkgs.is_empty() {
            let _ = crate::ops::shell::error("no packages selected");
            return Err(2.into());
        }

        let dry_run = !self.execute;
        let mut failed = false;

        let consolidate_commits = super::consolidate_commits(&selected_pkgs, &excluded_pkgs)?;

        // STEP 0: Help the user make the right decisions.
        failed |= !super::verify_git_is_clean(
            ws_meta.workspace_root.as_std_path(),
            dry_run,
            log::Level::Error,
        )?;

        failed |= !super::verify_tags_missing(&selected_pkgs, dry_run, log::Level::Error)?;

        failed |=
            !super::verify_monotonically_increasing(&selected_pkgs, dry_run, log::Level::Error)?;

        let mut double_publish = false;
        for pkg in &selected_pkgs {
            if !pkg.config.publish() {
                continue;
            }
            let version = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
            let crate_name = pkg.meta.name.as_str();
            if cargo::is_published(
                &mut index,
                pkg.config.registry(),
                crate_name,
                &version.full_version_string,
            ) {
                let _ = crate::ops::shell::error(format!(
                    "{} {} is already published",
                    crate_name, version.full_version_string
                ));
                double_publish = true;
            }
        }
        if double_publish {
            failed = true;
            if !dry_run {
                return Err(101.into());
            }
        }

        super::warn_changed(&ws_meta, &selected_pkgs)?;

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
        failed |=
            !super::verify_rate_limit(&selected_pkgs, &mut index, dry_run, log::Level::Error)?;

        // STEP 1: Release Confirmation
        super::confirm("Release", &selected_pkgs, self.no_confirm, dry_run)?;

        // STEP 2: update current version, save and commit
        if consolidate_commits {
            let update_lock =
                super::version::update_versions(&ws_meta, &selected_pkgs, &excluded_pkgs, dry_run)?;
            if update_lock {
                log::debug!("updating lock file");
                if !dry_run {
                    let workspace_path = ws_meta.workspace_root.as_std_path().join("Cargo.toml");
                    cargo::update_lock(&workspace_path)?;
                }
            }

            for pkg in &selected_pkgs {
                super::replace::replace(pkg, dry_run)?;

                // pre-release hook
                super::hook::hook(&ws_meta, pkg, dry_run)?;
            }

            super::commit::workspace_commit(&ws_meta, &ws_config, &selected_pkgs, dry_run)?;
        } else {
            for pkg in &selected_pkgs {
                if let Some(version) = pkg.planned_version.as_ref() {
                    let crate_name = pkg.meta.name.as_str();
                    let _ = crate::ops::shell::status(
                        "Upgrading",
                        format!(
                            "{} from {} to {}",
                            crate_name,
                            pkg.initial_version.full_version_string,
                            version.full_version_string
                        ),
                    );
                    cargo::set_package_version(
                        &pkg.manifest_path,
                        version.full_version_string.as_str(),
                        dry_run,
                    )?;
                    crate::steps::version::update_dependent_versions(
                        &ws_meta, pkg, version, dry_run,
                    )?;
                    if dry_run {
                        log::debug!("updating lock file");
                    } else {
                        cargo::update_lock(&pkg.manifest_path)?;
                    }
                }

                super::replace::replace(pkg, dry_run)?;

                // pre-release hook
                super::hook::hook(&ws_meta, pkg, dry_run)?;

                super::commit::pkg_commit(pkg, dry_run)?;
            }
        }

        // STEP 3: cargo publish
        super::publish::publish(&ws_meta, &selected_pkgs, &mut index, dry_run)?;
        super::owner::ensure_owners(&selected_pkgs, dry_run)?;

        // STEP 5: Tag
        super::tag::show_status(&selected_pkgs);
        super::tag::tag(&selected_pkgs, dry_run)?;

        // STEP 6: git push
        super::push::push(&ws_config, &ws_meta, &selected_pkgs, dry_run)?;

        super::finish(failed, dry_run)
    }
}
