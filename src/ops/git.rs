use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use bstr::ByteSlice;

use crate::error::CargoResult;
use crate::ops::cmd::call_on_path;

pub fn fetch(dir: &Path, remote: &str, branch: &str) -> CargoResult<()> {
    Command::new("git")
        .arg("fetch")
        .arg(remote)
        .arg(branch)
        .current_dir(dir)
        .output()
        .map(|_| ())
        .map_err(|_| anyhow::format_err!("`git` not found"))
}

pub fn is_behind_remote(dir: &Path, remote: &str, branch: &str) -> CargoResult<bool> {
    let repo = git2::Repository::discover(dir)?;

    let branch_id = repo.revparse_single(branch)?.id();

    let remote_branch = format!("{remote}/{branch}");
    let behind = match repo.revparse_single(&remote_branch) {
        Ok(o) => {
            let remote_branch_id = o.id();

            let base_id = repo.merge_base(remote_branch_id, branch_id)?;

            log::trace!("{remote_branch}: {remote_branch_id}");
            log::trace!("merge base: {base_id}");

            base_id != remote_branch_id
        }
        Err(err) => {
            let _ = crate::ops::shell::warn(format!("push target `{remote_branch}` doesn't exist"));
            log::trace!("error {err}");
            false
        }
    };

    Ok(behind)
}

pub fn is_local_unchanged(dir: &Path, remote: &str, branch: &str) -> CargoResult<bool> {
    let repo = git2::Repository::discover(dir)?;

    let branch_id = repo.revparse_single(branch)?.id();

    let remote_branch = format!("{remote}/{branch}");
    let unchanged = match repo.revparse_single(&remote_branch) {
        Ok(o) => {
            let remote_branch_id = o.id();

            let base_id = repo.merge_base(remote_branch_id, branch_id)?;

            log::trace!("{remote_branch}: {remote_branch_id}");
            log::trace!("merge base: {base_id}");

            base_id == branch_id
        }
        Err(err) => {
            let _ = crate::ops::shell::warn(format!("push target `{remote_branch}` doesn't exist"));
            log::trace!("error {err}");
            false
        }
    };

    Ok(unchanged)
}

pub fn current_branch(dir: &Path) -> CargoResult<String> {
    let repo = git2::Repository::discover(dir)?;

    let resolved = repo.head()?.resolve()?;
    let name = resolved.shorthand().unwrap_or("HEAD");
    Ok(name.to_owned())
}

pub fn is_dirty(dir: &Path) -> CargoResult<Option<Vec<String>>> {
    let repo = git2::Repository::discover(dir)?;

    let mut entries = Vec::new();

    let state = repo.state();
    let dirty_state = state != git2::RepositoryState::Clean;
    if dirty_state {
        entries.push(format!("Dirty because of state {state:?}"));
    }

    let mut options = git2::StatusOptions::new();
    options
        .show(git2::StatusShow::IndexAndWorkdir)
        .include_untracked(true);
    let statuses = repo.statuses(Some(&mut options))?;
    let dirty_tree = !statuses.is_empty();
    if dirty_tree {
        for status in statuses.iter() {
            let path = bytes2path(status.path_bytes());
            entries.push(format!("{} ({:?})", path.display(), status.status()));
        }
    }

    if entries.is_empty() {
        Ok(None)
    } else {
        Ok(Some(entries))
    }
}

pub fn changed_files(dir: &Path, tag: &str) -> CargoResult<Option<Vec<PathBuf>>> {
    let root = top_level(dir)?;

    let output = Command::new("git")
        .arg("diff")
        .arg(format!("{tag}..HEAD"))
        .arg("--name-only")
        .arg("--exit-code")
        .arg("--")
        .arg(".")
        .current_dir(dir)
        .output()?;
    match output.status.code() {
        Some(0) => Ok(Some(Vec::new())),
        Some(1) => {
            let paths = output
                .stdout
                .lines()
                .map(|l| root.join(l.to_path_lossy()))
                .collect();
            Ok(Some(paths))
        }
        _ => Ok(None), // For cases like non-existent tag
    }
}

