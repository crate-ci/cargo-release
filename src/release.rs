use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;

use indexmap::IndexMap;
use indexmap::IndexSet;
use itertools::Itertools;

use crate::error::FatalError;
use crate::replace::{do_file_replacements, Template};
use crate::version::VersionExt;
use crate::*;

pub(crate) fn release_workspace(args: &args::ReleaseOpt) -> Result<i32, error::FatalError> {
    let ws_meta = args
        .manifest
        .metadata()
        // When evaluating dependency ordering, we need to consider optional dependencies
        .features(cargo_metadata::CargoOpt::AllFeatures)
        .exec()
        .map_err(FatalError::from)?;
    let root = git::top_level(ws_meta.workspace_root.as_std_path())?;
    let ws_config = config::load_workspace_config(args, &ws_meta)?;

    let member_ids = cargo::sort_workspace(&ws_meta);
    let pkgs: Result<IndexMap<_, _>, _> = member_ids
        .iter()
        .filter_map(|p| PackageRelease::load(args, &root, &ws_meta, &ws_meta[p]).transpose())
        .map(|p| p.map(|p| (&p.meta.id, p)))
        .collect();
    let mut pkgs = pkgs?;

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
        if let Some((changed, lock_changed)) = changed_since(&ws_meta, pkg, prev_tag_name) {
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

    let mut shared_max: Option<version::Version> = None;
    let mut shared_ids = IndexSet::new();
    for (pkg_id, pkg) in pkgs.iter() {
        if pkg.config.shared_version() {
            shared_ids.insert(*pkg_id);
            let planned = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
            if shared_max
                .as_ref()
                .map(|max| max.full_version < planned.full_version)
                .unwrap_or(true)
            {
                shared_max = Some(planned.clone());
            }
        }
    }
    if let Some(shared_max) = shared_max {
        for shared_id in shared_ids {
            let shared_pkg = &mut pkgs[shared_id];
            if shared_pkg.prev_version.bare_version != shared_max.bare_version {
                shared_pkg.version = Some(shared_max.clone());
            }
        }
    }

    for pkg in pkgs.values_mut() {
        pkg.plan()?;
    }

    let pkgs: Vec<_> = pkgs
        .into_iter()
        .map(|(_, pkg)| pkg)
        .filter(|p| p.config.release())
        .collect();
    if pkgs.is_empty() {
        log::info!("No packages selected.");
        return Ok(0);
    }
    release_packages(args, &ws_meta, &ws_config, pkgs.as_slice())
}

fn release_packages<'m>(
    args: &args::ReleaseOpt,
    ws_meta: &cargo_metadata::Metadata,
    ws_config: &config::Config,
    pkgs: &'m [PackageRelease<'m>],
) -> Result<i32, error::FatalError> {
    let dry_run = args.dry_run();

    // STEP 0: Help the user make the right decisions.
    git::git_version()?;

    if git::is_dirty(ws_meta.workspace_root.as_std_path())? {
        log::error!("Uncommitted changes detected, please commit before release.");
        if !dry_run {
            return Ok(101);
        }
    }

    let mut tag_exists = false;
    let mut seen_tags = HashSet::new();
    for pkg in pkgs {
        if let Some(tag_name) = pkg.tag.as_ref() {
            if seen_tags.insert(tag_name) {
                let cwd = pkg.package_root;
                if git::tag_exists(cwd, tag_name)? {
                    let crate_name = pkg.meta.name.as_str();
                    log::error!("Tag `{}` already exists (for `{}`)", tag_name, crate_name);
                    tag_exists = true;
                }
            }
        }
    }
    if tag_exists {
        if !dry_run {
            return Ok(101);
        }
    }

    let mut downgrades_present = false;
    for pkg in pkgs {
        if let Some(version) = pkg.version.as_ref() {
            if version.full_version < pkg.prev_version.full_version {
                let crate_name = pkg.meta.name.as_str();
                log::error!(
                    "Cannot downgrade {} from {} to {}",
                    crate_name,
                    version.full_version,
                    pkg.prev_version.full_version
                );
                downgrades_present = true;
            }
        }
    }
    if downgrades_present {
        if !dry_run {
            return Ok(101);
        }
    }

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
        if !dry_run {
            return Ok(101);
        }
    }

    let mut changed_pkgs = HashSet::new();
    for pkg in pkgs {
        if let Some(version) = pkg.version.as_ref() {
            let crate_name = pkg.meta.name.as_str();
            let prev_tag_name = &pkg.prev_tag;
            if let Some((changed, lock_changed)) = changed_since(ws_meta, pkg, prev_tag_name) {
                if !changed.is_empty() {
                    log::debug!(
                        "Files changed in {} since {}: {:#?}",
                        crate_name,
                        prev_tag_name,
                        changed
                    );
                    changed_pkgs.insert(&pkg.meta.id);
                    changed_pkgs.extend(pkg.dependents.iter().map(|d| &d.pkg.id));
                } else if changed_pkgs.contains(&pkg.meta.id) {
                    log::debug!(
                        "Dependency changed for {} since {}",
                        crate_name,
                        prev_tag_name,
                    );
                    changed_pkgs.insert(&pkg.meta.id);
                    changed_pkgs.extend(pkg.dependents.iter().map(|d| &d.pkg.id));
                } else if lock_changed {
                    log::debug!(
                        "Lock file changed for {} since {}, assuming its relevant",
                        crate_name,
                        prev_tag_name
                    );
                    changed_pkgs.insert(&pkg.meta.id);
                    // Lock file changes don't invalidate dependents, which is why this check is
                    // after the transitive check, so that can invalidate dependents
                } else {
                    log::warn!(
                        "Updating {} to {} despite no changes made since tag {}",
                        crate_name,
                        version.full_version_string,
                        prev_tag_name
                    );
                }
            } else {
                log::debug!(
                    "Cannot detect changes for {} because tag {} is missing. Try setting `--prev-tag-name <TAG>`.",
                    crate_name,
                    prev_tag_name
                );
            }
        }
    }

    let git_remote = ws_config.push_remote();
    let branch = git::current_branch(ws_meta.workspace_root.as_std_path())?;
    let mut good_branches = ignore::gitignore::GitignoreBuilder::new(".");
    for pattern in ws_config.allow_branch() {
        good_branches.add_line(None, pattern)?;
    }
    let good_branches = good_branches.build()?;
    let good_branch_match = good_branches.matched_path_or_any_parents(&branch, false);
    if !good_branch_match.is_ignore() {
        log::error!(
            "Cannot release from branch {:?}, instead switch to {:?}",
            branch,
            ws_config.allow_branch().join(", ")
        );
        log::trace!("Due to {:?}", good_branch_match);
        if !dry_run {
            return Ok(101);
        }
    }
    git::fetch(ws_meta.workspace_root.as_std_path(), git_remote, &branch)?;
    if git::is_behind_remote(ws_meta.workspace_root.as_std_path(), git_remote, &branch)? {
        log::warn!("{} is behind {}/{}", branch, git_remote, branch);
    }

    let mut is_shared = true;
    let mut shared_version: Option<version::Version> = None;
    for pkg in pkgs {
        if let Some(version) = pkg.version.as_ref() {
            if pkg.config.shared_version() && pkg.version.is_some() {
                if let Some(shared_version) = shared_version.as_ref() {
                    if shared_version.bare_version != version.bare_version {
                        is_shared = false;
                        log::error!(
                            "{} has version {}, should be {}",
                            pkg.meta.name,
                            version.bare_version,
                            shared_version.bare_version_string
                        );
                    }
                } else {
                    shared_version = Some(version.clone());
                }
            }
        }
    }
    if !is_shared {
        log::error!("Crate versions deviated, aborting");
        return Ok(110);
    }

    // STEP 1: Release Confirmation
    if !dry_run && !args.no_confirm {
        let prompt = if pkgs.len() == 1 {
            let pkg = &pkgs[0];
            let crate_name = pkg.meta.name.as_str();
            let version = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
            format!("Release {} {}?", crate_name, version.full_version_string)
        } else {
            let mut buffer: Vec<u8> = vec![];
            writeln!(&mut buffer, "Release").unwrap();
            for pkg in pkgs {
                let crate_name = pkg.meta.name.as_str();
                let version = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
                writeln!(
                    &mut buffer,
                    "  {} {}",
                    crate_name, version.full_version_string
                )
                .unwrap();
            }
            write!(&mut buffer, "?").unwrap();
            String::from_utf8(buffer).expect("Only valid UTF-8 has been written")
        };

        let confirmed = shell::confirm(&prompt);
        if !confirmed {
            return Ok(0);
        }
    }

    // STEP 2: update current version, save and commit
    let mut shared_commit = false;
    for pkg in pkgs {
        let cwd = pkg.package_root;
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
                pkg.manifest_path,
                version.full_version_string.as_str(),
                dry_run,
            )?;
            update_dependent_versions(pkg, version, dry_run)?;
            if dry_run {
                log::debug!("Updating lock file");
            } else {
                cargo::update_lock(pkg.manifest_path)?;
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
                do_file_replacements(
                    pkg.config.pre_release_replacements(),
                    &template,
                    cwd,
                    prerelease,
                    dry_run,
                )?;
            }

            // pre-release hook
            if let Some(pre_rel_hook) = pkg.config.pre_release_hook() {
                let pre_rel_hook = pre_rel_hook.args();
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
                    return Ok(107);
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
                    return Ok(102);
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
            return Ok(102);
        }
    }

    // STEP 3: cargo publish
    for pkg in pkgs {
        if !pkg.config.publish() {
            continue;
        }

        let crate_name = pkg.meta.name.as_str();
        if pkg.config.registry().is_none() && pkg.version.is_none() {
            let version = &pkg.prev_version;
            let index = crates_index::Index::new_cargo_default()?;
            if cargo::is_published(&index, crate_name, &version.full_version_string) {
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
        if !cargo::publish(
            dry_run,
            verify,
            pkg.manifest_path,
            features,
            pkg.config.registry(),
            args.token.as_ref().map(AsRef::as_ref),
            pkg.config.target.as_ref().map(AsRef::as_ref),
        )? {
            return Ok(103);
        }
        let timeout = std::time::Duration::from_secs(300);

        if pkg.config.registry().is_none() {
            let mut index = crates_index::Index::new_cargo_default()?;

            let version = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
            cargo::wait_for_publish(
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

    // STEP 5: Tag
    let mut seen_tags = HashSet::new();
    for pkg in pkgs {
        if let Some(tag_name) = pkg.tag.as_ref() {
            if seen_tags.insert(tag_name) {
                let cwd = pkg.package_root;
                let crate_name = pkg.meta.name.as_str();

                let version = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
                let prev_version_var = pkg.prev_version.bare_version_string.as_str();
                let prev_metadata_var = pkg.prev_version.full_version.build.as_str();
                let version_var = version.bare_version_string.as_str();
                let metadata_var = version.full_version.build.as_str();
                let template = Template {
                    prev_version: Some(prev_version_var),
                    prev_metadata: Some(prev_metadata_var),
                    version: Some(version_var),
                    metadata: Some(metadata_var),
                    crate_name: Some(crate_name),
                    tag_name: Some(tag_name),
                    date: Some(NOW.as_str()),
                    ..Default::default()
                };
                let tag_message = template.render(pkg.config.tag_message());

                log::debug!("Creating git tag {}", tag_name);
                if !git::tag(cwd, tag_name, &tag_message, pkg.config.sign_tag(), dry_run)? {
                    // tag failed, abort release
                    return Ok(104);
                }
            }
        }
    }

    // STEP 6: bump version
    let mut shared_commit = false;
    let mut shared_post_version: Option<version::Version> = None;
    for pkg in pkgs {
        if let Some(next_version) = pkg.post_version.as_ref() {
            let cwd = pkg.package_root;
            let crate_name = pkg.meta.name.as_str();

            log::info!(
                "Starting {}'s next development iteration {}",
                crate_name,
                next_version.full_version_string
            );
            update_dependent_versions(pkg, next_version, dry_run)?;
            cargo::set_package_version(
                pkg.manifest_path,
                next_version.full_version_string.as_str(),
                dry_run,
            )?;
            if !dry_run {
                cargo::update_lock(pkg.manifest_path)?;
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
                do_file_replacements(
                    pkg.config.post_release_replacements(),
                    &template,
                    cwd,
                    false, // post-release replacements should always be applied
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
                    return Ok(105);
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
            return Ok(102);
        }
    }

    // STEP 7: post-release hook
    for pkg in pkgs {
        if let (Some(post_rel_hook), Some(_)) =
            (pkg.config.post_release_hook(), pkg.version.as_ref())
        {
            let cwd = pkg.package_root;
            let post_rel_hook = post_rel_hook.args();
            log::debug!("Calling post-release hook: {:?}", post_rel_hook);
            let envs = maplit::btreemap! {
                OsStr::new("RELEASE_TAG") => OsStr::new(pkg.tag.as_ref().expect("post-release hook with no tag")),
                OsStr::new("DRY_RUN") => OsStr::new(if dry_run { "true" } else { "false" }),
            };
            // we use dry_run environmental variable to run the script
            // so here we set dry_run=false and always execute the command.
            if !cmd::call_with_env(post_rel_hook, envs, cwd, false)? {
                log::error!("Post release hook failed. Aborting. CHANGES NOT ROLLED BACK.",);
                return Ok(107);
            }
        }
    }

    // STEP 8: git push
    if ws_config.push() {
        let mut shared_refs = HashSet::new();
        for pkg in pkgs {
            if !pkg.config.push() {
                continue;
            }

            if pkg.config.consolidate_pushes() {
                shared_refs.insert(branch.as_str());
                if let Some(tag_name) = pkg.tag.as_deref() {
                    shared_refs.insert(tag_name);
                }
            } else {
                let mut refs = vec![branch.as_str()];
                if let Some(tag_name) = pkg.tag.as_deref() {
                    refs.push(tag_name)
                }
                log::info!("Pushing {} to {}", refs.join(", "), git_remote);
                let cwd = pkg.package_root;
                if !git::push(cwd, git_remote, refs, pkg.config.push_options(), dry_run)? {
                    return Ok(106);
                }
            }
        }
        if !shared_refs.is_empty() {
            let mut shared_refs = shared_refs.into_iter().collect::<Vec<_>>();
            shared_refs.sort_unstable();
            log::info!("Pushing {} to {}", shared_refs.join(", "), git_remote);
            if !git::push(
                ws_meta.workspace_root.as_std_path(),
                git_remote,
                shared_refs,
                ws_config.push_options(),
                dry_run,
            )? {
                return Ok(106);
            }
        }
    }

    if dry_run {
        log::warn!("Ran a `dry-run`, re-run with `--execute` if all looked good.");
    }

    Ok(0)
}

static NOW: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| {
    time::OffsetDateTime::now_utc()
        .format(time::macros::format_description!("[year]-[month]-[day]"))
        .unwrap()
});

fn find_dependents<'w>(
    ws_meta: &'w cargo_metadata::Metadata,
    pkg_meta: &'w cargo_metadata::Package,
) -> impl Iterator<Item = (&'w cargo_metadata::Package, &'w cargo_metadata::Dependency)> {
    ws_meta.packages.iter().filter_map(move |p| {
        if ws_meta.workspace_members.iter().any(|m| *m == p.id) {
            p.dependencies
                .iter()
                .find(|d| d.name == pkg_meta.name)
                .map(|d| (p, d))
        } else {
            None
        }
    })
}

struct PackageRelease<'m> {
    meta: &'m cargo_metadata::Package,
    manifest_path: &'m Path,
    package_root: &'m Path,
    is_root: bool,
    config: config::Config,

    package_content: Vec<std::path::PathBuf>,
    bin: bool,
    dependents: Vec<Dependency<'m>>,
    features: cargo::Features,

    prev_version: version::Version,
    prev_tag: String,

    version: Option<version::Version>,
    tag: Option<String>,
    post_version: Option<version::Version>,
}

impl<'m> PackageRelease<'m> {
    fn load(
        args: &args::ReleaseOpt,
        git_root: &Path,
        ws_meta: &'m cargo_metadata::Metadata,
        pkg_meta: &'m cargo_metadata::Package,
    ) -> Result<Option<Self>, error::FatalError> {
        let manifest_path = pkg_meta.manifest_path.as_std_path();
        let package_root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        let config = config::load_package_config(args, ws_meta, pkg_meta)?;
        if !config.release() {
            log::trace!("Disabled in config, skipping {}", manifest_path.display());
            return Ok(None);
        }

        let package_content = cargo::package_content(manifest_path)?;
        let bin = pkg_meta
            .targets
            .iter()
            .flat_map(|t| t.kind.iter())
            .any(|k| k == "bin");
        let features = config.features();
        let dependents = find_dependents(ws_meta, pkg_meta)
            .map(|(pkg, dep)| Dependency { pkg, req: &dep.req })
            .collect();

        let is_root = git_root == package_root;
        let prev_version = version::Version::from(pkg_meta.version.clone());
        let prev_tag = if let Some(prev_tag) = args.prev_tag_name.as_ref() {
            // Trust the user that the tag passed in is the latest tag for the workspace and that
            // they don't care about any changes from before this tag.
            prev_tag.to_owned()
        } else {
            let prev_version_var = prev_version.bare_version_string.as_str();
            let prev_metadata_var = prev_version.full_version.build.as_str();
            let version_var = prev_version.bare_version_string.as_str();
            let metadata_var = prev_version.full_version.build.as_str();
            let mut template = Template {
                prev_version: Some(prev_version_var),
                prev_metadata: Some(prev_metadata_var),
                version: Some(version_var),
                metadata: Some(metadata_var),
                crate_name: Some(pkg_meta.name.as_str()),
                ..Default::default()
            };

            let tag_prefix = config.tag_prefix(is_root);
            let tag_prefix = template.render(tag_prefix);
            template.prefix = Some(&tag_prefix);
            template.render(config.tag_name())
        };

        let version = args
            .level_or_version
            .bump(&prev_version.full_version, args.metadata.as_deref())?;
        let tag = None;
        let post_version = None;

        let pkg = PackageRelease {
            meta: pkg_meta,
            manifest_path,
            package_root,
            is_root,
            config,

            package_content,
            bin,
            dependents,
            features,

            prev_version,
            prev_tag,

            version,
            tag,
            post_version,
        };
        Ok(Some(pkg))
    }

    fn plan(&mut self) -> Result<(), FatalError> {
        let base = self.version.as_ref().unwrap_or(&self.prev_version);
        let tag = if self.config.tag() {
            let prev_version_var = self.prev_version.bare_version_string.as_str();
            let prev_metadata_var = self.prev_version.full_version.build.as_str();
            let version_var = base.bare_version_string.as_str();
            let metadata_var = base.full_version.build.as_str();
            let mut template = Template {
                prev_version: Some(prev_version_var),
                prev_metadata: Some(prev_metadata_var),
                version: Some(version_var),
                metadata: Some(metadata_var),
                crate_name: Some(self.meta.name.as_str()),
                ..Default::default()
            };

            let tag_prefix = self.config.tag_prefix(self.is_root);
            let tag_prefix = template.render(tag_prefix);
            template.prefix = Some(&tag_prefix);
            Some(template.render(self.config.tag_name()))
        } else {
            None
        };

        let is_pre_release = base.is_prerelease();
        let post_version = if !is_pre_release && self.config.dev_version() {
            let mut post = base.full_version.clone();
            post.increment_patch();
            post.pre = semver::Prerelease::new(self.config.dev_version_ext())?;

            Some(version::Version::from(post))
        } else {
            None
        };

        self.tag = tag;
        self.post_version = post_version;

        Ok(())
    }
}

fn changed_since<'m>(
    ws_meta: &cargo_metadata::Metadata,
    pkg: &'m PackageRelease<'m>,
    since_ref: &str,
) -> Option<(Vec<std::path::PathBuf>, bool)> {
    let lock_path = ws_meta.workspace_root.join("Cargo.lock");
    let changed_root = if pkg.bin {
        ws_meta.workspace_root.as_std_path()
    } else {
        // Limit our lookup since we don't need to check for `Cargo.lock`
        pkg.package_root
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

struct Dependency<'m> {
    pkg: &'m cargo_metadata::Package,
    req: &'m semver::VersionReq,
}

fn update_dependent_versions(
    pkg: &PackageRelease,
    version: &version::Version,
    dry_run: bool,
) -> Result<(), error::FatalError> {
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
                if !dep.req.matches(&version.bare_version) {
                    let new_req = version::set_requirement(dep.req, &version.bare_version)?;
                    if let Some(new_req) = new_req {
                        log::info!(
                            "Fixing {}'s dependency on {} to `{}` (from `{}`)",
                            dep.pkg.name,
                            pkg.meta.name,
                            new_req,
                            dep.req
                        );
                        cargo::set_dependency_version(
                            dep.pkg.manifest_path.as_std_path(),
                            &pkg.meta.name,
                            &new_req,
                            dry_run,
                        )?;
                    }
                }
            }
            config::DependentVersion::Upgrade => {
                let new_req = version::set_requirement(dep.req, &version.bare_version)?;
                if let Some(new_req) = new_req {
                    log::info!(
                        "Upgrading {}'s dependency on {} to `{}` (from `{}`)",
                        dep.pkg.name,
                        pkg.meta.name,
                        new_req,
                        dep.req
                    );
                    cargo::set_dependency_version(
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
