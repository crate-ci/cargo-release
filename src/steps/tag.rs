use std::collections::HashSet;

use crate::error::CliError;
use crate::ops::git;
use crate::ops::replace::NOW;
use crate::ops::replace::Template;
use crate::steps::plan;

/// Tag the released commits
///
/// Will automatically skip existing tags
#[derive(Debug, Clone, clap::Args)]
pub struct TagStep {
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

    /// Unstable options
    #[arg(short, value_name = "FEATURE")]
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
    tag: crate::config::TagArgs,
}

impl TagStep {
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

            pkg.planned_tag = None;
            pkg.config.tag = Some(false);
            pkg.config.release = Some(false);

            let crate_name = pkg.meta.name.as_str();
            log::debug!("disabled by user, skipping {crate_name}",);
        }

        let mut pkgs = plan::plan(pkgs)?;

        for pkg in pkgs.values_mut() {
            if let Some(tag_name) = pkg.planned_tag.as_ref()
                && git::tag_exists(ws_meta.workspace_root.as_std_path(), tag_name)?
            {
                let crate_name = pkg.meta.name.as_str();
                let _ = crate::ops::shell::warn(format!(
                    "disabled due to existing tag ({tag_name}), skipping {crate_name}"
                ));
                pkg.planned_tag = None;
                pkg.config.tag = Some(false);
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
        super::confirm("Tag", &selected_pkgs, self.no_confirm, dry_run)?;

        // STEP 5: Tag
        tag(&selected_pkgs, dry_run)?;

        super::finish(failed, dry_run)
    }

    fn to_config(&self) -> crate::config::ConfigArgs {
        crate::config::ConfigArgs {
            custom_config: self.custom_config.clone(),
            isolated: self.isolated,
            z: self.z.clone(),
            allow_branch: self.allow_branch.clone(),
            tag: self.tag.clone(),
            ..Default::default()
        }
    }
}

pub fn tag(pkgs: &[plan::PackageRelease], dry_run: bool) -> Result<(), CliError> {
    let mut seen_tags = HashSet::new();
    for pkg in pkgs {
        if let Some(tag_name) = pkg.planned_tag.as_ref()
            && seen_tags.insert(tag_name)
        {
            let cwd = &pkg.package_root;
            let crate_name = pkg.meta.name.as_str();

            let version = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
            let prev_version_var = pkg.initial_version.bare_version_string.as_str();
            let prev_metadata_var = pkg.initial_version.full_version.build.as_str();
            let version_var = version.bare_version_string.as_str();
            let metadata_var = version.full_version.build.as_str();
            let template = Template {
                prev_version: Some(prev_version_var),
                prev_metadata: Some(prev_metadata_var),
                version: Some(version_var),
                metadata: Some(metadata_var),
                crate_name: Some(crate_name),
                date: Some(NOW.as_str()),
                tag_name: Some(tag_name),
                ..Default::default()
            };
            let tag_message = template.render(pkg.config.tag_message());

            log::debug!("creating git tag {tag_name}");
            if !git::tag(cwd, tag_name, &tag_message, pkg.config.sign_tag(), dry_run)? {
                // tag failed, abort release
                return Err(101.into());
            }
        }
    }

    Ok(())
}
