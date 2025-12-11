use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

use crate::error::CargoResult;

fn do_call(
    command: impl IntoIterator<Item = impl Into<String>>,
    path: Option<&Path>,
    envs: Option<BTreeMap<&OsStr, &OsStr>>,
    dry_run: bool,
) -> CargoResult<bool> {
    let command: Vec<_> = command.into_iter().map(|s| s.into()).collect();
    if let Some(path) = path {
        log::trace!("cd {}", path.display());
    }
    log::trace!("{}", command.join(" "));
    if dry_run {
        return Ok(true);
    }
    let mut iter = command.iter();
    let cmd_name = iter.next().unwrap();

    let mut cmd = Command::new(cmd_name);

    if let Some(p) = path {
        cmd.current_dir(p);
    }

    if let Some(e) = envs {
        cmd.envs(e.iter());
    }

    for arg in iter {
        if !arg.is_empty() {
            cmd.arg(arg);
        }
    }

    let ctx_dir = match path {
        Some(p) => format!(" within {}", p.display()),
        None => String::new(),
    };

    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::format_err!("failed to launch `{cmd_name}`{ctx_dir}: {e}"))?;
    let result = child
        .wait()
        .map_err(|e| anyhow::format_err!("failed to launch `{cmd_name}`{ctx_dir}: {e}"))?;

    Ok(result.success())
}

pub fn call(
    command: impl IntoIterator<Item = impl Into<String>>,
    dry_run: bool,
) -> CargoResult<bool> {
    do_call(command, None, None, dry_run)
}

pub fn call_on_path(
    command: impl IntoIterator<Item = impl Into<String>>,
    path: &Path,
    dry_run: bool,
) -> CargoResult<bool> {
    do_call(command, Some(path), None, dry_run)
}

pub fn call_with_env(
    command: impl IntoIterator<Item = impl Into<String>>,
    envs: BTreeMap<&OsStr, &OsStr>,
    path: &Path,
    dry_run: bool,
) -> CargoResult<bool> {
    do_call(command, Some(path), Some(envs), dry_run)
}
