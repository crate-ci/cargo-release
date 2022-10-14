use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use bstr::ByteSlice;

use crate::error::FatalError;
use crate::ops::cmd::call_on_path;

pub fn fetch(dir: &Path, remote: &str, branch: &str) -> Result<(), FatalError> {
    Command::new("git")
        .arg("fetch")
        .arg(remote)
        .arg(branch)
        .current_dir(dir)
        .output()
        .map(|_| ())
        .map_err(|_| FatalError::GitBinError)
}

pub fn is_behind_remote(dir: &Path, remote: &str, branch: &str) -> Result<bool, FatalError> {
    let repo = git2::Repository::discover(dir)?;

    let branch_id = repo.revparse_single(branch)?.id();

    let remote_branch = format!("{}/{}", remote, branch);
    let behind = match repo.revparse_single(&remote_branch) {
        Ok(o) => {
            let remote_branch_id = o.id();

            let base_id = repo.merge_base(remote_branch_id, branch_id)?;

            log::trace!("{}: {}", remote_branch, remote_branch_id);
            log::trace!("merge base: {}", base_id);

            base_id != remote_branch_id
        }
        Err(err) => {
            log::warn!("Push target `{}` doesn't exist", remote_branch);
            log::trace!("Error {}", err);
            false
        }
    };

    Ok(behind)
}

pub fn current_branch(dir: &Path) -> Result<String, FatalError> {
    let repo = git2::Repository::discover(dir)?;

    let resolved = repo.head()?.resolve()?;
    let name = resolved.shorthand().unwrap_or("HEAD");
    Ok(name.to_owned())
}

pub fn is_dirty(dir: &Path) -> Result<bool, FatalError> {
    let output = Command::new("git")
        .arg("diff")
        .arg("HEAD")
        .arg("--exit-code")
        .arg("--name-only")
        .arg("--")
        .arg(".")
        .current_dir(dir)
        .output()
        .map_err(FatalError::from)?;
    let tracked_unclean = !output.status.success();
    if tracked_unclean {
        let tracked = String::from_utf8_lossy(&output.stdout);
        log::debug!("Dirty because of:\n{}", tracked.trim());
    }

    let output = Command::new("git")
        .arg("ls-files")
        .arg("--exclude-standard")
        .arg("--others")
        .current_dir(dir)
        .output()
        .map_err(FatalError::from)?;
    let untracked_files = String::from_utf8_lossy(&output.stdout);
    let untracked = !untracked_files.as_ref().trim().is_empty();
    if untracked {
        log::debug!("Dirty because of:\n{}", untracked_files.trim());
    }

    Ok(tracked_unclean || untracked)
}

pub fn changed_files(dir: &Path, tag: &str) -> Result<Option<Vec<PathBuf>>, FatalError> {
    let root = top_level(dir)?;

    let output = Command::new("git")
        .arg("diff")
        .arg(&format!("{}..HEAD", tag))
        .arg("--name-only")
        .arg("--exit-code")
        .arg("--")
        .arg(".")
        .current_dir(dir)
        .output()
        .map_err(FatalError::from)?;
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

pub fn commit_all(dir: &Path, msg: &str, sign: bool, dry_run: bool) -> Result<bool, FatalError> {
    call_on_path(
        vec!["git", "commit", if sign { "-S" } else { "" }, "-am", msg],
        dir,
        dry_run,
    )
}

pub fn tag(
    dir: &Path,
    name: &str,
    msg: &str,
    sign: bool,
    dry_run: bool,
) -> Result<bool, FatalError> {
    let mut cmd = vec!["git", "tag", name];
    if !msg.is_empty() {
        cmd.extend(["-a", "-m", msg]);
        if sign {
            cmd.push("-s");
        }
    }
    call_on_path(cmd, dir, dry_run)
}

pub fn tag_exists(dir: &Path, name: &str) -> Result<bool, FatalError> {
    let repo = git2::Repository::discover(dir)?;

    let names = repo.tag_names(Some(name))?;
    Ok(!names.is_empty())
}

pub fn push<'s>(
    dir: &Path,
    remote: &str,
    refs: impl IntoIterator<Item = &'s str>,
    options: impl IntoIterator<Item = &'s str>,
    dry_run: bool,
) -> Result<bool, FatalError> {
    let mut command = vec!["git", "push"];

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

pub fn top_level(dir: &Path) -> Result<PathBuf, FatalError> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(dir)
        .output()
        .map_err(FatalError::from)?;
    let path = std::str::from_utf8(&output.stdout)
        .map_err(FatalError::from)?
        .trim_end();
    Ok(Path::new(path).to_owned())
}

pub fn git_version() -> Result<(), FatalError> {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|_| ())
        .map_err(|_| FatalError::GitBinError)
}
