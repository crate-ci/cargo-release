use std::env;
use std::path::Path;

use bstr::ByteSlice;

use crate::config;
use crate::error::CargoResult;
use crate::ops::cmd::call;

/// Expresses what features flags should be used
#[derive(Clone, Debug)]
pub enum Features {
    /// None - don't use special features
    None,
    /// Only use selected features
    Selective(Vec<String>),
    /// Use all features via `all-features`
    All,
}

fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

pub fn package_content(manifest_path: &Path) -> CargoResult<Vec<std::path::PathBuf>> {
    let mut cmd = std::process::Command::new(cargo());
    cmd.arg("package");
    cmd.arg("--manifest-path");
    cmd.arg(manifest_path);
    cmd.arg("--list");
    // Not worth passing around allow_dirty to here since we are just getting a file list.
    cmd.arg("--allow-dirty");
    let output = cmd.output()?;

    let parent = manifest_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new(""));

    if output.status.success() {
        let paths = ByteSlice::lines(output.stdout.as_slice())
            .map(|l| parent.join(l.to_path_lossy()))
            .collect();
        Ok(paths)
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::format_err!(
            "failed to get package content for {}: {}",
            manifest_path.display(),
            error
        ))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn publish(
    dry_run: bool,
    verify: bool,
    manifest_path: &Path,
    pkgid: Option<&str>,
    features: &Features,
    registry: Option<&str>,
    target: Option<&str>,
) -> CargoResult<bool> {
    let cargo = cargo();

    let mut command: Vec<&str> = vec![
        &cargo,
        "publish",
        "--manifest-path",
        manifest_path.to_str().unwrap(),
    ];

    if let Some(pkgid) = pkgid {
        command.push("--package");
        command.push(pkgid);
    }

    if let Some(registry) = registry {
        command.push("--registry");
        command.push(registry);
    }

    if dry_run {
        command.push("--dry-run");
        command.push("--allow-dirty");
    }

    if !verify {
        command.push("--no-verify");
    }

    if let Some(target) = target {
        command.push("--target");
        command.push(target);
    }

    let feature_arg;
    match features {
        Features::None => (),
        Features::Selective(vec) => {
            feature_arg = vec.join(" ");
            command.push("--features");
            command.push(&feature_arg);
        }
        Features::All => {
            command.push("--all-features");
        }
    };

    call(command, false)
}

pub fn wait_for_publish(
    index: &mut tame_index::index::ComboIndex,
    name: &str,
    version: &str,
    timeout: std::time::Duration,
    dry_run: bool,
) -> CargoResult<()> {
    if !dry_run {
        let now = std::time::Instant::now();
        let sleep_time = std::time::Duration::from_secs(1);
        let mut logged = false;
        loop {
            if let tame_index::index::ComboIndex::Git(gi) = index {
                if let Err(e) = gi.fetch() {
                    log::debug!("crate index update failed with {}", e);
                }
            }
            if is_published(index, name, version) {
                break;
            } else if timeout < now.elapsed() {
                anyhow::bail!("timeout waiting for crate to be published");
            }

            if !logged {
                let _ = crate::ops::shell::status(
                    "Waiting",
                    format!("on {name} to propagate to index"),
                );
                logged = true;
            }
            std::thread::sleep(sleep_time);
        }
    }

    Ok(())
}

pub fn is_published(index: &tame_index::index::ComboIndex, name: &str, version: &str) -> bool {
    match index.krate(name.try_into().expect("crate name is invalid"), true) {
        Ok(Some(crate_data)) => crate_data
            .versions
            .into_iter()
            .any(|iv| iv.version == version),
        Ok(None) => false,
        Err(err) => {
            // For both http and git indices, this _might_ be an error that goes away in
            // a future call, but at least printing out something should give the user
            // an indication something is amiss
            log::warn!("failed to read metadata for {name}: {err:#}");
            false
        }
    }
}

pub fn set_workspace_version(
    manifest_path: &Path,
    version: &str,
    dry_run: bool,
) -> CargoResult<()> {
    let original_manifest = std::fs::read_to_string(manifest_path)?;
    let mut manifest: toml_edit::Document = original_manifest.parse()?;
    manifest["workspace"]["package"]["version"] = toml_edit::value(version);
    let manifest = manifest.to_string();

    if dry_run {
        if manifest != original_manifest {
            let display_path = manifest_path.display().to_string();
            let old_lines: Vec<_> = original_manifest
                .lines()
                .map(|s| format!("{}\n", s))
                .collect();
            let new_lines: Vec<_> = manifest.lines().map(|s| format!("{}\n", s)).collect();
            let diff = difflib::unified_diff(
                &old_lines,
                &new_lines,
                display_path.as_str(),
                display_path.as_str(),
                "original",
                "updated",
                0,
            );
            log::debug!("change:\n{}", itertools::join(diff.into_iter(), ""));
        }
    } else {
        atomic_write(manifest_path, &manifest)?;
    }

    Ok(())
}

