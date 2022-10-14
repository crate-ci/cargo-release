use std::ffi::OsStr;
use std::path::Path;

use crate::args;
use crate::config;
use crate::error::FatalError;
use crate::error::ProcessError;
use crate::ops::cargo;
use crate::ops::cmd;
use crate::ops::git;
use crate::ops::replace::{do_file_replacements, Template, NOW};
use crate::ops::version;
use crate::steps::plan;
use crate::steps::plan::PackageRelease;

pub(crate) fn release_workspace(args: &args::ReleaseOpt) -> Result<(), ProcessError> {
    let ws_meta = args
        .manifest
        .metadata()
        // When evaluating dependency ordering, we need to consider optional dependencies
        .features(cargo_metadata::CargoOpt::AllFeatures)
        .exec()
        .map_err(FatalError::from)?;
    let ws_config = config::load_workspace_config(&args.config, &ws_meta)?;
    let mut pkgs = plan::load(&args.config, &ws_meta)?;

    for pkg in pkgs.values_mut() {
        if let Some(prev_tag) = args.prev_tag_name.as_ref() {
            // Trust the user that the tag passed in is the latest tag for the workspace and that
            // they don't care about any changes from before this tag.
            pkg.set_prev_tag(prev_tag.to_owned());
        }
        pkg.bump(&args.level_or_version, args.metadata.as_deref())?;
    }

    let (_selected_pkgs, excluded_pkgs) = args.workspace.partition_packages(&ws_meta);
    for excluded_pkg in excluded_pkgs {
        let pkg = if let Some(pkg) = pkgs.get_mut(&excluded_pkg.id) {
            pkg
        } else {
            // Either not in workspace or marked as `release = false`.
            continue;
        };
        pkg.config.release = Some(false);
        pkg.version = None;

        let crate_name = pkg.meta.name.as_str();
        let prev_tag_name = &pkg.prev_tag;
        if let Some((changed, lock_changed)) =
            crate::steps::version::changed_since(&ws_meta, pkg, prev_tag_name)
        {
            if !changed.is_empty() {
                log::warn!(
                    "Disabled by user, skipping {} which has files changed since {}: {:#?}",
                    crate_name,
                    prev_tag_name,
                    changed
                );
            } else if lock_changed {
                log::warn!(
                    "Disabled by user, skipping {} despite lock file being changed since {}",
                    crate_name,
                    prev_tag_name
                );
            } else {
                log::trace!(
                    "Disabled by user, skipping {} (no changes since {})",
                    crate_name,
                    prev_tag_name
                );
            }
        } else {
            log::debug!(
                "Disabled by user, skipping {} (no {} tag)",
                crate_name,
                prev_tag_name
            );
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
        return Err(0.into());
    }
    release_packages(args, &ws_meta, &ws_config, pkgs.as_slice())
}

fn release_packages<'m>(
    args: &args::ReleaseOpt,
    ws_meta: &cargo_metadata::Metadata,
    ws_config: &config::Config,
    pkgs: &'m [PackageRelease],
) -> Result<(), ProcessError> {
    let dry_run = args.dry_run();
    let mut failed = false;

    // STEP 0: Help the user make the right decisions.
    git::git_version()?;

    failed |= !super::verify_git_is_clean(ws_meta.workspace_root.as_std_path(), dry_run)?;

    failed |= !super::verify_tags_missing(pkgs, dry_run)?;

    failed |= !super::verify_monotonically_increasing(pkgs, dry_run)?;

    let mut double_publish = false;
    for pkg in pkgs {
        if !pkg.config.publish() {
            continue;
        }
        if pkg.config.registry().is_none() {
            // While we'll publish when there is `pkg.version.is_none()`, we'll check that case
            // during the publish
            if let Some(version) = pkg.version.as_ref() {
                let index = crates_index::Index::new_cargo_default()?;
                let crate_name = pkg.meta.name.as_str();
                if cargo::is_published(&index, crate_name, &version.full_version_string) {
                    log::error!(
                        "{} {} is already published",
                        crate_name,
                        version.full_version_string
                    );
                    double_publish = true;
                }
            }
        }
    }
    if double_publish {
        failed = true;
        if !dry_run {
            return Err(101.into());
        }
    }

    super::warn_changed(ws_meta, pkgs)?;

    failed |= !super::verify_git_branch(ws_meta.workspace_root.as_std_path(), ws_config, dry_run)?;

    super::warn_if_behind(ws_meta.workspace_root.as_std_path(), ws_config)?;

    let shared_version = super::find_shared_versions(pkgs)?;

    // STEP 1: Release Confirmation
    super::confirm("Release", pkgs, args.no_confirm, dry_run)?;

    // STEP 2: update current version, save and commit
    let mut shared_commit = false;
    for pkg in pkgs {
        let cwd = &pkg.package_root;
        let crate_name = pkg.meta.name.as_str();

        if let Some(version) = pkg.version.as_ref() {
            let prev_version_var = pkg.prev_version.bare_version_string.as_str();
            let prev_metadata_var = pkg.prev_version.full_version.build.as_str();
            let version_var = version.bare_version_string.as_str();
            let metadata_var = version.full_version.build.as_str();
            log::info!(
                "Update {} to version {}",
                crate_name,
                version.full_version_string
            );
            cargo::set_package_version(
                &pkg.manifest_path,
                version.full_version_string.as_str(),
                dry_run,
            )?;
            crate::steps::version::update_dependent_versions(pkg, version, dry_run)?;
            if dry_run {
                log::debug!("Updating lock file");
            } else {
                cargo::update_lock(&pkg.manifest_path)?;
            }

            if !pkg.config.pre_release_replacements().is_empty() {
                // try replacing text in configured files
                let template = Template {
                    prev_version: Some(prev_version_var),
                    prev_metadata: Some(prev_metadata_var),
                    version: Some(version_var),
                    metadata: Some(metadata_var),
                    crate_name: Some(crate_name),
                    date: Some(NOW.as_str()),
                    tag_name: pkg.tag.as_deref(),
                    ..Default::default()
                };
                let prerelease = version.is_prerelease();
                let noisy = false;
                do_file_replacements(
                    pkg.config.pre_release_replacements(),
                    &template,
                    cwd,
                    prerelease,
                    noisy,
                    dry_run,
                )?;
            }

            // pre-release hook
            if let Some(pre_rel_hook) = pkg.config.pre_release_hook() {
                let template = Template {
                    prev_version: Some(prev_version_var),
                    prev_metadata: Some(prev_metadata_var),
                    version: Some(version_var),
                    metadata: Some(metadata_var),
                    crate_name: Some(crate_name),
                    date: Some(NOW.as_str()),
                    tag_name: pkg.tag.as_deref(),
                    ..Default::default()
                };
                let pre_rel_hook = pre_rel_hook
                    .args()
                    .into_iter()
                    .map(|arg| template.render(arg));
                log::debug!("Calling pre-release hook: {:?}", pre_rel_hook);
                let envs = maplit::btreemap! {
                    OsStr::new("PREV_VERSION") => prev_version_var.as_ref(),
                    OsStr::new("PREV_METADATA") => prev_metadata_var.as_ref(),
                    OsStr::new("NEW_VERSION") => version_var.as_ref(),
                    OsStr::new("NEW_METADATA") => metadata_var.as_ref(),
                    OsStr::new("DRY_RUN") => OsStr::new(if dry_run { "true" } else { "false" }),
                    OsStr::new("CRATE_NAME") => OsStr::new(crate_name),
                    OsStr::new("WORKSPACE_ROOT") => ws_meta.workspace_root.as_os_str(),
                    OsStr::new("CRATE_ROOT") => pkg.manifest_path.parent().unwrap_or_else(|| Path::new(".")).as_os_str(),
                };
                // we use dry_run environmental variable to run the script
                // so here we set dry_run=false and always execute the command.
                if !cmd::call_with_env(pre_rel_hook, envs, cwd, false)? {
                    log::error!(
                        "Release of {} aborted by non-zero return of prerelease hook.",
                        crate_name
                    );
                    return Err(107.into());
                }
            }

            if pkg.config.consolidate_commits() {
                shared_commit = true;
            } else {
                let template = Template {
                    prev_version: Some(prev_version_var),
                    prev_metadata: Some(prev_metadata_var),
                    version: Some(version_var),
                    metadata: Some(metadata_var),
                    crate_name: Some(crate_name),
                    date: Some(NOW.as_str()),
                    ..Default::default()
                };
                let commit_msg = template.render(pkg.config.pre_release_commit_message());
                let sign = pkg.config.sign_commit();
                if !git::commit_all(cwd, &commit_msg, sign, dry_run)? {
                    // commit failed, abort release
                    return Err(102.into());
                }
            }
        }
    }
    if shared_commit {
        let shared_commit_msg = {
            let version_var = shared_version
                .as_ref()
                .map(|v| v.bare_version_string.as_str());
            let metadata_var = shared_version
                .as_ref()
                .map(|v| v.full_version.build.as_str());
            let template = Template {
                version: version_var,
                metadata: metadata_var,
                date: Some(NOW.as_str()),
                ..Default::default()
            };
            template.render(ws_config.pre_release_commit_message())
        };
        if !git::commit_all(
            ws_meta.workspace_root.as_std_path(),
            &shared_commit_msg,
            ws_config.sign_commit(),
            dry_run,
        )? {
            // commit failed, abort release
            return Err(102.into());
        }
    }

    // STEP 3: cargo publish
    super::publish::publish(ws_meta, pkgs, dry_run)?;

    // STEP 5: Tag
    super::tag::tag(pkgs, dry_run)?;

    // STEP 6: bump version
    let mut shared_commit = false;
    let mut shared_post_version: Option<version::Version> = None;
    for pkg in pkgs {
        if let Some(next_version) = pkg.post_version.as_ref() {
            let cwd = &pkg.package_root;
            let crate_name = pkg.meta.name.as_str();

            log::info!(
                "Starting {}'s next development iteration {}",
                crate_name,
                next_version.full_version_string
            );
            crate::steps::version::update_dependent_versions(pkg, next_version, dry_run)?;
            cargo::set_package_version(
                &pkg.manifest_path,
                next_version.full_version_string.as_str(),
                dry_run,
            )?;
            if !dry_run {
                cargo::update_lock(&pkg.manifest_path)?;
            }
            let version = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
            let prev_version_var = pkg.prev_version.bare_version_string.as_str();
            let prev_metadata_var = pkg.prev_version.full_version.build.as_str();
            let version_var = version.bare_version_string.as_str();
            let metadata_var = version.full_version.build.as_str();
            let next_version_var = next_version.bare_version_string.as_ref();
            let next_metadata_var = next_version.full_version.build.as_ref();
            let template = Template {
                prev_version: Some(prev_version_var),
                prev_metadata: Some(prev_metadata_var),
                version: Some(version_var),
                metadata: Some(metadata_var),
                crate_name: Some(crate_name),
                date: Some(NOW.as_str()),
                tag_name: pkg.tag.as_deref(),
                next_version: Some(next_version_var),
                next_metadata: Some(next_metadata_var),
                ..Default::default()
            };
            if !pkg.config.post_release_replacements().is_empty() {
                // try replacing text in configured files
                let noisy = false;
                do_file_replacements(
                    pkg.config.post_release_replacements(),
                    &template,
                    cwd,
                    false, // post-release replacements should always be applied
                    noisy,
                    dry_run,
                )?;
            }

            if pkg.config.shared_version() && shared_post_version.is_none() {
                shared_post_version = Some(next_version.clone());
            }
            if pkg.config.consolidate_commits() {
                shared_commit = true;
            } else {
                let sign = pkg.config.sign_commit();

                let commit_msg = template.render(pkg.config.post_release_commit_message());
                if !git::commit_all(cwd, &commit_msg, sign, dry_run)? {
                    return Err(105.into());
                }
            }
        }
    }
    if shared_commit {
        let shared_commit_msg = {
            let version_var = shared_version
                .as_ref()
                .map(|v| v.bare_version_string.as_str());
            let metadata_var = shared_version
                .as_ref()
                .map(|v| v.full_version.build.as_str());
            let next_version_var = shared_post_version
                .as_ref()
                .map(|v| v.bare_version_string.as_str());
            let next_metadata_var = shared_post_version
                .as_ref()
                .map(|v| v.full_version.build.as_str());
            let template = Template {
                version: version_var,
                metadata: metadata_var,
                date: Some(NOW.as_str()),
                next_version: next_version_var,
                next_metadata: next_metadata_var,
                ..Default::default()
            };
            template.render(ws_config.post_release_commit_message())
        };
        if !git::commit_all(
            ws_meta.workspace_root.as_std_path(),
            &shared_commit_msg,
            ws_config.sign_commit(),
            dry_run,
        )? {
            // commit failed, abort release
            return Err(102.into());
        }
    }

    // STEP 7: git push
    super::push::push(ws_config, ws_meta, pkgs, dry_run)?;

    super::finish(failed, dry_run)
}
