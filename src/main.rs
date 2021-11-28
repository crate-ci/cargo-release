#![allow(clippy::collapsible_if)]
#![allow(clippy::comparison_chain)]

use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::Path;
use std::process::exit;

use chrono::prelude::Local;
use itertools::Itertools;
use structopt::StructOpt;

mod args;
mod cargo;
mod cmd;
mod config;
mod error;
mod git;
mod replace;
mod shell;
mod version;

use crate::error::FatalError;
use crate::replace::{do_file_replacements, Template};
use crate::version::VersionExt;

fn main() {
    match args::Command::from_args() {
        args::Command::Release(ref release_matches) => {
            let mut builder = get_logging(release_matches.logging.log_level());
            builder.init();

            match release_workspace(release_matches) {
                Ok(code) => exit(code),
                Err(e) => {
                    log::error!("Fatal: {}", e);
                    exit(128);
                }
            }
        }
        args::Command::Completions(ref completion_matches) => {
            args::Command::clap().gen_completions_to(
                "cargo-release",
                completion_matches.shell,
                &mut io::stdout(),
            );
        }
    }
}

fn release_workspace(args: &args::ReleaseOpt) -> Result<i32, error::FatalError> {
    let ws_meta = args.manifest.metadata().exec().map_err(FatalError::from)?;
    let ws_config = {
        let mut release_config = config::Config::default();

        if !args.isolated {
            let cfg = config::resolve_workspace_config(ws_meta.workspace_root.as_std_path())?;
            release_config.update(&cfg);
        }

        if let Some(custom_config_path) = args.custom_config.as_ref() {
            // when calling with -c option
            let cfg =
                config::resolve_custom_config(Path::new(custom_config_path))?.unwrap_or_default();
            release_config.update(&cfg);
        }

        release_config.update(&args.config.to_config());
        release_config
    };

    let pkg_ids = cargo::sort_workspace(&ws_meta);

    let (selected_pkgs, excluded_pkgs) = args.workspace.partition_packages(&ws_meta);
    if selected_pkgs.is_empty() {
        log::info!("No packages selected.");
        return Ok(0);
    }

    let root = git::top_level(ws_meta.workspace_root.as_std_path())?;
    let pkg_releases: Result<HashMap<_, _>, _> = selected_pkgs
        .iter()
        .filter_map(|p| PackageRelease::load(args, &root, &ws_meta, p).transpose())
        .map(|p| p.map(|p| (&p.meta.id, p)))
        .collect();
    let pkg_releases = pkg_releases?;
    let pkg_releases: Vec<_> = pkg_ids
        .iter()
        .filter_map(|id| pkg_releases.get(id))
        .collect();

    let excluded_pkgs: Result<Vec<_>, _> = excluded_pkgs
        .iter()
        .filter(|p| pkg_ids.contains(&&p.id))
        .filter_map(|p| PackageRelease::load(args, &root, &ws_meta, p).transpose())
        .collect();
    let excluded_pkgs = excluded_pkgs?;
    for pkg in excluded_pkgs {
        if pkg.version.is_some() {
            let crate_name = pkg.meta.name.as_str();
            let prev_tag_name = &pkg.prev_tag;
            if let Some((changed, lock_changed)) = changed_since(&ws_meta, &pkg, prev_tag_name) {
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
    }

    release_packages(args, &ws_meta, &ws_config, pkg_releases.as_slice())
}

fn release_packages<'m>(
    args: &args::ReleaseOpt,
    ws_meta: &cargo_metadata::Metadata,
    ws_config: &config::Config,
    pkgs: &'m [&'m PackageRelease<'m>],
) -> Result<i32, error::FatalError> {
    let dry_run = args.dry_run();

    // STEP 0: Help the user make the right decisions.
    git::git_version()?;
    let mut dirty = false;
    if ws_config.consolidate_commits() {
        if git::is_dirty(ws_meta.workspace_root.as_std_path())? {
            log::error!("Uncommitted changes detected, please commit before release.");
            dirty = true;
        }
    } else {
        for pkg in pkgs {
            let cwd = pkg.package_path;
            if git::is_dirty(cwd)? {
                let crate_name = pkg.meta.name.as_str();
                log::error!(
                    "Uncommitted changes detected for {}, please commit before release.",
                    crate_name
                );
                dirty = true;
            }
        }
    }
    if dirty {
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
            let pkg = pkgs[0];
            let crate_name = pkg.meta.name.as_str();
            let base = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
            format!("Release {} {}?", crate_name, base.full_version_string)
        } else {
            let mut buffer: Vec<u8> = vec![];
            writeln!(&mut buffer, "Release").unwrap();
            for pkg in pkgs {
                let crate_name = pkg.meta.name.as_str();
                let base = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
                writeln!(&mut buffer, "  {} {}", crate_name, base.full_version_string).unwrap();
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
        let cwd = pkg.package_path;
        let crate_name = pkg.meta.name.as_str();

        if let Some(version) = pkg.version.as_ref() {
            let prev_version_string = pkg.prev_version.full_version_string.as_str();
            let new_version_string = version.full_version_string.as_str();
            log::info!("Update {} to version {}", crate_name, new_version_string);
            if !dry_run {
                cargo::set_package_version(pkg.manifest_path, new_version_string)?;
            }
            update_dependent_versions(pkg, version, dry_run)?;
            if dry_run {
                log::debug!("Updating lock file");
            } else {
                cargo::update_lock(pkg.manifest_path)?;
            }

            if !pkg.config.pre_release_replacements().is_empty() {
                // try replacing text in configured files
                let template = Template {
                    prev_version: Some(&prev_version_string),
                    version: Some(new_version_string),
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
                    OsStr::new("PREV_VERSION") => prev_version_string.as_ref(),
                    OsStr::new("NEW_VERSION") => new_version_string.as_ref(),
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
                    prev_version: Some(prev_version_string),
                    version: Some(new_version_string),
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
            let template = Template {
                version: shared_version
                    .as_ref()
                    .map(|v| v.full_version_string.as_str()),
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
        if pkg.config.publish() {
            let crate_name = pkg.meta.name.as_str();
            let base = pkg.version.as_ref().unwrap_or(&pkg.prev_version);

            log::info!("Running cargo publish on {}", crate_name);
            // feature list to release
            let features = &pkg.features;
            if !cargo::publish(
                dry_run,
                pkg.config.verify(),
                pkg.manifest_path,
                features,
                pkg.config.registry(),
                args.token.as_ref().map(AsRef::as_ref),
            )? {
                return Ok(103);
            }
            let timeout = std::time::Duration::from_secs(300);

            if pkg.config.registry().is_none() {
                cargo::wait_for_publish(crate_name, &base.full_version_string, timeout, dry_run)?;
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
    }

    // STEP 5: Tag
    for pkg in pkgs {
        if let Some(tag_name) = pkg.tag.as_ref() {
            let sign = pkg.config.sign_commit() || pkg.config.sign_tag();

            // FIXME: remove when the meaning of sign_commit is changed
            if !pkg.config.sign_tag() && pkg.config.sign_commit() {
                log::warn!("In next minor release, `sign-commit` will only be used to control git commit signing. Use option `sign-tag` for tag signing.");
            }

            let cwd = pkg.package_path;
            let crate_name = pkg.meta.name.as_str();

            let base = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
            let template = Template {
                prev_version: Some(&pkg.prev_version.full_version_string),
                version: Some(&base.full_version_string),
                crate_name: Some(crate_name),
                tag_name: Some(tag_name),
                date: Some(NOW.as_str()),
                ..Default::default()
            };
            let tag_message = template.render(pkg.config.tag_message());

            log::debug!("Creating git tag {}", tag_name);
            if !git::tag(cwd, tag_name, &tag_message, sign, dry_run)? {
                // tag failed, abort release
                return Ok(104);
            }
        }
    }

    // STEP 6: bump version
    let mut shared_commit = false;
    let mut shared_post_version: Option<version::Version> = None;
    for pkg in pkgs {
        if let Some(version) = pkg.post_version.as_ref() {
            let cwd = pkg.package_path;
            let crate_name = pkg.meta.name.as_str();

            let updated_version_string = version.full_version_string.as_ref();
            log::info!(
                "Starting {}'s next development iteration {}",
                crate_name,
                updated_version_string,
            );
            update_dependent_versions(pkg, version, dry_run)?;
            if !dry_run {
                cargo::set_package_version(pkg.manifest_path, updated_version_string)?;
                cargo::update_lock(pkg.manifest_path)?;
            }
            let base = pkg.version.as_ref().unwrap_or(&pkg.prev_version);
            let template = Template {
                prev_version: Some(&pkg.prev_version.full_version_string),
                version: Some(&base.full_version_string),
                crate_name: Some(crate_name),
                date: Some(NOW.as_str()),
                tag_name: pkg.tag.as_deref(),
                next_version: Some(updated_version_string),
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
                shared_post_version = Some(version.clone());
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
            let template = Template {
                version: shared_version
                    .as_ref()
                    .map(|v| v.full_version_string.as_str()),
                date: Some(NOW.as_str()),
                next_version: shared_post_version
                    .as_ref()
                    .map(|v| v.full_version_string.as_str()),
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

    // STEP 7: git push
    if ws_config.push() {
        let mut shared_push = false;
        for pkg in pkgs {
            if !pkg.config.push() {
                continue;
            }

            let cwd = pkg.package_path;
            if let Some(tag_name) = pkg.tag.as_ref() {
                log::info!("Pushing {} to {}", tag_name, git_remote);
                if !git::push_tag(cwd, git_remote, tag_name, dry_run)? {
                    return Ok(106);
                }
            }

            if pkg.config.consolidate_pushes() {
                shared_push = true
            } else if !shared_push {
                log::info!("Pushing HEAD to {}", git_remote);
                if !git::push(
                    cwd,
                    git_remote,
                    Some(branch.as_str()),
                    pkg.config.push_options(),
                    dry_run,
                )? {
                    return Ok(106);
                }
            }
        }
        if shared_push {
            log::info!("Pushing HEAD to {}", git_remote);
            if !git::push(
                ws_meta.workspace_root.as_std_path(),
                git_remote,
                Some(branch.as_str()),
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

pub fn get_logging(level: log::Level) -> env_logger::Builder {
    let mut builder = env_logger::Builder::new();

    builder.filter(None, level.to_level_filter());

    builder.format_timestamp_secs().format_module_path(false);

    builder
}

static NOW: once_cell::sync::Lazy<String> =
    once_cell::sync::Lazy::new(|| Local::now().format("%Y-%m-%d").to_string());

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
    package_path: &'m Path,
    config: config::Config,

    bin: bool,

    package_content: Vec<std::path::PathBuf>,

    prev_version: version::Version,
    prev_tag: String,
    version: Option<version::Version>,
    tag: Option<String>,
    post_version: Option<version::Version>,

    dependents: Vec<Dependency<'m>>,

    features: cargo::Features,
}

impl<'m> PackageRelease<'m> {
    fn load(
        args: &args::ReleaseOpt,
        git_root: &Path,
        ws_meta: &'m cargo_metadata::Metadata,
        pkg_meta: &'m cargo_metadata::Package,
    ) -> Result<Option<Self>, error::FatalError> {
        let manifest_path = pkg_meta.manifest_path.as_std_path();
        let cwd = manifest_path.parent().unwrap_or_else(|| Path::new("."));

        let config = {
            let mut release_config = config::Config::default();

            if !args.isolated {
                let cfg =
                    config::resolve_config(ws_meta.workspace_root.as_std_path(), manifest_path)?;
                release_config.update(&cfg);
            }

            if let Some(custom_config_path) = args.custom_config.as_ref() {
                // when calling with -c option
                let cfg = config::resolve_custom_config(Path::new(custom_config_path))?
                    .unwrap_or_default();
                release_config.update(&cfg);
            }

            release_config.update(&args.config.to_config());

            // the publish flag in cargo file
            let cargo_file = cargo::parse_cargo_config(manifest_path)?;
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

            release_config
        };
        if !config.release() {
            log::trace!("Disabled in config, skipping {}", manifest_path.display());
            return Ok(None);
        }

        let is_root = git_root == cwd;

        let prev_version = version::Version::from(pkg_meta.version.clone());

        let package_content = cargo::package_content(manifest_path)?;

        let prev_tag = if let Some(prev_tag) = args.prev_tag_name.as_ref() {
            // Trust the user that the tag passed in is the latest tag for the workspace and that
            // they don't care about any changes from before this tag.
            prev_tag.to_owned()
        } else {
            let mut template = Template {
                prev_version: Some(&prev_version.full_version_string),
                version: Some(&prev_version.full_version_string),
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
        let is_pre_release = version
            .as_ref()
            .map(version::Version::is_prerelease)
            .unwrap_or(false);
        let dependents = find_dependents(ws_meta, pkg_meta)
            .map(|(pkg, dep)| Dependency { pkg, req: &dep.req })
            .collect();

        let base = version.as_ref().unwrap_or(&prev_version);

        let tag = if config.tag() {
            let mut template = Template {
                prev_version: Some(&prev_version.full_version_string),
                version: Some(&base.full_version_string),
                crate_name: Some(pkg_meta.name.as_str()),
                ..Default::default()
            };

            let tag_prefix = config.tag_prefix(is_root);
            let tag_prefix = template.render(tag_prefix);
            template.prefix = Some(&tag_prefix);
            Some(template.render(config.tag_name()))
        } else {
            None
        };

        let post_version = if !is_pre_release && config.dev_version() {
            let mut post = base.full_version.clone();
            post.increment_patch();
            post.pre = semver::Prerelease::new(config.dev_version_ext())?;

            Some(version::Version::from(post))
        } else {
            None
        };

        let bin = pkg_meta
            .targets
            .iter()
            .flat_map(|t| t.kind.iter())
            .any(|k| k == "bin");

        let features = config.features();

        let pkg = PackageRelease {
            meta: pkg_meta,
            manifest_path,
            package_path: cwd,
            config,

            bin,

            package_content,

            prev_version,
            prev_tag,
            version,
            tag,
            post_version,
            dependents,

            features,
        };
        Ok(Some(pkg))
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
        pkg.package_path
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
                        if !dry_run {
                            cargo::set_dependency_version(
                                dep.pkg.manifest_path.as_std_path(),
                                &pkg.meta.name,
                                &new_req,
                            )?;
                        }
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
                    if !dry_run {
                        cargo::set_dependency_version(
                            dep.pkg.manifest_path.as_std_path(),
                            &pkg.meta.name,
                            &new_req,
                        )?;
                    }
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