pub fn ensure_owners(
    name: &str,
    logins: &[String],
    registry: Option<&str>,
    dry_run: bool,
) -> CargoResult<()> {
    let cargo = cargo();

    // "Look-before-you-leap" in case the user has permission to publish but not set owners.
    let mut cmd = std::process::Command::new(&cargo);
    cmd.arg("owner").arg(name).arg("--color=never");
    cmd.arg("--list");
    if let Some(registry) = registry {
        cmd.arg("--registry");
        cmd.arg(registry);
    }
    let output = cmd.output()?;
    if !output.status.success() {
        anyhow::bail!(
            "failed talking to registry about crate owners: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let raw = String::from_utf8(output.stdout)
        .map_err(|_| anyhow::format_err!("unrecognized response from registry"))?;

    let mut current = std::collections::BTreeSet::new();
    // HACK: No programmatic CLI access and don't want to link against `cargo` (yet), so parsing
    // text output
    for line in raw.lines() {
        if let Some((owner, _)) = line.split_once(' ') {
            if !owner.is_empty() {
                current.insert(owner);
            }
        }
    }

    let expected = logins
        .iter()
        .map(|s| s.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    let missing = expected.difference(&current).copied().collect::<Vec<_>>();
    if !missing.is_empty() {
        let _ = crate::ops::shell::status(
            "Adding",
            format!("owners for {}: {}", name, missing.join(", ")),
        );
        if !dry_run {
            let mut cmd = std::process::Command::new(&cargo);
            cmd.arg("owner").arg(name).arg("--color=never");
            for missing in missing {
                cmd.arg("--add").arg(missing);
            }
            if let Some(registry) = registry {
                cmd.arg("--registry");
                cmd.arg(registry);
            }
            let output = cmd.output()?;
            if !output.status.success() {
                // HACK: Can't error as the user might not have permission to set owners and we can't
                // tell what the error was without parsing it
                let _ = crate::ops::shell::warn(format!(
                    "failed to set owners for {}: {}",
                    name,
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }
    }

    let extra = current.difference(&expected).copied().collect::<Vec<_>>();
    if !extra.is_empty() {
        log::debug!("extra owners for {}: {}", name, extra.join(", "));
    }

    Ok(())
}

pub fn set_package_version(manifest_path: &Path, version: &str, dry_run: bool) -> CargoResult<()> {
    let original_manifest = std::fs::read_to_string(manifest_path)?;
    let mut manifest: toml_edit::Document = original_manifest.parse()?;
    manifest["package"]["version"] = toml_edit::value(version);
    let manifest = manifest.to_string();

    if dry_run {
        if manifest != original_manifest {
            let display_path = manifest_path.display().to_string();
            let old_lines: Vec<_> = original_manifest
                .lines()
                .map(|s| format!("{}\n", s))
                .collect();
            let new_lines: Vec<_> = manifest.lines().map(|s| format!("{}\n", s)).collect();
            let diff = difflib::unified_diff(
                &old_lines,
                &new_lines,
                display_path.as_str(),
                display_path.as_str(),
                "original",
                "updated",
                0,
            );
            log::debug!("change:\n{}", itertools::join(diff.into_iter(), ""));
        }
    } else {
        atomic_write(manifest_path, &manifest)?;
    }

    Ok(())
}

pub fn upgrade_dependency_req(
    manifest_name: &str,
    manifest_path: &Path,
    root: &Path,
    name: &str,
    version: &semver::Version,
    upgrade: config::DependentVersion,
    dry_run: bool,
) -> CargoResult<()> {
    let manifest_root = manifest_path
        .parent()
        .expect("always at least a parent dir");
    let original_manifest = std::fs::read_to_string(manifest_path)?;
    let mut manifest: toml_edit::Document = original_manifest.parse()?;

    for dep_item in find_dependency_tables(manifest.as_table_mut())
        .flat_map(|t| t.iter_mut().filter_map(|(_, d)| d.as_table_like_mut()))
        .filter(|d| is_relevant(*d, manifest_root, root))
    {
        upgrade_req(manifest_name, dep_item, name, version, upgrade);
    }

    let manifest = manifest.to_string();
    if manifest != original_manifest {
        if dry_run {
            let display_path = manifest_path.display().to_string();
            let old_lines: Vec<_> = original_manifest
                .lines()
                .map(|s| format!("{}\n", s))
                .collect();
            let new_lines: Vec<_> = manifest.lines().map(|s| format!("{}\n", s)).collect();
            let diff = difflib::unified_diff(
                &old_lines,
                &new_lines,
                display_path.as_str(),
                display_path.as_str(),
                "original",
                "updated",
                0,
            );
            log::debug!("change:\n{}", itertools::join(diff.into_iter(), ""));
        } else {
            atomic_write(manifest_path, &manifest)?;
        }
    }

    Ok(())
}

fn find_dependency_tables(
    root: &mut toml_edit::Table,
) -> impl Iterator<Item = &mut dyn toml_edit::TableLike> + '_ {
    const DEP_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

    root.iter_mut().flat_map(|(k, v)| {
        if DEP_TABLES.contains(&k.get()) {
            v.as_table_like_mut().into_iter().collect::<Vec<_>>()
        } else if k == "workspace" {
            v.as_table_like_mut()
                .unwrap()
                .iter_mut()
                .filter_map(|(k, v)| {
                    if k.get() == "dependencies" {
                        v.as_table_like_mut()
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        } else if k == "target" {
            v.as_table_like_mut()
                .unwrap()
                .iter_mut()
                .flat_map(|(_, v)| {
                    v.as_table_like_mut().into_iter().flat_map(|v| {
                        v.iter_mut().filter_map(|(k, v)| {
                            if DEP_TABLES.contains(&k.get()) {
                                v.as_table_like_mut()
                            } else {
                                None
                            }
                        })
                    })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    })
}

fn is_relevant(d: &dyn toml_edit::TableLike, dep_crate_root: &Path, crate_root: &Path) -> bool {
    if !d.contains_key("version") {
        return false;
    }
    match d
        .get("path")
        .and_then(|i| i.as_str())
        .and_then(|relpath| dunce::canonicalize(dep_crate_root.join(relpath)).ok())
    {
        Some(dep_path) => dep_path == crate_root,
        None => false,
    }
}

fn upgrade_req(
    manifest_name: &str,
    dep_item: &mut dyn toml_edit::TableLike,
    name: &str,
    version: &semver::Version,
    upgrade: config::DependentVersion,
) -> bool {
    let version_value = if let Some(version_value) = dep_item.get_mut("version") {
        version_value
    } else {
        log::debug!("not updating path-only dependency on {}", name);
        return false;
    };

    let existing_req_str = if let Some(existing_req) = version_value.as_str() {
        existing_req
    } else {
        log::debug!("unsupported dependency {}", name);
        return false;
    };
    let existing_req = if let Ok(existing_req) = semver::VersionReq::parse(existing_req_str) {
        existing_req
    } else {
        log::debug!("unsupported dependency req {}={}", name, existing_req_str);
        return false;
    };
    let new_req = match upgrade {
        config::DependentVersion::Fix => {
            if !existing_req.matches(version) {
                let new_req = crate::ops::version::upgrade_requirement(existing_req_str, version)
                    .ok()
                    .flatten();
                if let Some(new_req) = new_req {
                    new_req
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }
        config::DependentVersion::Upgrade => {
            let new_req = crate::ops::version::upgrade_requirement(existing_req_str, version)
                .ok()
                .flatten();
            if let Some(new_req) = new_req {
                new_req
            } else {
                return false;
            }
        }
    };

    let _ = crate::ops::shell::status(
        "Updating",
        format!(
            "{}'s dependency from {} to {}",
            manifest_name, existing_req_str, new_req
        ),
    );
    *version_value = toml_edit::value(new_req);
    true
}

pub fn update_lock(manifest_path: &Path) -> CargoResult<()> {
    cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()?;

    Ok(())
}

pub fn sort_workspace(ws_meta: &cargo_metadata::Metadata) -> Vec<&cargo_metadata::PackageId> {
    let members: std::collections::HashSet<_> = ws_meta.workspace_members.iter().collect();
    let dep_tree: std::collections::HashMap<_, _> = ws_meta
        .resolve
        .as_ref()
        .expect("cargo-metadata resolved deps")
        .nodes
        .iter()
        .filter_map(|n| {
            if members.contains(&n.id) {
                // Ignore dev dependencies. This breaks dev dependency cyles and allows for
                // correct publishing order when a workspace package depends on the root package.

                // It would be more correct to ignore only dev dependencies without a version
                // field specified. However, cargo_metadata exposes only the resolved version of
                // a package, and not what semver range (if any) is requested in Cargo.toml.

                let non_dev_pkgs = n.deps.iter().filter_map(|dep| {
                    let dev_only = dep
                        .dep_kinds
                        .iter()
                        .all(|info| info.kind == cargo_metadata::DependencyKind::Development);

                    if dev_only {
                        None
                    } else {
                        Some(&dep.pkg)
                    }
                });

                Some((&n.id, non_dev_pkgs.collect()))
            } else {
                None
            }
        })
        .collect();

    let mut sorted = Vec::new();
    let mut processed = std::collections::HashSet::new();
    for pkg_id in ws_meta.workspace_members.iter() {
        sort_workspace_inner(pkg_id, &dep_tree, &mut processed, &mut sorted);
    }

    sorted
}

fn sort_workspace_inner<'m>(
    pkg_id: &'m cargo_metadata::PackageId,
    dep_tree: &std::collections::HashMap<
        &'m cargo_metadata::PackageId,
        Vec<&'m cargo_metadata::PackageId>,
    >,
    processed: &mut std::collections::HashSet<&'m cargo_metadata::PackageId>,
    sorted: &mut Vec<&'m cargo_metadata::PackageId>,
) {
    if !processed.insert(pkg_id) {
        return;
    }

    for dep_id in dep_tree[pkg_id]
        .iter()
        .filter(|dep_id| dep_tree.contains_key(*dep_id))
    {
        sort_workspace_inner(dep_id, dep_tree, processed, sorted);
    }

    sorted.push(pkg_id);
}

fn atomic_write(path: &Path, data: &str) -> std::io::Result<()> {
    let temp_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Cargo.toml.work");
    std::fs::write(&temp_path, data)?;
    std::fs::rename(&temp_path, path)?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[allow(unused_imports)] // Not being detected
    use assert_fs::prelude::*;
    use predicates::prelude::*;

    mod set_package_version {
        use super::*;

        #[test]
        fn succeeds() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");

            let meta = cargo_metadata::MetadataCommand::new()
                .manifest_path(manifest_path.path())
                .exec()
                .unwrap();
            assert_eq!(meta.packages[0].version.to_string(), "0.1.0");

            set_package_version(manifest_path.path(), "2.0.0", false).unwrap();

            let meta = cargo_metadata::MetadataCommand::new()
                .manifest_path(manifest_path.path())
                .exec()
                .unwrap();
            assert_eq!(meta.packages[0].version.to_string(), "2.0.0");

            temp.close().unwrap();
        }
    }

    mod update_lock {
        use super::*;

        #[test]
        fn in_pkg() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            let lock_path = temp.child("Cargo.lock");

            set_package_version(manifest_path.path(), "2.0.0", false).unwrap();
            lock_path.assert(predicate::path::eq_file(Path::new(
                "tests/fixtures/simple/Cargo.lock",
            )));

            update_lock(manifest_path.path()).unwrap();
            lock_path.assert(
                predicate::path::eq_file(Path::new("tests/fixtures/simple/Cargo.lock")).not(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn in_pure_workspace() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/pure_ws", &["**"]).unwrap();
            let manifest_path = temp.child("b/Cargo.toml");
            let lock_path = temp.child("Cargo.lock");

            set_package_version(manifest_path.path(), "2.0.0", false).unwrap();
            lock_path.assert(predicate::path::eq_file(Path::new(
                "tests/fixtures/pure_ws/Cargo.lock",
            )));

            update_lock(manifest_path.path()).unwrap();
            lock_path.assert(
                predicate::path::eq_file(Path::new("tests/fixtures/pure_ws/Cargo.lock")).not(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn in_mixed_workspace() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/mixed_ws", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            let lock_path = temp.child("Cargo.lock");

            set_package_version(manifest_path.path(), "2.0.0", false).unwrap();
            lock_path.assert(predicate::path::eq_file(Path::new(
                "tests/fixtures/mixed_ws/Cargo.lock",
            )));

            update_lock(manifest_path.path()).unwrap();
            lock_path.assert(
                predicate::path::eq_file(Path::new("tests/fixtures/mixed_ws/Cargo.lock")).not(),
            );

            temp.close().unwrap();
        }
    }

    mod sort_workspace {
        use super::*;

        #[test]
        fn circular_dev_dependency() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/mixed_ws", &["**"]).unwrap();
            let manifest_path = temp.child("a/Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "a"
    version = "0.1.0"
    authors = []

    [dev-dependencies]
    b = { path = "../" }
    "#,
                )
                .unwrap();
            let root_manifest_path = temp.child("Cargo.toml");
            let meta = cargo_metadata::MetadataCommand::new()
                .manifest_path(root_manifest_path.path())
                .exec()
                .unwrap();

            let sorted = sort_workspace(&meta);
            let root_package = meta.resolve.as_ref().unwrap().root.as_ref().unwrap();
            assert_ne!(
                sorted[0], root_package,
                "The root package must not be the first one to be published."
            );

            temp.close().unwrap();
        }
    }
}
