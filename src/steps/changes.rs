use crate::error::CargoResult;
use crate::error::CliError;
use crate::ops::git;
use crate::ops::version::VersionExt as _;
use crate::steps::plan;
use clap_cargo::style::{ERROR, GOOD, NOP, WARN};

/// Print commits since last tag
#[derive(Debug, Clone, clap::Args)]
pub struct ChangesStep {
    #[command(flatten)]
    manifest: clap_cargo::Manifest,

    /// Custom config file
    #[arg(short, long = "config", value_name = "PATH")]
    custom_config: Option<std::path::PathBuf>,

    /// Ignore implicit configuration files.
    #[arg(long)]
    isolated: bool,

    /// Unstable options
    #[arg(short = 'Z', value_name = "FEATURE")]
    z: Vec<crate::config::UnstableValues>,

    /// Comma-separated globs of branch names a release can happen from
    #[arg(long, value_delimiter = ',')]
    allow_branch: Option<Vec<String>>,

    /// The name of tag for the previous release.
    #[arg(long, value_name = "NAME", help_heading = "Version")]
    prev_tag_name: Option<String>,
}

impl ChangesStep {
    pub fn run(&self) -> Result<(), CliError> {
        git::git_version()?;

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
        }

        let pkgs = plan::plan(pkgs)?;

        let (selected_pkgs, _excluded_pkgs): (Vec<_>, Vec<_>) = pkgs
            .into_iter()
            .map(|(_, pkg)| pkg)
            .partition(|p| p.config.release());
        if selected_pkgs.is_empty() {
            log::info!("No packages selected.");
            return Err(2.into());
        }

        let dry_run = false;
        let mut failed = false;

        // STEP 0: Help the user make the right decisions.
        failed |= !super::verify_git_is_clean(
            ws_meta.workspace_root.as_std_path(),
            dry_run,
            log::Level::Warn,
        )?;

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