pub fn commit_all(dir: &Path, msg: &str, sign: bool, dry_run: bool) -> CargoResult<bool> {
    let repo = git2::Repository::discover(dir)?;
    let mut options = git2::StatusOptions::new();
    options
        .show(git2::StatusShow::IndexAndWorkdir)
        .include_untracked(true);
    let statuses = repo.statuses(Some(&mut options))?;
    let dirty_tree = !statuses.is_empty();

    if dirty_tree || dry_run {
        call_on_path(
            vec!["git", "commit", if sign { "-S" } else { "" }, "-am", msg],
            dir,
            dry_run,
        )
    } else {
        log::debug!("No files changed, skipping commit");
        Ok(true)
    }
}

pub fn tag(dir: &Path, name: &str, msg: &str, sign: bool, dry_run: bool) -> CargoResult<bool> {
    let mut cmd = vec!["git", "tag", name];
    if !msg.is_empty() {
        cmd.extend(["-a", "-m", msg]);
        if sign {
            cmd.push("-s");
        }
    }
    call_on_path(cmd, dir, dry_run)
}

pub fn tag_exists(dir: &Path, name: &str) -> CargoResult<bool> {
    let repo = git2::Repository::discover(dir)?;

    let names = repo.tag_names(Some(name))?;
    Ok(!names.is_empty())
}

pub fn find_last_tag(dir: &Path, glob: &globset::GlobMatcher) -> Option<String> {
    let repo = git2::Repository::discover(dir).ok()?;
    let mut tags: std::collections::HashMap<git2::Oid, String> = Default::default();
    repo.tag_foreach(|id, name| {
        let name = String::from_utf8_lossy(name);
        let name = name.strip_prefix("refs/tags/").unwrap_or(&name);
        if glob.is_match(name) {
            let name = name.to_owned();
            let tag = repo.find_tag(id);
            let target = tag.and_then(|t| t.target());
            let commit = target.and_then(|t| t.peel_to_commit());
            if let Ok(commit) = commit {
                tags.insert(commit.id(), name);
            }
        }
        true
    })
    .ok()?;

    let mut revwalk = repo.revwalk().ok()?;
    revwalk.simplify_first_parent().ok()?;
    // If just walking first parents, shouldn't really need to sort
    revwalk.set_sorting(git2::Sort::NONE).ok()?;
    revwalk.push_head().ok()?;
    let name = revwalk.find_map(|id| {
        let id = id.ok()?;
        tags.remove(&id)
    })?;
    Some(name)
}

pub fn push<'s>(
    dir: &Path,
    remote: &str,
    refs: impl IntoIterator<Item = &'s str>,
    options: impl IntoIterator<Item = &'s str>,
    dry_run: bool,
) -> CargoResult<bool> {
    // Use an atomic push to ensure that e.g. if main and a tag are pushed together, and the local
    // main diverges from the remote main, that the push fails entirely.
    let mut command = vec!["git", "push", "--atomic"];

    for option in options {
        command.push("--push-option");
        command.push(option);
    }

    command.push(remote);

    let mut is_empty = true;
    for ref_ in refs {
        command.push(ref_);
        is_empty = false;
    }
    if is_empty {
        return Ok(true);
    }

    call_on_path(command, dir, dry_run)
}

pub fn top_level(dir: &Path) -> CargoResult<PathBuf> {
    let repo = git2::Repository::discover(dir)?;

    repo.workdir()
        .map(|p| p.to_owned())
        .ok_or_else(|| anyhow::format_err!("bare repos are unsupported"))
}

pub fn git_version() -> CargoResult<()> {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|_| ())
        .map_err(|_| anyhow::format_err!("`git` not found"))
}

// From git2 crate
#[cfg(unix)]
pub fn bytes2path(b: &[u8]) -> &Path {
    use std::os::unix::prelude::OsStrExt;
    Path::new(std::ffi::OsStr::from_bytes(b))
}

// From git2 crate
#[cfg(windows)]
pub fn bytes2path(b: &[u8]) -> &std::path::Path {
    use std::str;
    std::path::Path::new(str::from_utf8(b).unwrap())
}
