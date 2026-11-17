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

/// Resolve `remote` to the name of a configured remote
///
/// `remote` may be a remote name or the URL of a configured remote (matched against its `url` or
/// `pushurl`)
pub fn resolve_remote(dir: &Path, remote: &str) -> CargoResult<String> {
    let repo = git2::Repository::discover(dir)?;

    // `:` can't appear in a remote name, so anything without one is a name (or a local path). Pass
    // those through unchanged and let git report on remotes that don't exist, as before.
    if repo.find_remote(remote).is_ok() || !remote.contains(':') {
        return Ok(remote.to_owned());
    }

    let target = normalize_url(remote);
    let is_match = |url: Option<&str>| match (url, &target) {
        (Some(url), Some(target)) => normalize_url(url).as_ref() == Some(target),
        (Some(url), None) => url == remote,
        (None, _) => false,
    };

    let mut configured = Vec::new();
    let mut matches = Vec::new();
    for name in repo.remotes()?.iter().flatten() {
        let r = repo.find_remote(name)?;
        if is_match(r.url()) || is_match(r.pushurl()) {
            matches.push(name.to_owned());
        }
        configured.push((name.to_owned(), r.url().unwrap_or_default().to_owned()));
    }

    match matches.len() {
        1 => {
            let name = matches.pop().unwrap();
            log::debug!("push-remote `{remote}` resolved to remote `{name}`");
            Ok(name)
        }
        0 => {
            let mut msg = format!(
                "push-remote `{remote}` is neither a remote name nor the URL of a configured remote"
            );
            if configured.is_empty() {
                msg.push_str("\nno remotes are configured");
            } else {
                msg.push_str("\nconfigured remotes:");
                for (name, url) in &configured {
                    msg.push_str(&format!("\n  {name}  {url}"));
                }
            }
            msg.push_str(&format!(
                "\nto add it, run `git remote add <name> {remote}`"
            ));
            Err(anyhow::format_err!(msg))
        }
        _ => Err(anyhow::format_err!(
            "push-remote `{remote}` matches multiple remotes: {}; set push-remote to one of them",
            matches.join(", ")
        )),
    }
}

/// Normalize a remote URL to `(host, path)` so equivalent https/ssh forms compare equal
///
/// Returns `None` for local paths and `file://` URLs
fn normalize_url(url: &str) -> Option<(String, String)> {
    let (authority, path) = if let Some((scheme, rest)) = url.split_once("://") {
        if scheme.eq_ignore_ascii_case("file") {
            return None;
        }
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        // Drop the user and port
        let authority = authority
            .rsplit_once('@')
            .map(|(_, h)| h)
            .unwrap_or(authority);
        let host = authority
            .split_once(':')
            .map(|(h, _)| h)
            .unwrap_or(authority);
        (host, path)
    } else {
        // scp-like syntax: `[user@]host:path`, where the `:` comes before any `/`
        let (authority, path) = url.split_once(':')?;
        // A single letter is a Windows drive (`C:\repo`), not a host
        if authority.len() <= 1 || authority.contains('/') {
            return None;
        }
        let host = authority
            .rsplit_once('@')
            .map(|(_, h)| h)
            .unwrap_or(authority);
        (host, path)
    };
    if authority.is_empty() {
        return None;
    }

    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    Some((authority.to_ascii_lowercase(), path.to_owned()))
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn normalize_url_equivalent_forms() {
        let expected = Some(("github.com".to_owned(), "crate-ci/cargo-release".to_owned()));
        for url in [
            "https://github.com/crate-ci/cargo-release",
            "https://github.com/crate-ci/cargo-release.git",
            "https://github.com/crate-ci/cargo-release/",
            "https://user@GitHub.com/crate-ci/cargo-release",
            "git@github.com:crate-ci/cargo-release.git",
            "github.com:crate-ci/cargo-release",
            "ssh://git@github.com:22/crate-ci/cargo-release.git",
            "git://github.com/crate-ci/cargo-release",
        ] {
            assert_eq!(normalize_url(url), expected, "{url}");
        }
    }

    #[test]
    fn normalize_url_local_paths() {
        assert_eq!(normalize_url("/tmp/repo"), None);
        assert_eq!(normalize_url("../repo"), None);
        assert_eq!(normalize_url("./foo:bar"), None);
        assert_eq!(normalize_url("file:///tmp/repo"), None);
    }

    #[test]
    fn resolve_remote_by_name_and_url() {
        let dir = assert_fs::TempDir::new().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        repo.remote("origin", "https://github.com/me/cargo-release")
            .unwrap();
        repo.remote("upstream", "git@github.com:crate-ci/cargo-release.git")
            .unwrap();
        repo.remote("mirror", "https://example.com/mirror").unwrap();
        repo.remote_set_pushurl("mirror", Some("https://example.com/push-only"))
            .unwrap();

        assert_eq!(resolve_remote(dir.path(), "origin").unwrap(), "origin");
        assert_eq!(resolve_remote(dir.path(), "missing").unwrap(), "missing");
        assert_eq!(
            resolve_remote(dir.path(), "https://github.com/crate-ci/cargo-release").unwrap(),
            "upstream"
        );
        assert_eq!(
            resolve_remote(dir.path(), "https://example.com/push-only.git").unwrap(),
            "mirror"
        );

        let err = resolve_remote(dir.path(), "https://example.com/nope")
            .unwrap_err()
            .to_string();
        assert!(err.contains("upstream"), "{err}");

        repo.remote("upstream2", "https://github.com/crate-ci/cargo-release")
            .unwrap();
        let err = resolve_remote(dir.path(), "https://github.com/crate-ci/cargo-release")
            .unwrap_err()
            .to_string();
        assert!(err.contains("multiple remotes"), "{err}");
    }
}