        changes(&ws_meta, &selected_pkgs)?;

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

pub fn changes(
    ws_meta: &cargo_metadata::Metadata,
    selected_pkgs: &[plan::PackageRelease],
) -> CargoResult<()> {
    for pkg in selected_pkgs {
        let version = pkg.planned_version.as_ref().unwrap_or(&pkg.initial_version);
        let crate_name = pkg.meta.name.as_str();
        if let Some(prior_tag_name) = &pkg.prior_tag {
            let workspace_root = ws_meta.workspace_root.as_std_path();
            let repo = git2::Repository::discover(workspace_root)?;

            let mut tag_id = None;
            let fq_prior_tag_name = format!("refs/tags/{prior_tag_name}");
            repo.tag_foreach(|id, name| {
                if name == fq_prior_tag_name.as_bytes() {
                    tag_id = Some(id);
                    false
                } else {
                    true
                }
            })?;
            let tag_id =
                tag_id.ok_or_else(|| anyhow::format_err!("could not find tag {prior_tag_name}"))?;

            let head_id = repo.head()?.peel_to_commit()?.id();

            let mut revwalk = repo.revwalk()?;
            revwalk.push_range(&format!("{tag_id}..{head_id}"))?;

            let mut commits = Vec::new();
            for commit_id in revwalk {
                let commit_id = commit_id?;
                let commit = repo.find_commit(commit_id)?;
                if 1 < commit.parent_count() {
                    // Assuming merge commits can be ignored
                    continue;
                }
                let parent_tree = commit.parent(0).ok().map(|c| c.tree()).transpose()?;
                let tree = commit.tree()?;
                let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

                let mut changed_paths = std::collections::BTreeSet::new();
                for delta in diff.deltas() {
                    let old_path = delta.old_file().path();
                    let new_path = delta.new_file().path();
                    for entry_relpath in [old_path, new_path].into_iter().flatten() {
                        for path in pkg
                            .package_content
                            .iter()
                            .filter_map(|p| p.strip_prefix(workspace_root).ok())
                        {
                            if path == entry_relpath {
                                changed_paths.insert(path.to_owned());
                            }
                        }
                    }
                }

                if !changed_paths.is_empty() {
                    let short_id =
                        String::from_utf8_lossy(&repo.find_object(commit_id, None)?.short_id()?)
                            .into_owned();
                    commits.push(PackageCommit {
                        id: commit_id,
                        short_id,
                        summary: String::from_utf8_lossy(commit.summary_bytes().unwrap_or(b""))
                            .into_owned(),
                        message: String::from_utf8_lossy(commit.message_bytes()).into_owned(),
                        paths: changed_paths,
                    });
                }
            }

            if !commits.is_empty() {
                crate::ops::shell::status(
                    "Changes",
                    format!(
                        "for {} from {} to {}",
                        crate_name, prior_tag_name, version.full_version_string
                    ),
                )?;
                let prefix = format!("{:>13}", " ");
                let mut max_status = None;
                for commit in &commits {
                    let _ = crate::ops::shell::write_stderr(&prefix, &NOP);
                    let _ = crate::ops::shell::write_stderr(&commit.short_id, &WARN);
                    let _ = crate::ops::shell::write_stderr(" ", &NOP);
                    let _ = crate::ops::shell::write_stderr(&commit.summary, &NOP);

                    let current_status = commit.status();
                    write_status(current_status);
                    let _ = crate::ops::shell::write_stderr("\n", &NOP);
                    match (current_status, max_status) {
                        (Some(cur), Some(max)) => {
                            max_status = Some(cur.max(max));
                        }
                        (Some(s), None) | (None, Some(s)) => {
                            max_status = Some(s);
                        }
                        (None, None) => {}
                    }
                }
                if version.full_version.is_prerelease() {
                    // Enough unknowns about pre-release to not bother
                    max_status = None;
                }
                let unbumped = pkg
                    .planned_tag
                    .as_deref()
                    .and_then(|t| git::tag_exists(workspace_root, t).ok())
                    .unwrap_or(false);
                let bumped = !unbumped;
                if let Some(max_status) = max_status {
                    let suggested = match max_status {
                        CommitStatus::Breaking => {
                            match (
                                version.full_version.major,
                                version.full_version.minor,
                                version.full_version.patch,
                            ) {
                                (0, 0, _) if bumped => None,
                                (0, 0, _) => Some("patch"),
                                (0, _, 0) if bumped => None,
                                (0, _, _) => Some("minor"),
                                (_, 0, 0) if bumped => None,
                                (_, _, _) => Some("major"),
                            }
                        }
                        CommitStatus::Feature => {
                            match (
                                version.full_version.major,
                                version.full_version.minor,
                                version.full_version.patch,
                            ) {
                                (0, 0, _) if bumped => None,
                                (0, 0, _) => Some("patch"),
                                (0, _, _) if bumped => None,
                                (0, _, _) => Some("patch"),
                                (_, _, 0) if bumped => None,
                                (_, _, _) => Some("minor"),
                            }
                        }
                        CommitStatus::Fix if bumped => None,
                        CommitStatus::Fix => Some("patch"),
                        CommitStatus::Ignore => None,
                    };
                    if let Some(suggested) = suggested {
                        let _ = crate::ops::shell::note(format!(
                            "to update the version, run `cargo release version -p {crate_name} {suggested}`"
                        ));
                    } else if unbumped {
                        let _ = crate::ops::shell::note(format!(
                            "to update the version, run `cargo release version -p {crate_name} <LEVEL|VERSION>`"
                        ));
                    }
                }
            }
        } else {
            log::debug!(
                "Cannot detect changes for {crate_name} because no tag was found. Try setting `--prev-tag-name <TAG>`.",
            );
        }
    }

    Ok(())
}

fn write_status(status: Option<CommitStatus>) {
    if let Some(status) = status {
        let suffix;
        let mut style = NOP;
        match status {
            CommitStatus::Breaking => {
                suffix = " (breaking)";
                style = ERROR;
            }
            CommitStatus::Feature => {
                suffix = " (feature)";
                style = WARN;
            }
            CommitStatus::Fix => {
                suffix = " (fix)";
                style = GOOD;
            }
            CommitStatus::Ignore => {
                suffix = "";
            }
        }
        let _ = crate::ops::shell::write_stderr(suffix, &style);
    }
}

#[derive(Clone, Debug)]
pub struct PackageCommit {
    pub id: git2::Oid,
    pub short_id: String,
    pub summary: String,
    pub message: String,
    pub paths: std::collections::BTreeSet<std::path::PathBuf>,
}

impl PackageCommit {
    pub fn status(&self) -> Option<CommitStatus> {
        if let Some(status) = self.conventional_status() {
            return status;
        }

        None
    }

    fn conventional_status(&self) -> Option<Option<CommitStatus>> {
        let parts = git_conventional::Commit::parse(&self.message).ok()?;
        if parts.breaking() {
            return Some(Some(CommitStatus::Breaking));
        }

        if [
            git_conventional::Type::CHORE,
            git_conventional::Type::TEST,
            git_conventional::Type::STYLE,
            git_conventional::Type::REFACTOR,
            git_conventional::Type::REVERT,
        ]
        .contains(&parts.type_())
        {
            Some(Some(CommitStatus::Ignore))
        } else if [
            git_conventional::Type::DOCS,
            git_conventional::Type::PERF,
            git_conventional::Type::FIX,
        ]
        .contains(&parts.type_())
        {
            Some(Some(CommitStatus::Fix))
        } else if [git_conventional::Type::FEAT].contains(&parts.type_()) {
            Some(Some(CommitStatus::Feature))
        } else {
            Some(None)
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommitStatus {
    Ignore,
    Fix,
    Feature,
    Breaking,
}
