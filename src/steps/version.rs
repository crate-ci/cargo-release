use crate::error::CargoResult;
use crate::error::CliError;
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
    custom_config: Option<std::path::PathBuf>,

    /// Ignore implicit configuration files.
    #[arg(long)]
    isolated: bool,

    /// Comma-separated globs of branch names a release can happen from
    #[arg(long, value_delimiter = ',')]
    allow_branch: Option<Vec<String>>,

    /// Actually perform a release. Dry-run mode is the default
    #[arg(short = 'x', long)]
    execute: bool,

    #[arg(short = 'n', long, conflicts_with = "execute", hide = true)]
    dry_run: bool,

    #[arg(long)]
    exclude_unchanged: bool,

    /// Skip release confirmation and version preview
    #[arg(long)]
    no_confirm: bool,

    /// Either bump by LEVEL or set the VERSION for all selected packages
    #[arg(value_name = "LEVEL|VERSION", help_heading = "Version")]
    level_or_version: super::TargetVersion,

    /// Semver metadata
    #[arg(short, long, help_heading = "Version")]
    metadata: Option<String>,

    /// The name of tag for the previous release.
    #[arg(long, value_name = "NAME", help_heading = "Version")]
    prev_tag_name: Option<String>,
}

impl VersionStep {
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

        for pkg in pkgs.values_mut() {
            if let Some(prev_tag) = self.prev_tag_name.as_ref() {
                // Trust the user that the tag passed in is the latest tag for the workspace and that
                // they don't care about any changes from before this tag.
                pkg.set_prior_tag(prev_tag.to_owned());
            }
            if pkg.config.release() {
                pkg.bump(&self.level_or_version, self.metadata.as_deref())?;
            }
        }

        let (_selected_pkgs, excluded_pkgs) = self.workspace.partition_packages(&ws_meta);
        for excluded_pkg in excluded_pkgs {
            let pkg = if let Some(pkg) = pkgs.get_mut(&excluded_pkg.id) {
                pkg
            } else {
                // Either not in workspace or marked as `release = false`.
                continue;
            };
            if !pkg.config.release() {
                continue;
            }

            pkg.planned_version = None;
            pkg.config.release = Some(false);
        }

        let pkgs = plan::plan(pkgs)?;

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

        // STEP 0: Help the user make the right decisions.
        failed |= !super::verify_git_is_clean(
            ws_meta.workspace_root.as_std_path(),
            dry_run,
            log::Level::Warn,
        )?;

        failed |=
            !super::verify_monotonically_increasing(&selected_pkgs, dry_run, log::Level::Error)?;

        let selected_pkgs = super::detect_changed(&ws_meta, &selected_pkgs, self.exclude_unchanged)?;

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
        super::confirm("Bump", &selected_pkgs, self.no_confirm, dry_run)?;

        // STEP 2: update current version, save and commit
        let update_lock = update_versions(&ws_meta, &selected_pkgs, &excluded_pkgs, dry_run)?;
        if update_lock {
            log::debug!("Updating lock file");
            if !dry_run {
                let workspace_path = ws_meta.workspace_root.as_std_path().join("Cargo.toml");
                crate::ops::cargo::update_lock(&workspace_path)?;
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

pub fn changed_since(
    ws_meta: &cargo_metadata::Metadata,
    pkg: &plan::PackageRelease,
    since_ref: &str,
) -> Option<Vec<std::path::PathBuf>> {
    let changed_root = if pkg.bin {
        ws_meta.workspace_root.as_std_path()
    } else {
        // Limit our lookup since we don't need to check for `Cargo.lock`
        &pkg.package_root
    };
    let changed = git::changed_files(changed_root, since_ref).ok().flatten()?;
    let changed: Vec<_> = changed
        .into_iter()
        .filter(|p| pkg.package_content.contains(p))
        .collect();

    Some(changed)
}

pub fn update_versions(
    ws_meta: &cargo_metadata::Metadata,
    selected_pkgs: &[plan::PackageRelease],
    excluded_pkgs: &[plan::PackageRelease],
    dry_run: bool,
) -> CargoResult<bool> {
    let mut changed = false;

    let workspace_version = selected_pkgs
        .iter()
        .filter(|p| p.config.shared_version() == Some(crate::config::SharedVersion::WORKSPACE))
        .find_map(|p| p.planned_version.clone());

    if let Some(workspace_version) = &workspace_version {
        let _ = crate::ops::shell::status(
            "Upgrading",
            format!(
                "workspace to version {}",
                workspace_version.full_version_string
            ),
        );
        let workspace_path = ws_meta.workspace_root.as_std_path().join("Cargo.toml");
        crate::ops::cargo::set_workspace_version(
            &workspace_path,
            workspace_version.full_version_string.as_str(),
            dry_run,
        )?;
        // Deferring `update_dependent_versions` to the per-package logic
        changed = true;
    }

    for (selected, pkg) in selected_pkgs
        .iter()
        .map(|s| (true, s))
        .chain(excluded_pkgs.iter().map(|s| (false, s)))
    {
        let is_inherited =
            pkg.config.shared_version() == Some(crate::config::SharedVersion::WORKSPACE);
        let planned_version = if is_inherited {
            workspace_version.as_ref()
        } else if let Some(version) = pkg.planned_version.as_ref() {
            assert!(selected);
            Some(version)
        } else {
            None
        };

        if let Some(version) = planned_version {
            if is_inherited {
                let crate_name = pkg.meta.name.as_str();
                let _ = crate::ops::shell::status(
                    "Upgrading",
                    format!(
                        "{} from {} to {} (inherited from workspace)",
                        crate_name,
                        pkg.initial_version.full_version_string,
                        version.full_version_string
                    ),
                );
            } else {
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
                crate::ops::cargo::set_package_version(
                    &pkg.manifest_path,
                    version.full_version_string.as_str(),
                    dry_run,
                )?;
            }
            update_dependent_versions(ws_meta, pkg, version, dry_run)?;
            changed = true;
        }
    }

    Ok(changed)
}

pub fn update_dependent_versions(
    ws_meta: &cargo_metadata::Metadata,
    pkg: &plan::PackageRelease,
    version: &plan::Version,
    dry_run: bool,
) -> CargoResult<()> {
    // This is redundant with iterating over `workspace_members`
    // - As `find_dependency_tables` returns workspace dependencies
    // - If there is a root package
    //
    // But split this out for
    // - Virtual manifests
    // - Nicer message to the user
    {
        let workspace_path = ws_meta.workspace_root.as_std_path().join("Cargo.toml");
        crate::ops::cargo::upgrade_dependency_req(
            "workspace",
            &workspace_path,
            &pkg.package_root,
            &pkg.meta.name,
            &version.full_version,
            pkg.config.dependent_version(),
            dry_run,
        )?;
    }

    for dep in find_ws_members(ws_meta) {
        crate::ops::cargo::upgrade_dependency_req(
            &dep.name,
            dep.manifest_path.as_std_path(),
            &pkg.package_root,
            &pkg.meta.name,
            &version.full_version,
            pkg.config.dependent_version(),
            dry_run,
        )?;
    }

    Ok(())
}

fn find_ws_members(
    ws_meta: &cargo_metadata::Metadata,
) -> impl Iterator<Item = &cargo_metadata::Package> {
    let workspace_members: std::collections::HashSet<_> =
        ws_meta.workspace_members.iter().collect();
    ws_meta
        .packages
        .iter()
        .filter(move |p| workspace_members.contains(&p.id))
}
