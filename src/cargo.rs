use std::env;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::Path;

use bstr::ByteSlice;

use crate::cmd::call;
use crate::error::FatalError;

/// Expresses what features flags should be used
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

pub fn package_content(manifest_path: &Path) -> Result<Vec<std::path::PathBuf>, FatalError> {
    let mut cmd = std::process::Command::new(cargo());
    cmd.arg("package");
    cmd.arg("--manifest-path");
    cmd.arg(manifest_path);
    cmd.arg("--list");
    // Not worth passing around allow_dirty to here since we are just getting a file list.
    cmd.arg("--allow-dirty");
    let output = cmd.output().map_err(FatalError::from)?;

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
        Err(FatalError::PackageListFailed(
            manifest_path.to_owned(),
            error.to_string(),
        ))
    }
}

pub fn is_toolchain_present(toolchain_version: &str) -> bool {
    let formatted_toolchain_version = format!("+{}", toolchain_version);

    let output = std::process::Command::new("cargo")
        .args([&formatted_toolchain_version, "publish", "--help"])
        .output();
    match output {
        Ok(output) => {
            if output.status.success() {
                true
            } else {
                log::trace!(
                    "Error when checking for presence of {} toolchain: {}",
                    toolchain_version,
                    String::from_utf8_lossy(&output.stderr)
                );
                false
            }
        }
        Err(err) => {
            log::trace!(
                "Error when checking for presence of {} toolchain: {}",
                toolchain_version,
                err
            );
            false
        }
    }
}

pub fn publish(
    dry_run: bool,
    verify: bool,
    manifest_path: &Path,
    features: &Features,
    registry: Option<&str>,
    token: Option<&str>,
    toolchain_version: Option<&str>,
) -> Result<bool, FatalError> {
    let mut command: Vec<&str>;

    let bin: String;
    let formatted_toolchain_version;
    if let Some(toolchain_version) = toolchain_version {
        formatted_toolchain_version = format!("+{}", toolchain_version);
        command = vec!["cargo", &formatted_toolchain_version];
    } else {
        bin = cargo();
        command = vec![&bin];
    }

    command.extend([
        "publish",
        "--manifest-path",
        manifest_path.to_str().unwrap(),
    ]);

    if let Some(registry) = registry {
        command.push("--registry");
        command.push(registry);
    }

    if let Some(token) = token {
        command.push("--token");
        command.push(token);
    }

    if dry_run {
        command.push("--dry-run");
        command.push("--allow-dirty");
    }

    if !verify {
        command.push("--no-verify");
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
    name: &str,
    version: &str,
    timeout: std::time::Duration,
    dry_run: bool,
) -> Result<(), FatalError> {
    if !dry_run {
        let now = std::time::Instant::now();
        let sleep_time = std::time::Duration::from_secs(1);
        let mut index = crates_index::Index::new_cargo_default()?;
        let mut logged = false;
        loop {
            if let Err(e) = index.update() {
                log::debug!("Crate index update failed with {}", e);
            }
            let crate_data = index.crate_(name);
            let published = crate_data
                .iter()
                .flat_map(|c| c.versions().iter())
                .any(|v| v.version() == version);

            if published {
                break;
            } else if timeout < now.elapsed() {
                return Err(FatalError::PublishTimeoutError);
            }

            if !logged {
                log::info!("Waiting for publish to complete...");
                logged = true;
            }
            std::thread::sleep(sleep_time);
        }
    }

    Ok(())
}

pub fn set_package_version(manifest_path: &Path, version: &str) -> Result<(), FatalError> {
    let temp_manifest_path = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Cargo.toml.work");

    {
        let manifest = std::fs::read_to_string(manifest_path)?;
        let mut manifest: toml_edit::Document = manifest.parse().map_err(FatalError::from)?;
        manifest["package"]["version"] = toml_edit::value(version);

        let mut file_out = File::create(&temp_manifest_path).map_err(FatalError::from)?;
        file_out
            .write(manifest.to_string().as_bytes())
            .map_err(FatalError::from)?;
    }
    fs::rename(temp_manifest_path, manifest_path)?;

    Ok(())
}

pub fn set_dependency_version(
    manifest_path: &Path,
    name: &str,
    version: &str,
) -> Result<(), FatalError> {
    let temp_manifest_path = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Cargo.toml.work");

    {
        let manifest = std::fs::read_to_string(manifest_path)?;
        let mut manifest: toml_edit::Document = manifest.parse().map_err(FatalError::from)?;

        for deps_table in find_dependency_tables(manifest.as_table_mut()) {
            if let Some(dep_item) = deps_table.get_mut(name) {
                set_version(dep_item, name, version);
            }
        }

        let mut file_out = File::create(&temp_manifest_path).map_err(FatalError::from)?;
        file_out
            .write(manifest.to_string().as_bytes())
            .map_err(FatalError::from)?;
    }
    fs::rename(temp_manifest_path, manifest_path)?;

    Ok(())
}

fn find_dependency_tables<'r>(
    root: &'r mut toml_edit::Table,
) -> impl Iterator<Item = &mut dyn toml_edit::TableLike> + 'r {
    const DEP_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

    root.iter_mut().flat_map(|(k, v)| {
        if DEP_TABLES.contains(&k.get()) {
            v.as_table_like_mut().into_iter().collect::<Vec<_>>()
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

fn set_version(dep_item: &mut toml_edit::Item, name: &str, version: &str) -> bool {
    if let Some(table_like) = dep_item.as_table_like_mut() {
        if let Some(version_value) = table_like.get_mut("version") {
            *version_value = toml_edit::value(version);
            true
        } else {
            log::debug!("Not updating path-only dependency on {}", name);
            false
        }
    } else {
        log::debug!("Not updating version-only dependency on {}", name);
        false
    }
}

pub fn update_lock(manifest_path: &Path) -> Result<(), FatalError> {
    cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()
        .map_err(FatalError::from)?;

    Ok(())
}

pub fn parse_cargo_config(manifest_path: &Path) -> Result<toml_edit::easy::Value, FatalError> {
    let cargo_file_content = std::fs::read_to_string(manifest_path).map_err(FatalError::from)?;
    cargo_file_content.parse().map_err(FatalError::from)
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
                Some((&n.id, &n.dependencies))
            } else {
                None
            }
        })
        .collect();

    let mut sorted = Vec::new();
    let mut processed = std::collections::HashSet::new();
    for pkg_id in ws_meta.workspace_members.iter() {
        sort_workspace_inner(ws_meta, pkg_id, &dep_tree, &mut processed, &mut sorted);
    }

    sorted
}

