use std::path::Path;
use std::path::PathBuf;

use crate::config;
use crate::error;
use crate::error::FatalError;
use crate::ops::cargo;
use crate::ops::git;
use crate::ops::replace::Template;
use crate::ops::version;
use crate::ops::version::VersionExt as _;

pub fn load(
    args: &config::ConfigArgs,
    ws_meta: &cargo_metadata::Metadata,
) -> Result<indexmap::IndexMap<cargo_metadata::PackageId, PackageRelease>, error::FatalError> {
    let root = git::top_level(ws_meta.workspace_root.as_std_path())?;

    let member_ids = cargo::sort_workspace(ws_meta);
    member_ids
        .iter()
        .filter_map(|p| PackageRelease::load(args, &root, ws_meta, &ws_meta[p]).transpose())
        .map(|p| p.map(|p| (p.meta.id.clone(), p)))
        .collect()
}

pub fn plan(
    mut pkgs: indexmap::IndexMap<cargo_metadata::PackageId, PackageRelease>,
) -> Result<indexmap::IndexMap<cargo_metadata::PackageId, PackageRelease>, error::FatalError> {
    let mut shared_max: Option<version::Version> = None;
    let mut shared_ids = indexmap::IndexSet::new();
    for (pkg_id, pkg) in pkgs.iter() {
        if pkg.config.shared_version() {
            shared_ids.insert(pkg_id.clone());
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
            let shared_pkg = &mut pkgs[&shared_id];
            if shared_pkg.prev_version.bare_version != shared_max.bare_version {
                shared_pkg.version = Some(shared_max.clone());
            }
        }
    }

    for pkg in pkgs.values_mut() {
        pkg.plan()?;
    }

    Ok(pkgs)
}

pub struct PackageRelease {
    pub meta: cargo_metadata::Package,
    pub manifest_path: PathBuf,
    pub package_root: PathBuf,
    pub is_root: bool,
    pub config: config::Config,

    pub package_content: Vec<PathBuf>,
    pub bin: bool,
    pub dependents: Vec<Dependency>,
    pub features: cargo::Features,

    pub prev_version: version::Version,
    pub prev_tag: String,

    pub version: Option<version::Version>,
    pub tag: Option<String>,
    pub post_version: Option<version::Version>,
}

impl PackageRelease {
    pub fn load(
        args: &config::ConfigArgs,
        git_root: &Path,
        ws_meta: &cargo_metadata::Metadata,
        pkg_meta: &cargo_metadata::Package,
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
            .map(|(pkg, dep)| Dependency {
                pkg: pkg.clone(),
                req: dep.req.clone(),
            })
            .collect();

        let is_root = git_root == package_root;
        let prev_version = version::Version::from(pkg_meta.version.clone());
        let tag_name = config.tag_name();
        let tag_prefix = config.tag_prefix(is_root);
        let name = pkg_meta.name.as_str();
        let prev_tag = render_tag(tag_name, tag_prefix, name, &prev_version, &prev_version);

        let version = None;
        let tag = None;
        let post_version = None;

        let pkg = PackageRelease {
            meta: pkg_meta.clone(),
            manifest_path: manifest_path.to_owned(),
            package_root: package_root.to_owned(),
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

    pub fn set_prev_tag(&mut self, prev_tag: String) {
        self.prev_tag = prev_tag;
    }

    pub fn bump(
        &mut self,
        level_or_version: &version::TargetVersion,
        metadata: Option<&str>,
    ) -> Result<(), FatalError> {
        self.version = level_or_version.bump(&self.prev_version.full_version, metadata)?;
        Ok(())
    }

    pub fn plan(&mut self) -> Result<(), FatalError> {
        if !self.config.release() {
            return Ok(());
        }

        let base = self.version.as_ref().unwrap_or(&self.prev_version);
        let tag = if self.config.tag() {
            let tag_name = self.config.tag_name();
            let tag_prefix = self.config.tag_prefix(self.is_root);
            let name = self.meta.name.as_str();
            Some(render_tag(
                tag_name,
                tag_prefix,
                name,
                &self.prev_version,
                base,
            ))
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

fn render_tag(
    tag_name: &str,
    tag_prefix: &str,
    name: &str,
    prev: &version::Version,
    base: &version::Version,
) -> String {
    let prev_version_var = prev.bare_version_string.as_str();
    let prev_metadata_var = prev.full_version.build.as_str();
    let version_var = base.bare_version_string.as_str();
    let metadata_var = base.full_version.build.as_str();
    let mut template = Template {
        prev_version: Some(prev_version_var),
        prev_metadata: Some(prev_metadata_var),
        version: Some(version_var),
        metadata: Some(metadata_var),
        crate_name: Some(name),
        ..Default::default()
    };

    let tag_prefix = template.render(tag_prefix);
    template.prefix = Some(&tag_prefix);
    template.render(tag_name)
}

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

pub struct Dependency {
    pub pkg: cargo_metadata::Package,
    pub req: semver::VersionReq,
}