fn sort_workspace_inner<'m>(
    ws_meta: &'m cargo_metadata::Metadata,
    pkg_id: &'m cargo_metadata::PackageId,
    dep_tree: &std::collections::HashMap<
        &'m cargo_metadata::PackageId,
        &'m Vec<cargo_metadata::PackageId>,
    >,
    processed: &mut std::collections::HashSet<&'m cargo_metadata::PackageId>,
    sorted: &mut Vec<&'m cargo_metadata::PackageId>,
) {
    if !processed.insert(pkg_id) {
        return;
    }

    for dep_id in dep_tree[pkg_id]
        .iter()
        .filter(|dep_id| dep_tree.contains_key(dep_id))
    {
        sort_workspace_inner(ws_meta, dep_id, dep_tree, processed, sorted);
    }

    sorted.push(pkg_id);
}

#[cfg(test)]
mod test {
    use super::*;

    #[allow(unused_imports)] // Not being detected
    use assert_fs::prelude::*;
    use predicates::prelude::*;

    mod parse_cargo_config {
        use super::*;

        #[test]
        fn doesnt_panic() {
            parse_cargo_config(Path::new("Cargo.toml")).unwrap();
        }
    }

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

            set_package_version(manifest_path.path(), "2.0.0").unwrap();

            let meta = cargo_metadata::MetadataCommand::new()
                .manifest_path(manifest_path.path())
                .exec()
                .unwrap();
            assert_eq!(meta.packages[0].version.to_string(), "2.0.0");

            temp.close().unwrap();
        }
    }

    mod set_dependency_version {
        use super::*;

        #[test]
        fn preserve_table_order() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]
    foo = { version = "1.0", path = "../" }

    [package.metadata.release]
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::diff(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]
    foo = { version = "2.0", path = "../" }

    [package.metadata.release]
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn dependencies() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [build-dependencies]

    [dependencies]
    foo = { version = "1.0", path = "../" }
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::diff(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [build-dependencies]

    [dependencies]
    foo = { version = "2.0", path = "../" }
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn dev_dependencies() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]

    [dev-dependencies]
    foo = { version = "1.0", path = "../" }
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::diff(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]

    [dev-dependencies]
    foo = { version = "2.0", path = "../" }
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn build_dependencies() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dev-dependencies]

    [build-dependencies]
    foo = { version = "1.0", path = "../" }
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::diff(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dev-dependencies]

    [build-dependencies]
    foo = { version = "2.0", path = "../" }
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn all_dependencies() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]
    foo = { version = "1.0", path = "../" }

    [build-dependencies]
    foo = { version = "1.0", path = "../" }

    [dev-dependencies]
    foo = { version = "1.0", path = "../" }
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::diff(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]
    foo = { version = "2.0", path = "../" }

    [build-dependencies]
    foo = { version = "2.0", path = "../" }

    [dev-dependencies]
    foo = { version = "2.0", path = "../" }
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn no_path() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [build-dependencies]

    [dependencies]
    foo = "1.0"
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::diff(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [build-dependencies]

    [dependencies]
    foo = "1.0"
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn no_version() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]
    foo = { path = "../" }
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::diff(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [dependencies]
    foo = { path = "../" }
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

            temp.close().unwrap();
        }

        #[test]
        fn out_of_line_table() {
            let temp = assert_fs::TempDir::new().unwrap();
            temp.copy_from("tests/fixtures/simple", &["**"]).unwrap();
            let manifest_path = temp.child("Cargo.toml");
            manifest_path
                .write_str(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [build-dependencies]

    [dependencies.foo]
    version = "1.0"
    path = "../"
    "#,
                )
                .unwrap();

            set_dependency_version(manifest_path.path(), "foo", "2.0").unwrap();

            manifest_path.assert(
                predicate::str::diff(
                    r#"
    [package]
    name = "t"
    version = "0.1.0"
    authors = []
    edition = "2018"

    [build-dependencies]

    [dependencies.foo]
    version = "2.0"
    path = "../"
    "#,
                )
                .from_utf8()
                .from_file_path(),
            );

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

            set_package_version(manifest_path.path(), "2.0.0").unwrap();
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

            set_package_version(manifest_path.path(), "2.0.0").unwrap();
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

            set_package_version(manifest_path.path(), "2.0.0").unwrap();
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
}
